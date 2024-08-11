//! Test loading and on-disk manipulation.

use std::fmt::Debug;
use std::fs::File;
use std::io;
use std::io::Write;

use ecow::{eco_vec, EcoString, EcoVec};
use typst::syntax::{FileId, Source, VirtualPath};

use super::vcs::Vcs;
use crate::store::project::{Resolver, TestTarget};
use crate::store::{Document, LoadError, SaveError};
use crate::test::id::Identifier;
use crate::test::{Annotation, ReferenceKind};

pub mod collector;

/// A test handle for managing on-disk resources.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Test {
    id: Identifier,
    ref_kind: Option<ReferenceKind>,
    annotations: EcoVec<Annotation>,
}

/// References for a test.
#[derive(Debug, Clone)]
pub enum References {
    /// An ephemeral reference script used to compile the reference document on
    /// the fly.
    Ephemeral(EcoString),

    /// Persistent references which are stored on disk.
    Persistent(Document),
}

impl Test {
    /// Generates a new test which does not exist on disk yet.
    pub fn new(id: Identifier) -> Self {
        Self {
            id,
            ref_kind: None,
            annotations: eco_vec![],
        }
    }

    /// Generates a new test which does not exist on disk yet. This is primarily
    /// used in tests outside this module.
    #[doc(hidden)]
    pub fn new_full(
        id: Identifier,
        ref_kind: Option<ReferenceKind>,
        annotations: EcoVec<Annotation>,
    ) -> Self {
        Self {
            id,
            ref_kind,
            annotations,
        }
    }

    /// Returns a reference to the identifier of this test.
    pub fn id(&self) -> &Identifier {
        &self.id
    }

    /// Returns the reference kind of this test, if this test has references.
    pub fn ref_kind(&self) -> Option<ReferenceKind> {
        self.ref_kind
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
        self.ref_kind.is_none()
    }

    /// Returns a reference to this test's annotations.
    pub fn annotations(&self) -> &[Annotation] {
        &self.annotations
    }

    /// Returns whether this test has an ignored annotation.
    pub fn is_ignored(&self) -> bool {
        self.annotations.contains(&Annotation::Ignored)
    }

    /// Creates a new test directly on disk.
    pub fn create(
        resolver: &dyn Resolver,
        vcs: Option<&dyn Vcs>,
        id: Identifier,
        source: &str,
        references: Option<References>,
    ) -> Result<Self, SaveError> {
        let test_dir = resolver.resolve(&id, TestTarget::TestDir);
        typst_test_stdx::fs::create_dir(test_dir, true)?;

        let mut file = File::options()
            .write(true)
            .create_new(true)
            .open(resolver.resolve(&id, TestTarget::TestScript))?;

        file.write_all(source.as_bytes())?;

        let ref_kind = match references {
            Some(References::Ephemeral(_)) => Some(ReferenceKind::Ephemeral),
            Some(References::Persistent(_)) => Some(ReferenceKind::Persistent),
            None => None,
        };

        // TODO: we need to return a proper error here
        let annotations = source
            .lines()
            .take_while(|&l| l.starts_with("///"))
            .filter_map(|l| Annotation::parse_line(l).ok())
            .collect();

        let test = Self {
            id,
            ref_kind,
            annotations,
        };

        match references {
            Some(References::Ephemeral(reference)) => {
                test.create_reference_script(resolver, reference.as_str())?;
            }
            Some(References::Persistent(reference)) => {
                test.create_reference_documents(resolver, &reference)?;
            }
            None => {}
        }

        test.ignore_temporary_directories(resolver, vcs)?;

        Ok(test)
    }

    /// Creates this test's temporary directories, if they don't exist yet.
    pub fn create_temporary_directories(&self, resolver: &dyn Resolver) -> io::Result<()> {
        if self.is_ephemeral() {
            typst_test_stdx::fs::create_dir(resolver.resolve(&self.id, TestTarget::RefDir), true)?;
        }

        typst_test_stdx::fs::create_dir(resolver.resolve(&self.id, TestTarget::OutDir), true)?;
        typst_test_stdx::fs::create_dir(resolver.resolve(&self.id, TestTarget::DiffDir), true)?;
        Ok(())
    }

    /// Creates this test's reference script, this will truncate the file if it
    /// already exists.
    pub fn create_reference_script(
        &self,
        resolver: &dyn Resolver,
        reference: &str,
    ) -> io::Result<()> {
        std::fs::write(resolver.resolve(&self.id, TestTarget::RefScript), reference)?;
        Ok(())
    }

    /// Creates this test's persistent references, this will fail if there are
    /// already pages in the directory.
    pub fn create_reference_documents(
        &self,
        resolver: &dyn Resolver,
        reference: &Document,
    ) -> Result<(), SaveError> {
        let ref_dir = resolver.resolve(&self.id, TestTarget::RefDir);
        typst_test_stdx::fs::create_dir(ref_dir, true)?;
        reference.save(ref_dir)?;
        Ok(())
    }

