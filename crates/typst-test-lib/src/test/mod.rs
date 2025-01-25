//! Test loading and on-disk manipulation.

use std::fmt::Debug;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};

use ecow::{eco_vec, EcoString, EcoVec};
use thiserror::Error;
use tiny_skia::Pixmap;
use typst::syntax::{FileId, Source, VirtualPath};

use crate::doc::{Document, LoadError, SaveError};
use crate::project::{Paths, Vcs};
use crate::{doc, stdx};

mod annotation;
mod id;
mod result;
mod suite;

pub use self::annotation::{Annotation, ParseAnnotationError};
pub use self::id::{Id, ParseIdError};
pub use self::result::{Kind as TestResultKind, SuiteResult, TestResult};
pub use self::suite::{CollectError as CollectSuiteError, Suite};

/// The default test input as source code.
pub const DEFAULT_TEST_INPUT: &str = include_str!("../../../../assets/default-test/test.typ");

/// The default test output as a compressed PNG.
pub const DEFAULT_TEST_OUTPUT: &[u8] = include_bytes!("../../../../assets/default-test/test.png");

/// References for a test.
#[derive(Debug, Clone)]
pub enum Reference {
    /// An ephemeral reference script used to compile the reference document on
    /// the fly.
    Ephemeral(EcoString),

    /// Persistent references which are stored on disk.
    Persistent(Document, Option<Box<oxipng::Options>>),
}

/// The kind of a unit test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    /// Test is compared to ephemeral references, these are compiled on the fly
    /// from a reference script.
    Ephemeral,

    /// Test is compared to persistent references, these are pre-compiled and
    /// loaded for comparison.
    Persistent,

    /// Test is only compiled.
    CompileOnly,
}

impl Kind {
    /// Whether this kind is is ephemeral.
    pub fn is_ephemeral(self) -> bool {
        matches!(self, Kind::Ephemeral)
    }

    /// Whether this kind is persistent.
    pub fn is_persistent(self) -> bool {
        matches!(self, Kind::Persistent)
    }

    /// Whether this kind is compile-only.
    pub fn is_compile_only(self) -> bool {
        matches!(self, Kind::CompileOnly)
    }

    /// Returns a kebab-case string representing this kind.
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Ephemeral => "ephemeral",
            Kind::Persistent => "persistent",
            Kind::CompileOnly => "compile-only",
        }
    }
}

impl Reference {
    /// The kind of this reference.
    pub fn kind(&self) -> Kind {
        match self {
            Self::Ephemeral(_) => Kind::Ephemeral,
            Self::Persistent(_, _) => Kind::Persistent,
        }
    }
}

/// A unit test.
///
/// A test can be created on disk directly using [`Test::create`] or
/// [`Test::create_default`]. If a test was created using [`Test::new`] it can
/// be persisted to disk using one of its `make_*` methods.
///
/// This type is cheap to clone.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Test {
    id: Id,
    kind: Kind,
    annotations: EcoVec<Annotation>,
}

impl Test {
    /// Creates a new compile-only test with no annotations.
    pub fn new(id: Id) -> Self {
        Self {
            id,
            kind: Kind::CompileOnly,
            annotations: eco_vec![],
        }
    }

    /// Attempt to load a test, returns `None` if no test could be found.
    pub fn try_collect(paths: &Paths, id: Id) -> Result<Option<Test>, CollectError> {
        let test_script = paths.test_script(&id);

        if !test_script.try_exists()? {
            return Ok(None);
        }

        let kind = if paths.test_ref_script(&id).try_exists()? {
            Kind::Ephemeral
        } else if paths.test_ref_dir(&id).try_exists()? {
            Kind::Persistent
        } else {
            Kind::CompileOnly
        };

        let annotations = {
            let reader = BufReader::new(File::options().read(true).open(test_script)?);

            let mut annotations = eco_vec![];
            for line in reader.lines() {
                let line = line?;
                let Some(line) = line.strip_prefix("///") else {
                    break;
                };

                annotations.push(line.trim().parse()?);
            }

            annotations
        };

        Ok(Some(Test {
            id,
            kind,
            annotations,
        }))
    }
}

