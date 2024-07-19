use std::collections::BTreeMap;
use std::fs::File;
use std::io;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::Arc;

use super::{ReferenceKind, Test};
use crate::store::project::{Resolver, TestTarget};
use crate::test::id::{Identifier, ParseIdentifierError};
use crate::test_set;
use crate::test_set::TestSet;

/// An error that can occur during [`Test`] collection using a [`Collector`].
#[derive(Debug, thiserror::Error)]
pub enum CollectError {
    /// An error occured while traversing directories.
    #[error("an error occured while traversing directories")]
    WalkDir(#[from] ignore::Error),

    /// An io error occurred.
    #[error("an io error occurred")]
    Io(#[from] io::Error),

    /// An error occured while trying to parse a test identifier.
    #[error("an error occured while collecting a test")]
    Test(#[from] ParseIdentifierError),
}

/// Recursively collects tests, applying matchers and respecting ignore files.
#[derive(Debug)]
pub struct Collector<'p, R> {
    resolver: &'p R,
    matcher: Arc<dyn TestSet>,
    tests: BTreeMap<Identifier, Test>,
    filtered: BTreeMap<Identifier, Test>,
    errors: Vec<(Option<PathBuf>, CollectError)>,
}

impl<'p, R: Resolver + Sync> Collector<'p, R> {
    /// Creates a new collector for the given test root.
    pub fn new(project: &'p R) -> Self {
        Self {
            resolver: project,
            matcher: test_set::builtin::default(),
            tests: BTreeMap::new(),
            filtered: BTreeMap::new(),
            errors: vec![],
        }
    }

    /// Returns a reference to the [`Resolver`] used by this collector.
    pub fn resolver(&self) -> &'p R {
        &self.resolver
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
    /// occured.
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
    /// occured.
    pub fn take_errors(&mut self) -> Vec<(Option<PathBuf>, CollectError)> {
        std::mem::take(&mut self.errors)
    }

    /// Sets the matcher used for this collector, the matcher is applied to each
    /// test after it's type and annotations have been checked.
    pub fn with_matcher<M: TestSet + 'static>(&mut self, matcher: M) -> &mut Self {
        self.matcher = Arc::new(matcher);
        self
    }

    /// Starts collecting tests recursively.
    pub fn collect(&mut self) {
        // TODO: filtering is currently very project specific which will require
        // more than one collector per project structure version
        // the same applies to collect_single
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

                if R::RESERVED.contains(&name) {
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
        let is_ignored = self.get_test_annotations(&id)?;

        let test = Test {
            id,
            ref_kind: reference,
            is_ignored,
        };

        if self.matcher.is_match(&test) {
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
    pub fn get_test_annotations(&mut self, id: &Identifier) -> io::Result<bool> {
        let reader = BufReader::new(
            File::options()
                .read(true)
                .open(self.resolver.resolve(&id, TestTarget::TestScript))?,
        );

        let mut is_ignored = false;
        for line in reader.lines() {
            let line = line?;
            let Some(mut line) = line.strip_prefix("///") else {
                break;
            };

            line = line.strip_prefix(" ").unwrap_or(line);

            let Some(annotation) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) else {
                continue;
            };

            if annotation == "ignore" {
                is_ignored = true
            } else {
                // NOTE: this is implemented in two places and should be unified if more is added
                todo!("no proper annotation parsing implemented")
            }
        }

        Ok(is_ignored)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::_dev;
    use crate::store::project::v1::ResolverV1;

    const REFERNCE_BYTES: &[u8] = include_bytes!("../../../../../assets/default-test/test.png");

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
                    .setup_file("tests/compare/ephemeral-store/test.typ", "Hello World")
                    .setup_file("tests/compare/ephemeral-store/ref.typ", "Hello\nWorld")
                    .setup_file("tests/compare/ephemeral-store/ref", REFERNCE_BYTES)
                    // persistent
                    .setup_file("tests/compare/persistent/test.typ", "Hello World")
                    .setup_file("tests/compare/persistent/ref", REFERNCE_BYTES)
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
                    ("compile-only", None, false),
                    ("compare/ephemeral", Some(ReferenceKind::Ephemeral), false),
                    (
                        "compare/ephemeral-store",
                        Some(ReferenceKind::Ephemeral),
                        false,
                    ),
                    ("compare/persistent", Some(ReferenceKind::Persistent), false),
                ];

                let filtered = [("ignored", None, true)];

                for (key, kind, ignored) in tests {
                    let test = &collector.tests[key];
                    assert_eq!(test.is_ignored, ignored);
                    assert_eq!(test.ref_kind, kind);
                }

                for (key, kind, ignored) in filtered {
                    let test = &collector.filtered[key];
                    assert_eq!(test.is_ignored, ignored);
                    assert_eq!(test.ref_kind, kind);
                }

                assert!(collector.errors().is_empty());
            },
        );
    }
}