    /// Deletes this test's directories and scripts, if they exist.
    pub fn delete(&self, resolver: &dyn Resolver) -> io::Result<()> {
        self.delete_reference_documents(resolver)?;
        self.delete_reference_script(resolver)?;
        self.delete_temporary_directories(resolver)?;

        typst_test_stdx::fs::remove_file(resolver.resolve(&self.id, TestTarget::TestScript))?;
        typst_test_stdx::fs::remove_dir(resolver.resolve(&self.id, TestTarget::TestDir), true)?;

        Ok(())
    }

    /// Deletes this test's temporary directories, if they exist.
    pub fn delete_temporary_directories(&self, resolver: &dyn Resolver) -> io::Result<()> {
        if self.is_ephemeral() {
            typst_test_stdx::fs::remove_dir(resolver.resolve(&self.id, TestTarget::RefDir), true)?;
        }

        typst_test_stdx::fs::remove_dir(resolver.resolve(&self.id, TestTarget::OutDir), true)?;
        typst_test_stdx::fs::remove_dir(resolver.resolve(&self.id, TestTarget::DiffDir), true)?;
        Ok(())
    }

    /// Deletes this test's reference script, if it exists.
    pub fn delete_reference_script(&self, resolver: &dyn Resolver) -> io::Result<()> {
        typst_test_stdx::fs::remove_file(resolver.resolve(&self.id, TestTarget::RefScript))?;
        Ok(())
    }

    /// Deletes this test's persistent reference documents, if they exist.
    pub fn delete_reference_documents(&self, resolver: &dyn Resolver) -> io::Result<()> {
        typst_test_stdx::fs::remove_dir(resolver.resolve(&self.id, TestTarget::RefDir), true)?;
        Ok(())
    }

    /// Ignores this test's temporary directories in the vcs.
    pub fn ignore_temporary_directories(
        &self,
        resolver: &dyn Resolver,
        vcs: Option<&dyn Vcs>,
    ) -> io::Result<()> {
        if let Some(vcs) = vcs {
            if self.is_ephemeral() {
                vcs.ignore(resolver, &self.id, TestTarget::RefDir)?;
            }

            vcs.ignore(resolver, &self.id, TestTarget::OutDir)?;
            vcs.ignore(resolver, &self.id, TestTarget::DiffDir)?;
        }

        Ok(())
    }

    /// Ignores this test's persistent reference documents in the vcs.
    pub fn ignore_reference_documents(
        &self,
        resolver: &dyn Resolver,
        vcs: Option<&dyn Vcs>,
    ) -> io::Result<()> {
        if let Some(vcs) = vcs {
            vcs.ignore(resolver, &self.id, TestTarget::RefDir)?;
        }
        Ok(())
    }

    /// Ignores this test's persistent reference documents in the vcs.
    pub fn unignore_reference_documents(
        &self,
        resolver: &dyn Resolver,
        vcs: Option<&dyn Vcs>,
    ) -> io::Result<()> {
        if let Some(vcs) = vcs {
            vcs.unignore(resolver, &self.id, TestTarget::RefDir)?;
        }
        Ok(())
    }

    /// Removes any previous references, if they exist and creates a reference
    /// script by copying the test script.
    pub fn make_ephemeral(
        &mut self,
        resolver: &dyn Resolver,
        vcs: Option<&dyn Vcs>,
    ) -> io::Result<()> {
        self.delete_reference_script(resolver)?;
        self.delete_reference_documents(resolver)?;
        self.ignore_reference_documents(resolver, vcs)?;

        std::fs::copy(
            resolver.resolve(&self.id, TestTarget::TestScript),
            resolver.resolve(&self.id, TestTarget::RefScript),
        )?;

        self.ref_kind = Some(ReferenceKind::Ephemeral);
        Ok(())
    }

    /// Removes any previous references, if they exist and creates persistent
    /// references from the given pages.
    pub fn make_persistent(
        &mut self,
        resolver: &dyn Resolver,
        vcs: Option<&dyn Vcs>,
        reference: &Document,
    ) -> Result<(), SaveError> {
        self.delete_reference_script(resolver)?;
        self.delete_reference_documents(resolver)?;
        self.create_reference_documents(resolver, reference)?;
        self.unignore_reference_documents(resolver, vcs)?;

        self.ref_kind = Some(ReferenceKind::Persistent);
        Ok(())
    }

    /// Removes any previous references, if they exist.
    pub fn make_compile_only(
        &mut self,
        resolver: &dyn Resolver,
        vcs: Option<&dyn Vcs>,
    ) -> io::Result<()> {
        self.delete_reference_documents(resolver)?;
        self.delete_reference_script(resolver)?;
        self.ignore_reference_documents(resolver, vcs)?;

        self.ref_kind = None;
        Ok(())
    }

