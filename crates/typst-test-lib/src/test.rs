use std::fmt::Debug;
use std::fs::File;
use std::io::{self, Write};

use ecow::EcoString;
use id::Identifier;
use typst::syntax::{FileId, Source, VirtualPath};

use crate::store::page::{LoadError, PageFormat, SaveError};
use crate::store::project::{Project, TestTarget};
use crate::{store, util};

pub mod collector;
pub mod id;
pub mod matcher;

/// A thin test handle for managing on-disk resources.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Test {
    id: Identifier,
    ref_kind: Option<ReferenceKind>,
    is_ignored: bool,
}

/// References for a test.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum References<F: PageFormat> {
    /// An ephemeral reference script used to compile the reference document on
    /// the fly.
    Ephemeral(EcoString),

    /// Persistent references which are stored on disk.
    Persistent(Vec<F::Type>),
}

impl Test {
    /// Generates a new test which does not exist on disk yet.
    pub fn new(id: Identifier) -> Self {
        Self {
            id,
            ref_kind: None,
            is_ignored: false,
        }
    }

    /// Returns a reference to the identifier of this test.
    pub fn id(&self) -> &Identifier {
        &self.id
    }

    /// Returns a reference to the reference kind of this test.
    pub fn ref_kind(&self) -> Option<&ReferenceKind> {
        self.ref_kind.as_ref()
    }

    /// Returns whether this test is compared to a reference script.
    pub fn is_ephemeral(&self) -> bool {
        matches!(self.ref_kind, Some(ReferenceKind::Ephemeral))
    }

    /// Returns whether this test is compared to reference images directly.
    pub fn is_persistent(&self) -> bool {
        matches!(self.ref_kind, Some(ReferenceKind::Persistent))
    }

    /// Returns whether this test is not compared, but only compiled.
    pub fn is_compile_only(&self) -> bool {
        matches!(self.ref_kind, None)
    }

    /// Returns whether this test is marked as ignored.
    pub fn is_ignored(&self) -> bool {
        self.is_ignored
    }

    /// Creates a new test directly on disk.
    pub fn create<F: PageFormat, P: Project>(
        id: Identifier,
        project: &P,
        source: &str,
        references: Option<References<F>>,
    ) -> Result<Self, SaveError<F>> {
        let test_dir = project.resolve(&id, TestTarget::TestDir);
        util::fs::create_dir(test_dir, true)?;

        let mut file = File::options()
            .write(true)
            .create_new(true)
            .open(project.resolve(&id, TestTarget::TestScript))?;

        file.write_all(source.as_bytes())?;

        let ref_kind = match references {
            Some(References::Ephemeral(_)) => Some(ReferenceKind::Ephemeral),
            Some(References::Persistent(_)) => Some(ReferenceKind::Persistent),
            None => None,
        };

        let is_ignored = source
            .lines()
            .take_while(|&l| l.starts_with("///"))
            .filter(|l| {
                l.strip_prefix("///")
                    .is_some_and(|l| l.trim() == "[ignored]")
            })
            .next()
            .is_some();

        let test = Self {
            id,
            ref_kind,
            is_ignored,
        };

        match references {
            Some(References::Ephemeral(reference)) => {
                test.create_reference_script(project, reference.as_str())?;
            }
            Some(References::Persistent(pages)) => {
                test.create_reference_pages(project, pages.iter())?;
            }
            None => {}
        }

        Ok(test)
    }

    /// Creates this test's temporary directories.
    pub fn create_temporary_directories<P: Project>(&self, project: &P) -> io::Result<()> {
        if self.is_ephemeral() {
            util::fs::create_dir(project.resolve(&self.id, TestTarget::RefDir), true)?;
        }

        util::fs::create_dir(project.resolve(&self.id, TestTarget::OutDir), true)?;
        util::fs::create_dir(project.resolve(&self.id, TestTarget::DiffDir), true)?;
        Ok(())
    }

    /// Creates this test's reference script.
    pub fn create_reference_script<P: Project>(
        &self,
        project: &P,
        reference: &str,
    ) -> io::Result<()> {
        std::fs::write(project.resolve(&self.id, TestTarget::RefScript), reference)?;
        Ok(())
    }

