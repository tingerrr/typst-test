//! Test discovery.

use std::collections::BTreeMap;
use std::fs::File;
use std::io;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::Arc;

use ecow::{eco_vec, EcoVec};

use super::{ReferenceKind, Test};
use crate::store::project::{Resolver, TestTarget};
use crate::test::id::{Identifier, ParseIdentifierError};
use crate::test::{Annotation, ParseAnnotationError};
use crate::test_set;
use crate::test_set::TestSet;

/// An error that can occur during [`Test`] collection using a [`Collector`].
#[derive(Debug, thiserror::Error)]
pub enum CollectError {
    /// An error occurred while traversing directories.
    #[error("an error occurred while traversing directories")]
    WalkDir(#[from] ignore::Error),

    /// An io error occurred.
    #[error("an io error occurred")]
    Io(#[from] io::Error),

    /// An error occurred while trying to parse a test identifier.
    #[error("an error occurred while collecting a test")]
    Test(#[from] ParseIdentifierError),

    /// An error occurred while trying parsing a test's annotations.
    #[error("an error occurred while parsing a test's annotations")]
    Annotation(#[from] ParseAnnotationError),
}

/// Recursively collects tests, applying test set matchers and respecting ignore
/// files.
#[derive(Debug)]
pub struct Collector<'p> {
    resolver: &'p (dyn Resolver + Sync),
    matcher: Arc<dyn TestSet>,
    tests: BTreeMap<Identifier, Test>,
    filtered: BTreeMap<Identifier, Test>,
    errors: Vec<(Option<PathBuf>, CollectError)>,
}

impl<'p> Collector<'p> {
    /// Creates a new collector for the given test root.
    pub fn new(project: &'p (dyn Resolver + Sync)) -> Self {
        Self {
            resolver: project,
            matcher: test_set::builtin::default(),
            tests: BTreeMap::new(),
            filtered: BTreeMap::new(),
            errors: vec![],
        }
    }

    /// Returns a reference to the [`Resolver`] used by this collector.
    pub fn resolver(&self) -> &'p (dyn Resolver + Sync) {
        self.resolver
    }

    /// Returns a reference to the matcher used by this collector.
    pub fn matcher(&self) -> &dyn TestSet {
        &*self.matcher
    }

    /// Returns a reference to the tests which were collected by this collector.
    pub fn tests(&self) -> &BTreeMap<Identifier, Test> {
        &self.tests
    }

    /// Returns a reference to the tests which did not match the matcher of this
    /// collector.
    pub fn filtered(&self) -> &BTreeMap<Identifier, Test> {
        &self.filtered
    }

    /// Returns a reference to the test root used by this collector.
    ///
    /// The errors may contain the path of the directory at which the error
    /// occurred.
    pub fn errors(&self) -> &[(Option<PathBuf>, CollectError)] {
        &self.errors
    }

    /// Takes ownership of the tests which were collected by this collector.
    pub fn take_tests(&mut self) -> BTreeMap<Identifier, Test> {
        std::mem::take(&mut self.tests)
    }

    /// Takes ownership of the tests which did not match the matcher of this
    /// collector.
    pub fn take_filtered(&mut self) -> BTreeMap<Identifier, Test> {
        std::mem::take(&mut self.filtered)
    }

    /// Takes ownership of the test root used by this collector.
    ///
    /// The errors may contain the path of the directory at which the error
    /// occurred.
    pub fn take_errors(&mut self) -> Vec<(Option<PathBuf>, CollectError)> {
        std::mem::take(&mut self.errors)
    }

    /// Sets the test set matcher used for this collector, the matcher is
    /// applied to each test after it's type and annotations have been checked.
    pub fn with_test_set<T: TestSet + 'static>(&mut self, test_set: T) -> &mut Self {
        self.matcher = Arc::new(test_set);
        self
    }

    /// Starts collecting tests recursively.
    pub fn collect(&mut self) {
        // TODO: filtering is currently very project specific which will require
        // more than one collector per project structure version
        // the same applies to collect_single
        let reserved = self.resolver.reserved();
        for entry in ignore::WalkBuilder::new(self.resolver.test_root())
            .filter_entry(|entry| {
                if !entry.file_type().is_some_and(|t| t.is_dir()) {
                    // don't yield files
                    return false;
                }

                let Some(name) = entry.file_name().to_str() else {
                    // don't yield non UTF-8 paths
                    return false;
                };

                if reserved.contains(&name) {
                    // ignore reserved directories
                    return false;
                }

                // TODO: this will filter out potentially valid test roots if they aren't default
                // ensure directory is valid component
                if !Identifier::is_component_valid(name) {
                    return false;
                }

                true
            })
            .build()
            .skip(1)
        {
            match entry {
                Ok(entry) => {
                    let rel = entry
                        .path()
                        .strip_prefix(self.resolver.test_root())
                        .expect("must be within test_root");

                    let id = Identifier::new_from_path(rel).expect("all components must be valid");
                    if let Err(err) = self.collect_single_inner(id) {
                        self.errors.push((Some(entry.into_path()), err))
                    }
                }
                Err(err) => self.errors.push((None, CollectError::WalkDir(err))),
            }
        }
    }

    /// Attempts to collect a single test.
    pub fn collect_single(&mut self, id: Identifier) {
        if let Err(err) = self.collect_single_inner(id.clone()) {
            let test_dir = self
                .resolver
                .resolve(&id, TestTarget::TestDir)
                .to_path_buf();
            self.errors.push((Some(test_dir), err))
        }
    }

    fn collect_single_inner(&mut self, id: Identifier) -> Result<(), CollectError> {
        let test_path = self.resolver.resolve(&id, TestTarget::TestScript);
        if !test_path.try_exists()? {
            return Ok(());
        }

        let reference = self.get_reference_kind(&id)?;
        let annotations = self.get_test_annotations(&id)?;

        let test = Test {
            id,
            ref_kind: reference,
            annotations,
        };

        if self.matcher.contains(&test) {
            self.tests.insert(test.id.clone(), test);
        } else {
            self.filtered.insert(test.id.clone(), test);
        }

        Ok(())
    }

    /// Returns the reference kind for a test.
    pub fn get_reference_kind(&mut self, id: &Identifier) -> io::Result<Option<ReferenceKind>> {
        if self
            .resolver
            .resolve(id, TestTarget::RefScript)
            .try_exists()?
        {
            return Ok(Some(ReferenceKind::Ephemeral));
        }

        if self.resolver.resolve(id, TestTarget::RefDir).try_exists()? {
            return Ok(Some(ReferenceKind::Persistent));
        }

        Ok(None)
    }

    /// Returns the annotations for a test.
    ///
    /// At this moment only the `ignored` annotation is returned.
    pub fn get_test_annotations(
        &mut self,
        id: &Identifier,
    ) -> Result<EcoVec<Annotation>, CollectError> {
        let reader = BufReader::new(
            File::options()
                .read(true)
                .open(self.resolver.resolve(id, TestTarget::TestScript))?,
        );

        let mut annotations = eco_vec![];
        for line in reader.lines() {
            let line = line?;
            if !line.starts_with("///") {
                break;
            }

            annotations.push(Annotation::parse_line(&line)?);
        }

        Ok(annotations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::_dev;
    use crate::store::project::v1::ResolverV1;

    const REFERENCE_BYTES: &[u8] = include_bytes!("../../../../../assets/default-test/test.png");

    #[test]
    fn test_collect() {
        _dev::fs::TempEnv::run_no_check(
            |root| {
                root
                    // compile only
                    .setup_file("tests/compile-only/test.typ", "Hello World")
                    // regular ephemeral
                    .setup_file("tests/compare/ephemeral/test.typ", "Hello World")
                    .setup_file("tests/compare/ephemeral/ref.typ", "Hello\nWorld")
                    // ephemeral despite ref directory
                    .setup_file(
                        "tests/compare/ephemeral-store/test.typ",
                        "/// [custom: foo]\nHello World",
                    )
                    .setup_file("tests/compare/ephemeral-store/ref.typ", "Hello\nWorld")
                    .setup_file("tests/compare/ephemeral-store/ref", REFERENCE_BYTES)
                    // persistent
                    .setup_file(
                        "tests/compare/persistent/test.typ",
                        "/// [custom: foo]\nHello World",
                    )
                    .setup_file("tests/compare/persistent/ref", REFERENCE_BYTES)
                    // not a test
                    .setup_file_empty("tests/not-a-test/test.txt")
                    // ignored test
                    .setup_file("tests/ignored/test.typ", "/// [ignore]\nHello World")
            },
            |root| {
                let project = ResolverV1::new(root, "tests");
                let mut collector = Collector::new(&project);
                collector.collect();

                let tests = [
                    ("compile-only", None, eco_vec![]),
                    (
                        "compare/ephemeral",
                        Some(ReferenceKind::Ephemeral),
                        eco_vec![],
                    ),
                    (
                        "compare/ephemeral-store",
                        Some(ReferenceKind::Ephemeral),
                        eco_vec![Annotation::Custom(
                            test_set::Identifier::new("foo").unwrap()
                        )],
                    ),
                    (
                        "compare/persistent",
                        Some(ReferenceKind::Persistent),
                        eco_vec![Annotation::Custom(
                            test_set::Identifier::new("foo").unwrap()
                        )],
                    ),
                ];

                let filtered = [("ignored", None, eco_vec![Annotation::Ignored])];

                for (key, kind, annotations) in tests {
                    let test = &collector.tests[key];
                    assert_eq!(test.annotations, annotations);
                    assert_eq!(test.ref_kind, kind);
                }

                for (key, kind, annotations) in filtered {
                    let test = &collector.filtered[key];
                    assert_eq!(test.annotations, annotations);
                    assert_eq!(test.ref_kind, kind);
                }

                assert!(collector.errors().is_empty());
            },
        );
    }
}