impl Test {
    /// The id of this test.
    pub fn id(&self) -> &Id {
        &self.id
    }

    /// The kind of this test.
    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// This test's annotations.
    pub fn annotations(&self) -> &[Annotation] {
        &self.annotations
    }

    /// Whether this test has a skip annotation.
    pub fn is_skip(&self) -> bool {
        self.annotations.contains(&Annotation::Skip)
    }
}

impl Test {
    /// Creates a new default test on disk.
    pub fn create_default(paths: &Paths, id: Id) -> Result<Test, CreateError> {
        Self::create(
            paths,
            id,
            DEFAULT_TEST_INPUT,
            // NOTE(tinger): this image is already optimized
            Some(Reference::Persistent(
                Document::new([
                    Pixmap::decode_png(DEFAULT_TEST_OUTPUT).expect("bytes come from a valid PNG")
                ]),
                None,
            )),
        )
    }

    /// Creates a new test on disk.
    pub fn create(
        paths: &Paths,
        id: Id,
        source: &str,
        reference: Option<Reference>,
    ) -> Result<Test, CreateError> {
        let test_dir = paths.test_dir(&id);
        stdx::fs::create_dir(test_dir, true)?;

        let mut file = File::options()
            .write(true)
            .create_new(true)
            .open(paths.test_script(&id))?;

        file.write_all(source.as_bytes())?;

        let kind = reference
            .as_ref()
            .map(Reference::kind)
            .unwrap_or(Kind::CompileOnly);

        let annotations = source
            .lines()
            .take_while(|&l| l.starts_with("///"))
            .map(|l| {
                l.strip_prefix("///")
                    .expect("we only take leading annotation lines")
                    .parse()
            })
            .collect::<Result<_, _>>()?;

        let this = Self {
            id,
            kind,
            annotations,
        };

        match reference {
            Some(Reference::Ephemeral(reference)) => {
                this.create_reference_script(paths, reference.as_str())?;
            }
            Some(Reference::Persistent(reference, options)) => {
                this.create_reference_documents(paths, None, &reference, options.as_deref())?;
            }
            None => {}
        }

        Ok(this)
    }

    /// Creates this test's temporary directories, if they don't exist yet.
    pub fn create_temporary_directories(&self, paths: &Paths, vcs: Option<&Vcs>) -> io::Result<()> {
        self.delete_temporary_directories(paths)?;

        if self.kind.is_ephemeral() {
            stdx::fs::create_dir(paths.test_ref_dir(&self.id), true)?;
        }

        stdx::fs::create_dir(paths.test_out_dir(&self.id), true)?;
        stdx::fs::create_dir(paths.test_diff_dir(&self.id), true)?;

        if let Some(vcs) = vcs {
            self.ignore_temporary_directories(paths, vcs)?;
        }

        Ok(())
    }

    /// Creates this test's main script, this will truncate the file if it
    /// already exists.
    pub fn create_script(&self, paths: &Paths, source: &str) -> io::Result<()> {
        std::fs::write(paths.test_script(&self.id), source)?;
        Ok(())
    }

    /// Creates this test's reference script, this will truncate the file if it
    /// already exists.
    pub fn create_reference_script(&self, paths: &Paths, source: &str) -> io::Result<()> {
        std::fs::write(paths.test_ref_script(&self.id), source)?;
        Ok(())
    }

    /// Creates this test's persistent references.
    pub fn create_reference_documents(
        &self,
        paths: &Paths,
        vcs: Option<&Vcs>,
        reference: &Document,
        optimize_options: Option<&oxipng::Options>,
    ) -> Result<(), SaveError> {
        // NOTE(tinger): if there are already more pages than we want to create,
        // the surplus pages would persist and make every comparison fail due to
        // a page count mismatch, so we clear them to be sure.
        self.delete_reference_documents(paths)?;

        let ref_dir = paths.test_ref_dir(&self.id);
        stdx::fs::create_dir(&ref_dir, true)?;
        reference.save(&ref_dir, optimize_options)?;

        if self.kind().is_ephemeral() {
            if let Some(vcs) = vcs {
                self.ignore_reference_documents(paths, vcs)?;
            }
        }

        Ok(())
    }