    /// Creates this test's persistent references.
    pub fn create_reference_pages<'p, F: PageFormat, P: Project>(
        &self,
        project: &P,
        pages: impl IntoIterator<Item = &'p F::Type>,
    ) -> Result<(), SaveError<F>>
    where
        F::Type: 'p,
    {
        // TODO: the error handling is slightly inconvenient here, we don't
        // want this to be a format dependent error
        let ref_dir = project.resolve(&self.id, TestTarget::RefDir);
        util::fs::create_dir(ref_dir, true)?;
        store::page::save_pages::<F>(ref_dir, pages)?;
        Ok(())
    }

    /// Deletes this test's directories and scripts.
    pub fn delete<P: Project>(self, project: &P) -> io::Result<()> {
        self.delete_reference_pages(project)?;
        self.delete_reference_script(project)?;
        self.delete_temporary_directories(project)?;

        util::fs::remove_file(project.resolve(&self.id, TestTarget::TestScript))?;
        util::fs::remove_dir(project.resolve(&self.id, TestTarget::TestDir), true)?;

        Ok(())
    }

    /// Deletes this test's temporary directories.
    pub fn delete_temporary_directories<P: Project>(&self, project: &P) -> io::Result<()> {
        if self.is_ephemeral() {
            util::fs::remove_dir(project.resolve(&self.id, TestTarget::RefDir), true)?;
        }

        util::fs::remove_dir(project.resolve(&self.id, TestTarget::OutDir), true)?;
        util::fs::remove_dir(project.resolve(&self.id, TestTarget::DiffDir), true)?;
        Ok(())
    }

    /// Deletes this test's reference script.
    pub fn delete_reference_script<P: Project>(&self, project: &P) -> io::Result<()> {
        util::fs::remove_file(project.resolve(&self.id, TestTarget::RefScript))?;
        Ok(())
    }

    /// Deletes this test's persistent references.
    pub fn delete_reference_pages<P: Project>(&self, project: &P) -> io::Result<()> {
        util::fs::remove_dir(project.resolve(&self.id, TestTarget::RefDir), true)?;
        Ok(())
    }

    /// Removes any previous references and creates a reference script by
    /// copying the test script.
    pub fn make_ephemeral<P: Project>(&mut self, project: &P) -> io::Result<()> {
        self.delete_reference_script(project)?;
        self.delete_reference_pages(project)?;

        std::fs::copy(
            project.resolve(&self.id, TestTarget::TestScript),
            project.resolve(&self.id, TestTarget::RefScript),
        )?;

        self.ref_kind = Some(ReferenceKind::Ephemeral);
        Ok(())
    }

    /// Removes any previous references and creates a persistent references from the
    /// given pages.
    pub fn make_persistent<'p, F: PageFormat, P: Project>(
        &mut self,
        project: &P,
        pages: impl IntoIterator<Item = &'p F::Type>,
    ) -> Result<(), SaveError<F>>
    where
        F::Type: 'p,
    {
        self.delete_reference_script(project)?;
        self.delete_reference_pages(project)?;
        self.create_reference_pages(project, pages)?;

        self.ref_kind = Some(ReferenceKind::Persistent);
        Ok(())
    }

    /// Removes any previous references.
    pub fn make_compile_only<P: Project>(&mut self, project: &P) -> io::Result<()> {
        self.delete_reference_pages(project)?;
        self.delete_reference_script(project)?;

        self.ref_kind = None;
        Ok(())
    }

    /// Loads the test script source of this test.
    pub fn load_test_source<P: Project>(&self, project: &P) -> io::Result<Source> {
        let test_script = project.resolve(&self.id, TestTarget::TestScript);
        Ok(Source::new(
            FileId::new(None, VirtualPath::new(test_script)),
            std::fs::read_to_string(test_script)?,
        ))
    }

    /// Loads the reference test script source of this test, if one exists.
    pub fn load_ref_source<P: Project>(&self, project: &P) -> io::Result<Option<Source>> {
        match self.ref_kind {
            Some(ReferenceKind::Ephemeral) => {
                let ref_script = project.resolve(&self.id, TestTarget::RefScript);
                Ok(Some(Source::new(
                    FileId::new(None, VirtualPath::new(ref_script)),
                    std::fs::read_to_string(ref_script)?,
                )))
            }
            _ => Ok(None),
        }
    }

    /// Loads the persistent reference pages of this test, if they exist.
    pub fn load_ref_pages<F: PageFormat, P: Project>(
        &self,
        project: &P,
    ) -> Result<Option<Vec<F::Type>>, LoadError<F>> {
        match self.ref_kind {
            Some(ReferenceKind::Persistent) => {
                store::page::load_pages(project.resolve(&self.id, TestTarget::RefDir)).map(Some)
            }
            _ => Ok(None),
        }
    }
}

/// The kind of a [`Test`]'s reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReferenceKind {
    /// Ephemeral references are references which are compiled on the fly from a script.
    Ephemeral,

    /// Persistent references are pre compiled and fetched for comparison.
    Persistent,
}

#[cfg(test)]
mod tests {
    use store::page::Png;
    use store::project::legacy::ProjectLegacy;

    use super::*;
    use crate::_dev;
    use crate::_dev::fs::Setup;

    fn setup_all(root: &mut Setup) -> &mut Setup {
        root.setup_file("tests/compile-only/test.typ", "Hello World")
            .setup_file("tests/ephemeral/test.typ", "Hello World")
            .setup_file("tests/ephemeral/ref.typ", "Hello\nWorld")
            .setup_file("tests/persistent/test.typ", "Hello World")
            .setup_dir("tests/persistent/ref")
    }

