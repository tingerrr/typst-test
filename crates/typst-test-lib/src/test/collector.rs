use std::collections::BTreeMap;
use std::fs::File;
use std::io;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use super::id::{Identifier, ParseIdentifierError};
use super::matcher::Matcher;
use super::{ReferenceKind, Test};
use crate::store::project::{Project, TestTarget};

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
pub struct Collector<'p, P> {
    project: &'p P,
    matcher: Matcher,
    tests: BTreeMap<Identifier, Test>,
    filtered: BTreeMap<Identifier, Test>,
    errors: Vec<(Option<PathBuf>, CollectError)>,
}

impl<'p, P: Project> Collector<'p, P> {
    /// Creates a new collector for the given test root.
    pub fn new(project: &'p P) -> Self {
        Self {
            project,
            matcher: Matcher::default(),
            tests: BTreeMap::new(),
            filtered: BTreeMap::new(),
            errors: vec![],
        }
    }

    /// Returns a reference to the [`Project`] used by this collector.
    pub fn project(&self) -> &'p P {
        &self.project
    }

    /// Returns a reference to the matcher used by this collector.
    pub fn matcher(&self) -> &Matcher {
        &self.matcher
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
    pub fn with_matcher(&mut self, matcher: Matcher) -> &mut Self {
        self.matcher = matcher;
        self
    }

    /// Starts collecting tests recursively.
    pub fn collect(&mut self) {
        for entry in ignore::WalkBuilder::new(self.project.test_root())
            .filter_entry(|entry| {
                if !entry.file_type().is_some_and(|t| t.is_dir()) {
                    eprintln!("filtered non dir  {:?}", entry.path());
                    // don't yield files
                    return false;
                }

                let Some(name) = entry.file_name().to_str() else {
                    eprintln!("filtered non utf8 {:?}", entry.path());
                    // don't yield non UTF-8 paths
                    return false;
                };

                if P::RESERVED.contains(&name) {
                    eprintln!("filtered reserved {:?}", entry.path());
                    // ignore reserved directories
                    return false;
                }

                // TODO: this will filter out potentially valid test roots if they aren't default
                // ensure directory is valid component
                if !Identifier::is_component_valid(name) {
                    eprintln!("filtered invalid component {:?}", entry.path());
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
                        .strip_prefix(self.project.test_root())
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
            let test_dir = self.project.resolve(&id, TestTarget::TestDir).to_path_buf();
            self.errors.push((Some(test_dir), err))
        }
    }

    fn collect_single_inner(&mut self, id: Identifier) -> Result<(), CollectError> {
        let test_path = self.project.resolve(&id, TestTarget::TestScript);
        if !test_path.try_exists()? {
            eprintln!("not a test: {id}");
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
            .project
            .resolve(id, TestTarget::RefScript)
            .try_exists()?
        {
            return Ok(Some(ReferenceKind::Ephemeral));
        }

        if self.project.resolve(id, TestTarget::RefDir).try_exists()? {
            return Ok(Some(ReferenceKind::Persistent));
        }

        Ok(None)
    }

    /// Returns the annotations for a test.
    ///
    /// At this moment only the `ignored` annotation is returned.
    pub fn get_test_annotations(&mut self, id: &Identifier) -> io::Result<bool> {
        let test_script = self.project.resolve(&id, TestTarget::TestScript);
        let reader = BufReader::new(File::options().read(true).open(test_script)?);

        let mut is_ignored = false;
        for line in reader.lines() {
            let line = line?;
            let Some(mut line) = line.strip_prefix("///") else {
                break;
            };

            line = line.strip_prefix(" ").unwrap_or(line);

            if line.trim() == "[ignore]" {
                is_ignored = true
            }
        }

        Ok(is_ignored)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::project::v1::ProjectV1;

    #[test]
    fn test_collect() {
        let tests = [
            (
                "compare/ephemeral-no-store",
                Some(ReferenceKind::Ephemeral),
                false,
            ),
            (
                "compare/ephemeral-with-store",
                Some(ReferenceKind::Ephemeral),
                false,
            ),
            ("compare/persistent", Some(ReferenceKind::Persistent), false),
            ("compile", None, false),
        ]
        .map(|(name, reference, is_ignored)| Test {
            id: Identifier::new(name).unwrap(),
            ref_kind: reference,
            is_ignored,
        });

        let project = ProjectV1::new("../../assets/test-assets/collect");
        let mut collector = Collector::new(&project);
        collector.collect();

        assert!(collector.errors().is_empty());
        assert_eq!(
            collector.take_filtered().into_values().collect::<Vec<_>>(),
            [Test {
                id: Identifier::new("ignored").unwrap(),
                ref_kind: None,
                is_ignored: true,
            }],
        );
        assert_eq!(
            collector.take_tests().into_values().collect::<Vec<_>>(),
            tests,
        );
    }
}