    /// Deletes this test's directories and scripts, if they exist.
    pub fn delete(&self, paths: &Paths) -> io::Result<()> {
        self.delete_reference_documents(paths)?;
        self.delete_reference_script(paths)?;
        self.delete_temporary_directories(paths)?;

        stdx::fs::remove_file(paths.test_script(&self.id))?;
        stdx::fs::remove_dir(paths.test_dir(&self.id), true)?;

        Ok(())
    }

    /// Deletes this test's temporary directories, if they exist.
    pub fn delete_temporary_directories(&self, paths: &Paths) -> io::Result<()> {
        if self.kind.is_ephemeral() {
            stdx::fs::remove_dir(paths.test_ref_dir(&self.id), true)?;
        }

        stdx::fs::remove_dir(paths.test_out_dir(&self.id), true)?;
        stdx::fs::remove_dir(paths.test_diff_dir(&self.id), true)?;
        Ok(())
    }

    /// Deletes this test's main script, if it exists.
    pub fn delete_script(&self, paths: &Paths) -> io::Result<()> {
        stdx::fs::remove_file(paths.test_script(&self.id))?;
        Ok(())
    }

    /// Deletes this test's reference script, if it exists.
    pub fn delete_reference_script(&self, paths: &Paths) -> io::Result<()> {
        stdx::fs::remove_file(paths.test_ref_script(&self.id))?;
        Ok(())
    }

    /// Deletes this test's persistent reference documents, if they exist.
    pub fn delete_reference_documents(&self, paths: &Paths) -> io::Result<()> {
        stdx::fs::remove_dir(paths.test_ref_dir(&self.id), true)?;
        Ok(())
    }

    /// Ignores this test's temporary directories in the vcs.
    pub fn ignore_temporary_directories(&self, paths: &Paths, vcs: &Vcs) -> io::Result<()> {
        if self.kind.is_ephemeral() {
            vcs.ignore_dir(&paths.test_ref_dir(&self.id))?;
        }

        vcs.ignore_dir(&paths.test_out_dir(&self.id))?;
        vcs.ignore_dir(&paths.test_diff_dir(&self.id))?;

        Ok(())
    }

    /// Ignores this test's persistent reference documents in the vcs.
    pub fn ignore_reference_documents(&self, paths: &Paths, vcs: &Vcs) -> io::Result<()> {
        vcs.ignore_dir(&paths.test_ref_dir(&self.id))?;
        Ok(())
    }

    /// Ignores this test's persistent reference documents in the vcs.
    pub fn unignore_reference_documents(&self, paths: &Paths, vcs: &Vcs) -> io::Result<()> {
        vcs.unignore_dir(&paths.test_ref_dir(&self.id))?;
        Ok(())
    }

    /// Removes any previous references, if they exist and creates a reference
    /// script by copying the test script.
    pub fn make_ephemeral(&mut self, paths: &Paths, vcs: Option<&Vcs>) -> io::Result<()> {
        self.delete_reference_script(paths)?;
        self.delete_reference_documents(paths)?;
        if let Some(vcs) = vcs {
            self.ignore_reference_documents(paths, vcs)?;
        }

        std::fs::copy(paths.test_script(&self.id), paths.test_ref_script(&self.id))?;

        self.kind = Kind::Ephemeral;
        Ok(())
    }

    /// Removes any previous references, if they exist and creates persistent
    /// references from the given pages.
    pub fn make_persistent(
        &mut self,
        paths: &Paths,
        vcs: Option<&Vcs>,
        reference: &Document,
        optimize_options: Option<&oxipng::Options>,
    ) -> Result<(), SaveError> {
        self.delete_reference_script(paths)?;
        self.create_reference_documents(paths, vcs, reference, optimize_options)?;
        if let Some(vcs) = vcs {
            self.unignore_reference_documents(paths, vcs)?;
        }

        self.kind = Kind::Persistent;
        Ok(())
    }

