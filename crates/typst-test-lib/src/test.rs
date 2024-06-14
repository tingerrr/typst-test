use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use id::{Identifier, ParseIdentifierError};
use thiserror::Error;
use typst::syntax::{FileId, Source, VirtualPath};

use crate::store::page::{LoadError, PageFormat};
use crate::store::{self};

pub mod id;

/// The name of the persistent reference store directory or ephemeral test script.
pub const REF_NAME: &str = "ref";

/// The name of the test script.
pub const TEST_NAME: &str = "test";

/// The name of the ephemeral output directory.
pub const OUT_NAME: &str = "out";

/// The name of the ephemeral diff directory.
pub const DIFF_NAME: &str = "diff";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Test {
    id: Identifier,
    ref_kind: Option<ReferenceKind>,
}

// TODO: in code construction of tests for saving
impl Test {
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

    /// Loads the test script source of this test.
    pub fn load_test_source<P: AsRef<Path>>(&self, test_root: P) -> io::Result<Source> {
        let path = self.id.to_path().join(TEST_NAME).with_extension("typ");
        let path = test_root.as_ref().join(path);
        Ok(Source::new(
            FileId::new(None, VirtualPath::new(&path)),
            std::fs::read_to_string(&path)?,
        ))
    }

    /// Loads the reference test script source of this test, if one exists.
    pub fn load_ref_source<P: AsRef<Path>>(&self, test_root: P) -> io::Result<Option<Source>> {
        match self.ref_kind {
            Some(ReferenceKind::Ephemeral) => {
                let path = self.id.to_path().join(REF_NAME).with_extension("typ");
                let path = test_root.as_ref().join(path);
                Ok(Some(Source::new(
                    FileId::new(None, VirtualPath::new(&path)),
                    std::fs::read_to_string(&path)?,
                )))
            }
            _ => Ok(None),
        }
    }

    /// Loads the persistent reference pages of this test, if they exist.
    pub fn load_ref_pages<F: PageFormat, P: AsRef<Path>>(
        &self,
        test_root: P,
    ) -> Result<Option<Vec<F::Type>>, LoadError<F>> {
        match self.ref_kind {
            Some(ReferenceKind::Persistent) => {
                let path = self.id.to_path().join("ref");
                let path = test_root.as_ref().join(path);
                store::page::load_pages(&path).map(Some)
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

/// An error that can occur during [`Test`] collection. Returned by [`collect`].
#[derive(Debug, Error)]
pub enum CollectError {
    /// An error occured while traversing directories.
    #[error("an error occured while traversing directories")]
    WalkDir(#[from] ignore::Error),

    /// An io error occurred.
    #[error("an io error occurred")]
    Io(#[from] io::Error),

    /// An error occured while collecting a test.
    #[error("an error occured while collecting a test")]
    Test(#[from] ParseIdentifierError),
}

/// Collects tests from the test root path, this is the top level directory
/// containing all tests and additional helper and template scripts.
///
/// This function will walk the path recursively and respect ignore files.
pub fn collect<P: AsRef<Path>>(test_root: P) -> Result<BTreeMap<Identifier, Test>, CollectError> {
    let test_root = test_root.as_ref();
    let mut tests = BTreeMap::new();

    for entry in ignore::WalkBuilder::new(test_root)
        .filter_entry(|entry| {
            entry.file_type().is_some_and(|file_type| {
                file_type.is_dir() && Identifier::RESERVED.iter().all(|&r| entry.file_name() != r)
            })
        })
        .build()
    {
        let entry = entry?;

        let test_path = entry.path().join(TEST_NAME).with_extension("typ");
        if !test_path.try_exists()? {
            continue;
        }

        let mut ref_path = entry.path().join(REF_NAME);

        let reference = if ref_path.try_exists()? {
            Some(ReferenceKind::Persistent)
        } else {
            ref_path.set_extension("typ");
            if ref_path.try_exists()? {
                Some(ReferenceKind::Ephemeral)
            } else {
                None
            }
        };

        let id = Identifier::from_path(
            entry
                .path()
                .strip_prefix(test_root)
                .expect("must be within test_root"),
        )?;

        tests.insert(
            id.clone(),
            Test {
                id,
                ref_kind: reference,
            },
        );
    }

    Ok(tests)
}

#[cfg(test)]
mod tests {
    use typst::eval::Tracer;

    use super::*;
    use crate::_dev::GlobalTestWorld;
    use crate::compile::Metrics;
    use crate::store::page::Png;
    use crate::{compare, compile, render};

    #[test]
    fn test_e2e() {
        let world = GlobalTestWorld::default();
        let root = "../../assets/test-assets/collect/";

        // taken from typst-cli which generated the persistent ref iamges
        let ppi = 144.0;
        let strategy = render::Strategy::Raster {
            pixel_per_pt: ppi / 72.0,
        };

        let tests = collect(root).unwrap();
        for test in tests.values() {
            let source = test.load_test_source(root).unwrap();
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

            let reference = if let Some(reference) = test.load_ref_source(root).unwrap() {
                let reference = compile::compile(
                    reference.clone(),
                    &world,
                    &mut Tracer::new(),
                    &mut Metrics::new(),
                )
                .unwrap();

                render::render_document(&reference, strategy).collect()
            } else if let Some(pages) = test.load_ref_pages::<Png, _>(root).unwrap() {
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
        let root = "../../assets/test-assets/";

        let test = Test {
            id: Identifier::from_path("collect/compare/ephemeral").unwrap(),
            ref_kind: Some(ReferenceKind::Ephemeral),
        };

        test.load_test_source(root).unwrap();
        test.load_ref_source(root).unwrap().unwrap();
    }

    #[test]
    fn test_collect() {
        let tests = [
            ("compare/ephemeral", Some(ReferenceKind::Ephemeral)),
            ("compare/persistent", Some(ReferenceKind::Persistent)),
            ("compile", None),
        ]
        .map(|(name, reference)| Test {
            id: Identifier::new(name).unwrap(),
            ref_kind: reference,
        });

        assert_eq!(
            collect("../../assets/test-assets/collect")
                .unwrap()
                .into_values()
                .collect::<Vec<_>>(),
            tests,
        );
    }
}
