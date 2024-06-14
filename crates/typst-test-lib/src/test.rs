use std::fmt::Debug;
use std::io;

use id::Identifier;
use typst::syntax::{FileId, Source, VirtualPath};

use crate::store::page::{LoadError, PageFormat, SaveError};
use crate::store::project::{Project, TestTarget};
use crate::{store, util};

pub mod collector;
pub mod id;
pub mod matcher;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Test {
    id: Identifier,
    ref_kind: Option<ReferenceKind>,
    is_ignored: bool,
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

    /// Deletes this test's directories and scripts.
    pub fn delete_test<P: Project>(self, project: &P) -> io::Result<()> {
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
        self.delete_reference_pages(project)?;
        self.delete_reference_script(project)?;

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
        // TODO: the error handling is slightly wrong here
        // the upper io error should not be converted into a SaveError<F>::Io
        self.delete_reference_pages(project)?;
        store::page::save_pages::<F>(project.resolve(&self.id, TestTarget::RefDir), pages)?;

        self.ref_kind = Some(ReferenceKind::Persistent);
        Ok(())
    }

    /// Removes any previous references.
    pub fn make_compile_only<F: PageFormat, P: Project>(&mut self, project: &P) -> io::Result<()> {
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
    use collector::Collector;
    use store::project::v1::ProjectV1;
    use typst::eval::Tracer;

    use super::*;
    use crate::_dev::GlobalTestWorld;
    use crate::compile::Metrics;
    use crate::store::page::Png;
    use crate::{compare, compile, render};

    #[test]
    fn test_e2e() {
        let world = GlobalTestWorld::default();
        let project = ProjectV1::new("../../assets/test-assets/collect/");

        // taken from typst-cli which generated the persistent ref iamges
        let ppi = 144.0;
        let strategy = render::Strategy::Raster {
            pixel_per_pt: ppi / 72.0,
        };

        let mut collector = Collector::new(&project);
        collector.collect();

        for test in collector.tests().values() {
            let source = test.load_test_source(&project).unwrap();
            let output = compile::compile(
                source.clone(),
                &world,
                &mut Tracer::new(),
                &mut Metrics::new(),
            )
            .unwrap();

            if test.is_compile_only() {
                continue;
            }

            let output = render::render_document(&output, strategy);

            let reference = if let Some(reference) = test.load_ref_source(&project).unwrap() {
                let reference = compile::compile(
                    reference.clone(),
                    &world,
                    &mut Tracer::new(),
                    &mut Metrics::new(),
                )
                .unwrap();

                render::render_document(&reference, strategy).collect()
            } else if let Some(pages) = test.load_ref_pages::<Png, _>(&project).unwrap() {
                pages
            } else {
                panic!()
            };

            compare::visual::compare_pages(
                output,
                reference.into_iter(),
                compare::visual::Strategy::default(),
                false,
            )
            .unwrap();
        }
    }

    #[test]
    fn test_load_sources() {
        let project = ProjectV1::new("../../assets/test-assets/");

        let test = Test {
            id: Identifier::new("collect/compare/ephemeral-no-store").unwrap(),
            ref_kind: Some(ReferenceKind::Ephemeral),
            is_ignored: false,
        };

        test.load_test_source(&project).unwrap();
        test.load_ref_source(&project).unwrap().unwrap();
    }
}