    /// Removes any previous references, if they exist.
    pub fn make_compile_only(&mut self, paths: &Paths, vcs: Option<&Vcs>) -> io::Result<()> {
        self.delete_reference_documents(paths)?;
        self.delete_reference_script(paths)?;
        if let Some(vcs) = vcs {
            self.ignore_reference_documents(paths, vcs)?;
        }

        self.kind = Kind::CompileOnly;
        Ok(())
    }

    /// Loads the test script source of this test.
    pub fn load_source(&self, paths: &Paths) -> io::Result<Source> {
        let test_script = paths.test_script(&self.id);

        Ok(Source::new(
            FileId::new(
                None,
                VirtualPath::new(
                    test_script
                        .strip_prefix(paths.project_root())
                        .unwrap_or(&test_script),
                ),
            ),
            std::fs::read_to_string(test_script)?,
        ))
    }

    /// Loads the reference test script source of this test, if this test is
    /// ephemeral.
    pub fn load_reference_source(&self, paths: &Paths) -> io::Result<Option<Source>> {
        if !self.kind().is_ephemeral() {
            return Ok(None);
        }

        let ref_script = paths.test_ref_script(&self.id);
        Ok(Some(Source::new(
            FileId::new(
                None,
                VirtualPath::new(
                    ref_script
                        .strip_prefix(paths.project_root())
                        .unwrap_or(&ref_script),
                ),
            ),
            std::fs::read_to_string(ref_script)?,
        )))
    }

    /// Loads the persistent reference pages of this test, if they exist.
    pub fn load_reference_documents(&self, paths: &Paths) -> Result<Option<Document>, LoadError> {
        match self.kind {
            Kind::Persistent => Document::load(paths.test_ref_dir(&self.id)).map(Some),
            _ => Ok(None),
        }
    }
}

