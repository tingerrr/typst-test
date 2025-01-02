//! Wrappers around [`typst::compile`] for easier error handling.

use std::fmt::Debug;

use ecow::EcoVec;
use thiserror::Error;
use typst::diag::{FileResult, SourceDiagnostic, Warned};
use typst::foundations::{Bytes, Datetime};
use typst::model::Document;
use typst::syntax::{FileId, Source};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, World};

use crate::stdx::fmt::Term;

/// An error which may occur during compilation. This struct only exists to
/// implement [`Error`][trait@std::error::Error].
#[derive(Debug, Clone, Error)]
#[error("compilation failed with {} {}", .0.len(), Term::simple("error").with(.0.len()))]
pub struct Error(pub EcoVec<SourceDiagnostic>);

/// Compiles a source with the given global world.
pub fn compile(source: Source, world: &dyn World) -> Warned<Result<Document, Error>> {
    struct TestWorldAdapter<'s, 'w> {
        source: &'s Source,
        global: &'w dyn World,
    }

    impl World for TestWorldAdapter<'_, '_> {
        fn library(&self) -> &LazyHash<Library> {
            self.global.library()
        }

        fn book(&self) -> &LazyHash<FontBook> {
            self.global.book()
        }

        fn main(&self) -> FileId {
            self.source.id()
        }

        fn source(&self, id: FileId) -> FileResult<Source> {
            if id == self.source.id() {
                Ok(self.source.clone())
            } else {
                self.global.source(id)
            }
        }

        fn file(&self, id: FileId) -> FileResult<Bytes> {
            self.global.file(id)
        }

        fn font(&self, index: usize) -> Option<Font> {
            self.global.font(index)
        }

        fn today(&self, offset: Option<i64>) -> Option<Datetime> {
            self.global.today(offset)
        }
    }

    let Warned { output, warnings } = typst::compile(&TestWorldAdapter {
        source: &source,
        global: world,
    });

    Warned {
        output: output.map_err(Error),
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::_dev::GlobalTestWorld;

    #[test]
    fn test_compile() {
        let world = GlobalTestWorld::default();
        let source = Source::detached("Hello World");

        compile(source, &world).output.unwrap();
    }

    #[test]
    #[should_panic]
    fn test_compile_failure() {
        let world = GlobalTestWorld::default();
        let source = Source::detached("#panic()");

        compile(source, &world).output.unwrap();
    }
}