    /// Loads the test script source of this test.
    pub fn load_source(&self, resolver: &dyn Resolver) -> io::Result<Source> {
        let test_script = resolver.resolve(&self.id, TestTarget::TestScript);

        Ok(Source::new(
            FileId::new(
                None,
                VirtualPath::new(
                    test_script
                        .strip_prefix(resolver.project_root())
                        .unwrap_or(test_script),
                ),
            ),
            std::fs::read_to_string(test_script)?,
        ))
    }

    /// Loads the reference test script source of this test, if this test is
    /// ephemeral.
    pub fn load_reference_source(&self, resolver: &dyn Resolver) -> io::Result<Option<Source>> {
        match self.ref_kind {
            Some(ReferenceKind::Ephemeral) => {
                let ref_script = resolver.resolve(&self.id, TestTarget::RefScript);
                Ok(Some(Source::new(
                    FileId::new(
                        None,
                        VirtualPath::new(
                            ref_script
                                .strip_prefix(resolver.project_root())
                                .unwrap_or(ref_script),
                        ),
                    ),
                    std::fs::read_to_string(ref_script)?,
                )))
            }
            _ => Ok(None),
        }
    }

    /// Loads the persistent reference pages of this test, if they exist.
    pub fn load_reference_documents(
        &self,
        resolver: &dyn Resolver,
    ) -> Result<Option<Document>, LoadError> {
        match self.ref_kind {
            Some(ReferenceKind::Persistent) => {
                Document::load(resolver.resolve(&self.id, TestTarget::RefDir)).map(Some)
            }
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::_dev;
    use crate::_dev::fs::Setup;
    use crate::store::project::v1::ResolverV1;

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
                let project = ResolverV1::new(root, "tests");
                Test::create(
                    &project,
                    None,
                    Identifier::new("compile-only").unwrap(),
                    "Hello World",
                    None,
                )
                .unwrap();

                Test::create(
                    &project,
                    None,
                    Identifier::new("ephemeral").unwrap(),
                    "Hello World",
                    Some(References::Ephemeral("Hello\nWorld".into())),
                )
                .unwrap();

                Test::create(
                    &project,
                    None,
                    Identifier::new("persistent").unwrap(),
                    "Hello World",
                    Some(References::Persistent(Document::new(vec![]))),
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
                let project = ResolverV1::new(root, "tests");
                Test::new(Identifier::new("compile-only").unwrap())
                    .make_ephemeral(&project, None)
                    .unwrap();

                Test::new(Identifier::new("ephemeral").unwrap())
                    .make_ephemeral(&project, None)
                    .unwrap();

                Test::new(Identifier::new("persistent").unwrap())
                    .make_ephemeral(&project, None)
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
                let project = ResolverV1::new(root, "tests");
                Test::new(Identifier::new("compile-only").unwrap())
                    .make_persistent(&project, None, &Document::new(vec![]))
                    .unwrap();

                Test::new(Identifier::new("ephemeral").unwrap())
                    .make_persistent(&project, None, &Document::new(vec![]))
                    .unwrap();

                Test::new(Identifier::new("persistent").unwrap())
                    .make_persistent(&project, None, &Document::new(vec![]))
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
                let project = ResolverV1::new(root, "tests");
                Test::new(Identifier::new("compile-only").unwrap())
                    .make_compile_only(&project, None)
                    .unwrap();

                Test::new(Identifier::new("ephemeral").unwrap())
                    .make_compile_only(&project, None)
                    .unwrap();

                Test::new(Identifier::new("persistent").unwrap())
                    .make_compile_only(&project, None)
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
        _dev::fs::TempEnv::run_no_check(
            |root| {
                root.setup_file("tests/fancy/test.typ", "Hello World")
                    .setup_file("tests/fancy/ref.typ", "Hello\nWorld")
            },
            |root| {
                let project = ResolverV1::new(root, "tests");

                let test = Test {
                    id: Identifier::new("fancy").unwrap(),
                    ref_kind: Some(ReferenceKind::Ephemeral),
                    annotations: eco_vec![],
                };

                test.load_source(&project).unwrap();
                test.load_reference_source(&project).unwrap().unwrap();
            },
        );
    }

    #[test]
    fn test_sources_virtual() {
        _dev::fs::TempEnv::run_no_check(
            |root| root.setup_file_empty("tests/fancy/test.typ"),
            |root| {
                let project = ResolverV1::new(root, "tests");

                let test = Test {
                    id: Identifier::new("fancy").unwrap(),
                    ref_kind: None,
                    annotations: eco_vec![],
                };

                let source = test.load_source(&project).unwrap();
                assert_eq!(
                    source.id().vpath().resolve(root).unwrap(),
                    root.join("tests/fancy/test.typ")
                );
            },
        );
    }
}
