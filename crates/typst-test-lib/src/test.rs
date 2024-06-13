use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use id::{Identifier, IdentifierError};
use thiserror::Error;

pub mod id;

pub const REF_NAME: &str = "ref";
pub const TEST_NAME: &str = "test";
pub const OUT_NAME: &str = "out";
pub const DIFF_NAME: &str = "diff";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Test {
    pub name: Identifier,
    pub reference: Option<ReferenceKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReferenceKind {
    Ephemeral,
    Persistent,
}

#[derive(Debug, Error)]
pub enum CollectError {
    #[error("an error occured while traversing directories")]
    WalkDir(#[from] ignore::Error),

    #[error("an io error occurred")]
    Io(#[from] io::Error),

    #[error("an error occured while collecting a test")]
    Test(#[from] IdentifierError),
}

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

        let id = Identifier::from_path(entry.path().strip_prefix(test_root).unwrap())?;

        tests.insert(
            id.clone(),
            Test {
                name: id,
                reference,
            },
        );
    }

    Ok(tests)
}

#[cfg(test)]
mod tests {
    use typst::eval::Tracer;
    use typst::syntax::{FileId, Source, VirtualPath};

    use super::*;
    use crate::compile::Metrics;
    use crate::{compare, compile, render};

    #[test]
    fn test_full_ephemeral_pass() {
        let src_path = "../../assets/test-assets/test/ephemeral-src.typ";
        let ref_path = "../../assets/test-assets/test/ephemeral-ref.typ";

        let source = Source::new(
            FileId::new(None, VirtualPath::new(src_path)),
            std::fs::read_to_string(src_path).unwrap(),
        );

        let reference = Source::new(
            FileId::new(None, VirtualPath::new(ref_path)),
            std::fs::read_to_string(ref_path).unwrap(),
        );

        let world = crate::_dev::GlobalTestWorld::default();

        let output = compile::in_memory::compile(
            source.clone(),
            &world,
            &mut Tracer::new(),
            &mut Metrics::new(),
        )
        .unwrap();

        let reference = compile::in_memory::compile(
            reference.clone(),
            &world,
            &mut Tracer::new(),
            &mut Metrics::new(),
        )
        .unwrap();

        let output = render::render_document(&output, render::Strategy::default());
        let reference = render::render_document(&reference, render::Strategy::default());

        compare::visual::compare_pages(
            output,
            reference,
            compare::visual::Strategy::default(),
            false,
        )
        .unwrap();
    }

    #[test]
    fn test_collect() {
        let tests = [
            ("compare/ephemeral", Some(ReferenceKind::Ephemeral)),
            ("compare/persistent", Some(ReferenceKind::Persistent)),
            ("compile", None),
        ]
        .map(|(name, reference)| Test {
            name: Identifier::new(name).unwrap(),
            reference,
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
