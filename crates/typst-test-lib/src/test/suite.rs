//! Reading, and filtering of test suites.

use std::collections::BTreeMap;
use std::path::Path;
use std::{fs, io};

use thiserror::Error;

use super::{Id, Test};
use crate::project::Paths;
use crate::stdx::result::ResultEx;
use crate::test;
use crate::test_set::{Error as TestSetError, TestSet};

/// A suite of tests.
#[derive(Debug, Clone)]
pub struct Suite {
    matched: BTreeMap<Id, Test>,
    filtered: BTreeMap<Id, Test>,
    template: Option<String>,
}

impl Suite {
    /// Creates a new empty suite.
    pub fn new() -> Self {
        Self {
            matched: BTreeMap::new(),
            filtered: BTreeMap::new(),
            template: None,
        }
    }

    /// Recursively collects entries in the given directory, separating them
    /// into matched and filtered by the given [`TestSet`].
    #[tracing::instrument(skip(paths, test_set), fields(test_root = ?paths.test_root()))]
    pub fn collect(paths: &Paths, test_set: &TestSet) -> Result<Self, CollectError> {
        let root = paths.test_root();

        let mut this = Self {
            matched: BTreeMap::new(),
            filtered: BTreeMap::new(),
            template: None,
        };

        tracing::debug!("loading test template");
        if let Some(content) =
            fs::read_to_string(paths.template()).ignore(|e| e.kind() == io::ErrorKind::NotFound)?
        {
            this.template = Some(content);
        }

        tracing::debug!("collecting from test root directory");
        for entry in fs::read_dir(root)? {
            let entry = entry?;

            if entry.metadata()?.is_dir() {
                let abs = entry.path();
                let rel = abs
                    .strip_prefix(paths.test_root())
                    .expect("entry must be in full");

                this.collect_dir(paths, rel, test_set)?;
            }
        }

        Ok(this)
    }

    /// Recursively collect tests in the given directory.
    fn collect_dir(
        &mut self,
        paths: &Paths,
        dir: &Path,
        test_set: &TestSet,
    ) -> Result<(), CollectError> {
        let abs = paths.test_root().join(dir);

        tracing::trace!(?dir, "collecting directory");

        let id = Id::new_from_path(dir)?;

        if let Some(test) = Test::try_collect(paths, id.clone())? {
            if test_set.contains(&test)? {
                tracing::debug!(id = %test.id(), "matched test");
                self.matched.insert(id, test);
            } else {
                tracing::debug!(id = %test.id(), "filtered test");
                self.filtered.insert(id, test);
            }
        } else {
            for entry in fs::read_dir(&abs)? {
                let entry = entry?;

                if entry.metadata()?.is_dir() {
                    let abs = entry.path();
                    let rel = abs
                        .strip_prefix(paths.test_root())
                        .expect("entry must be in full");

                    tracing::trace!(path = ?rel, "reading directory entry");
                    self.collect_dir(paths, rel, test_set)?;
                }
            }
        }

        Ok(())
    }
}

impl Suite {
    /// All entries in this suite, this is constructed by adding the filtered
    /// tests to the matched tests.
    pub fn to_entries(&self) -> BTreeMap<Id, Test> {
        let mut entries = self.matched.clone();
        entries.extend(
            self.filtered
                .iter()
                .map(|(id, test)| (id.clone(), test.clone())),
        );
        entries
    }

    /// The matched entries in this suite, i.e. those which were contained in a
    /// [`TestSet`].
    ///
    /// The keys in this map are mutually exclusive with [`Suite::filtered`].
    pub fn matched(&self) -> &BTreeMap<Id, Test> {
        &self.matched
    }

    /// The filtered entries in this suite, i.e. those which were _not_
    /// contained in a [`TestSet`].
    ///
    /// The keys in this map are mutually exclusive with [`Suite::matched`].
    pub fn filtered(&self) -> &BTreeMap<Id, Test> {
        &self.filtered
    }

    /// The template for new tests in this suite.
    pub fn template(&self) -> Option<&str> {
        self.template.as_deref()
    }

    /// The total length of this suite.
    pub fn len(&self) -> usize {
        self.matched.len() + self.filtered.len()
    }

    /// Whether the suite is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for Suite {
    fn default() -> Self {
        Self::new()
    }
}

/// Returned by [`Suite::collect`].
#[derive(Debug, Error)]
pub enum CollectError {
    /// An error occurred while trying to parse a test [`Id`].
    #[error("an error occurred while collecting a test")]
    Id(#[from] test::ParseIdError),

    /// An error occurred while matching with a [`TestSet`].
    #[error("an error occurred while matching with a test set")]
    TestSet(#[from] TestSetError),

    /// An error occurred while trying to collect a test.
    #[error("an error occurred while collecting a test")]
    Test(#[from] test::CollectError),

    /// An io error occurred.
    #[error("an io error occurred")]
    Io(#[from] io::Error),
}

#[cfg(test)]
mod tests {
    use ecow::eco_vec;

    use super::*;
    use crate::_dev;
    use crate::test::{Annotation, Kind};
    use crate::test_set::eval;

    #[test]
    fn test_collect() {
        _dev::fs::TempEnv::run_no_check(
            |root| {
                root
                    // template
                    .setup_file("tests/template.typ", "Blah Blah")
                    // compile only
                    .setup_file("tests/compile-only/test.typ", "Hello World")
                    // regular ephemeral
                    .setup_file("tests/compare/ephemeral/test.typ", "Hello World")
                    .setup_file("tests/compare/ephemeral/ref.typ", "Hello\nWorld")
                    // ephemeral despite ref directory
                    .setup_file("tests/compare/ephemeral-store/test.typ", "Hello World")
                    .setup_file("tests/compare/ephemeral-store/ref.typ", "Hello\nWorld")
                    .setup_file("tests/compare/ephemeral-store/ref", "Blah Blah")
                    // persistent
                    .setup_file("tests/compare/persistent/test.typ", "Hello World")
                    .setup_file("tests/compare/persistent/ref", "Blah Blah")
                    // not a test
                    .setup_file_empty("tests/not-a-test/test.txt")
                    // ignored test
                    .setup_file("tests/ignored/test.typ", "/// [skip]\nHello World")
            },
            |root| {
                let paths = Paths::new(root, None);
                let suite = Suite::collect(
                    &paths,
                    &TestSet::new(eval::Context::empty(), eval::Set::built_in_all()),
                )
                .unwrap();

                let tests = [
                    ("compile-only", Kind::CompileOnly, eco_vec![]),
                    ("compare/ephemeral", Kind::Ephemeral, eco_vec![]),
                    ("compare/ephemeral-store", Kind::Ephemeral, eco_vec![]),
                    ("compare/persistent", Kind::Persistent, eco_vec![]),
                    ("ignored", Kind::CompileOnly, eco_vec![Annotation::Skip]),
                ];

                assert_eq!(suite.template, Some("Blah Blah".into()));

                for (key, kind, annotations) in tests {
                    let test = &suite.matched[key];
                    assert_eq!(test.annotations, annotations);
                    assert_eq!(test.kind, kind);
                }
            },
        );
    }
}