    #[test]
    fn test_create_new() {
        _dev::fs::TempEnv::run(
            |root| root.setup_dir("tests"),
            |root| {
                let project = ProjectLegacy::new(root, "tests");
                Test::create::<Png, _>(
                    Identifier::new("compile-only").unwrap(),
                    &project,
                    "Hello World",
                    None,
                )
                .unwrap();

                Test::create::<Png, _>(
                    Identifier::new("ephemeral").unwrap(),
                    &project,
                    "Hello World",
                    Some(References::Ephemeral("Hello\nWorld".into())),
                )
                .unwrap();

                Test::create::<Png, _>(
                    Identifier::new("persistent").unwrap(),
                    &project,
                    "Hello World",
                    Some(References::Persistent(vec![])),
                )
                .unwrap();
            },
            |root| {
                root.expect_file("tests/compile-only/test.typ", "Hello World")
                    .expect_file("tests/ephemeral/test.typ", "Hello World")
                    .expect_file("tests/ephemeral/ref.typ", "Hello\nWorld")
                    .expect_file("tests/persistent/test.typ", "Hello World")
                    .expect_dir("tests/persistent/ref")
            },
        );
    }

    #[test]
    fn test_make_ephemeral() {
        _dev::fs::TempEnv::run(
            setup_all,
            |root| {
                let project = ProjectLegacy::new(root, "tests");
                Test::new(Identifier::new("compile-only").unwrap())
                    .make_ephemeral(&project)
                    .unwrap();

                Test::new(Identifier::new("ephemeral").unwrap())
                    .make_ephemeral(&project)
                    .unwrap();

                Test::new(Identifier::new("persistent").unwrap())
                    .make_ephemeral(&project)
                    .unwrap();
            },
            |root| {
                root.expect_file("tests/compile-only/test.typ", "Hello World")
                    .expect_file("tests/compile-only/ref.typ", "Hello World")
                    .expect_file("tests/ephemeral/test.typ", "Hello World")
                    .expect_file("tests/ephemeral/ref.typ", "Hello World")
                    .expect_file("tests/persistent/test.typ", "Hello World")
                    .expect_file("tests/persistent/ref.typ", "Hello World")
            },
        );
    }

    #[test]
    fn test_make_persistent() {
        _dev::fs::TempEnv::run(
            setup_all,
            |root| {
                let project = ProjectLegacy::new(root, "tests");
                Test::new(Identifier::new("compile-only").unwrap())
                    .make_persistent::<Png, _>(&project, [])
                    .unwrap();

                Test::new(Identifier::new("ephemeral").unwrap())
                    .make_persistent::<Png, _>(&project, [])
                    .unwrap();

                Test::new(Identifier::new("persistent").unwrap())
                    .make_persistent::<Png, _>(&project, [])
                    .unwrap();
            },
            |root| {
                root.expect_file("tests/compile-only/test.typ", "Hello World")
                    .expect_dir("tests/compile-only/ref")
                    .expect_file("tests/ephemeral/test.typ", "Hello World")
                    .expect_dir("tests/ephemeral/ref")
                    .expect_file("tests/persistent/test.typ", "Hello World")
                    .expect_dir("tests/persistent/ref")
            },
        );
    }

    #[test]
    fn test_make_compile_only() {
        _dev::fs::TempEnv::run(
            setup_all,
            |root| {
                let project = ProjectLegacy::new(root, "tests");
                Test::new(Identifier::new("compile-only").unwrap())
                    .make_compile_only(&project)
                    .unwrap();

                Test::new(Identifier::new("ephemeral").unwrap())
                    .make_compile_only(&project)
                    .unwrap();

                Test::new(Identifier::new("persistent").unwrap())
                    .make_compile_only(&project)
                    .unwrap();
            },
            |root| {
                root.expect_file("tests/compile-only/test.typ", "Hello World")
                    .expect_file("tests/ephemeral/test.typ", "Hello World")
                    .expect_file("tests/persistent/test.typ", "Hello World")
            },
        );
    }

    #[test]
    fn test_load_sources() {
        _dev::fs::TempEnv::run(
            |root| {
                root.setup_file("tests/fancy/test.typ", "Hello World")
                    .setup_file("tests/fancy/ref.typ", "Hello\nWorld")
            },
            |root| {
                let project = ProjectLegacy::new(root, "tests");

                let test = Test {
                    id: Identifier::new("fancy").unwrap(),
                    ref_kind: Some(ReferenceKind::Ephemeral),
                    is_ignored: false,
                };

                test.load_test_source(&project).unwrap();
                test.load_ref_source(&project).unwrap().unwrap();
            },
            |root| {
                root.expect_file("tests/fancy/test.typ", "Hello World")
                    .expect_file("tests/fancy/ref.typ", "Hello\nWorld")
            },
        );
    }
}