/// Returned by [`Test::create`].
#[derive(Debug, Error)]
pub enum CreateError {
    /// An error occurred while parsing a test annotation.
    #[error("an error occurred while parsing a test annotation")]
    Annotation(#[from] ParseAnnotationError),

    /// An error occurred while saving test files.
    #[error("an error occurred while saving test files")]
    Save(#[from] doc::SaveError),

    /// An io error occurred.
    #[error("an io error occurred")]
    Io(#[from] io::Error),
}

/// Returned by [`Test::try_collect`].
#[derive(Debug, Error)]
pub enum CollectError {
    /// An error occurred while parsing a test annotation.
    #[error("an error occurred while parsing a test annotation")]
    Annotation(#[from] ParseAnnotationError),

    /// An io error occurred.
    #[error("an io error occurred")]
    Io(#[from] io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::_dev;
    use crate::_dev::fs::Setup;

    fn id(id: &str) -> Id {
        Id::new(id).unwrap()
    }

    fn test(test_id: &str) -> Test {
        Test::new(id(test_id))
    }

    fn setup_all(root: &mut Setup) -> &mut Setup {
        root.setup_file("tests/compile-only/test.typ", "Hello World")
            .setup_file("tests/ephemeral/test.typ", "Hello World")
            .setup_file("tests/ephemeral/ref.typ", "Hello\nWorld")
            .setup_file("tests/persistent/test.typ", "Hello World")
            .setup_dir("tests/persistent/ref")
    }

    #[test]
    fn test_create() {
        _dev::fs::TempEnv::run(
            |root| root.setup_dir("tests"),
            |root| {
                let paths = Paths::new(root, None);
                Test::create(&paths, id("compile-only"), "Hello World", None).unwrap();

                Test::create(
                    &paths,
                    id("ephemeral"),
                    "Hello World",
                    Some(Reference::Ephemeral("Hello\nWorld".into())),
                )
                .unwrap();

                Test::create(
                    &paths,
                    id("persistent"),
                    "Hello World",
                    Some(Reference::Persistent(Document::new(vec![]), None)),
                )
                .unwrap();

                Test::create_default(&paths, id("default")).unwrap();
            },
            |root| {
                root.expect_file_content("tests/compile-only/test.typ", "Hello World")
                    .expect_file_content("tests/ephemeral/test.typ", "Hello World")
                    .expect_file_content("tests/ephemeral/ref.typ", "Hello\nWorld")
                    .expect_file_content("tests/persistent/test.typ", "Hello World")
                    .expect_file_content("tests/default/test.typ", DEFAULT_TEST_INPUT)
                    .expect_file("tests/default/ref/1.png")
                    .expect_dir("tests/persistent/ref")
            },
        );
    }

    #[test]
    fn test_make_ephemeral() {
        _dev::fs::TempEnv::run(
            setup_all,
            |root| {
                let paths = Paths::new(root, None);
                test("compile-only").make_ephemeral(&paths, None).unwrap();
                test("ephemeral").make_ephemeral(&paths, None).unwrap();
                test("persistent").make_ephemeral(&paths, None).unwrap();
            },
            |root| {
                root.expect_file_content("tests/compile-only/test.typ", "Hello World")
                    .expect_file_content("tests/compile-only/ref.typ", "Hello World")
                    .expect_file_content("tests/ephemeral/test.typ", "Hello World")
                    .expect_file_content("tests/ephemeral/ref.typ", "Hello World")
                    .expect_file_content("tests/persistent/test.typ", "Hello World")
                    .expect_file_content("tests/persistent/ref.typ", "Hello World")
            },
        );
    }

    #[test]
    fn test_make_persistent() {
        _dev::fs::TempEnv::run(
            setup_all,
            |root| {
                let paths = Paths::new(root, None);
                test("compile-only")
                    .make_persistent(&paths, None, &Document::new([]), None)
                    .unwrap();

                test("ephemeral")
                    .make_persistent(&paths, None, &Document::new([]), None)
                    .unwrap();

                test("persistent")
                    .make_persistent(&paths, None, &Document::new([]), None)
                    .unwrap();
            },
            |root| {
                root.expect_file_content("tests/compile-only/test.typ", "Hello World")
                    .expect_dir("tests/compile-only/ref")
                    .expect_file_content("tests/ephemeral/test.typ", "Hello World")
                    .expect_dir("tests/ephemeral/ref")
                    .expect_file_content("tests/persistent/test.typ", "Hello World")
                    .expect_dir("tests/persistent/ref")
            },
        );
    }

    #[test]
    fn test_make_compile_only() {
        _dev::fs::TempEnv::run(
            setup_all,
            |root| {
                let paths = Paths::new(root, None);
                test("compile-only")
                    .make_compile_only(&paths, None)
                    .unwrap();
                test("ephemeral").make_compile_only(&paths, None).unwrap();
                test("persistent").make_compile_only(&paths, None).unwrap();
            },
            |root| {
                root.expect_file_content("tests/compile-only/test.typ", "Hello World")
                    .expect_file_content("tests/ephemeral/test.typ", "Hello World")
                    .expect_file_content("tests/persistent/test.typ", "Hello World")
            },
        );
    }

    #[test]
    fn test_load_sources() {
        _dev::fs::TempEnv::run_no_check(
            |root| {
                root.setup_file("tests/fancy/test.typ", "Hello World")
                    .setup_file("tests/fancy/ref.typ", "Hello\nWorld")
            },
            |root| {
                let paths = Paths::new(root, None);

                let mut test = test("fancy");
                test.kind = Kind::Ephemeral;

                test.load_source(&paths).unwrap();
                test.load_reference_source(&paths).unwrap().unwrap();
            },
        );
    }

    #[test]
    fn test_sources_virtual() {
        _dev::fs::TempEnv::run_no_check(
            |root| root.setup_file_empty("tests/fancy/test.typ"),
            |root| {
                let paths = Paths::new(root, None);

                let test = test("fancy");

                let source = test.load_source(&paths).unwrap();
                assert_eq!(
                    source.id().vpath().resolve(root).unwrap(),
                    root.join("tests/fancy/test.typ")
                );
            },
        );
    }
}
