//! Wrappers around [`typst::compile`] for easier error handling.

use std::fmt::Debug;

use comemo::Prehashed;
use ecow::EcoVec;
use thiserror::Error;
use typst::diag::{FileResult, SourceDiagnostic};
use typst::eval::Tracer;
use typst::foundations::{Bytes, Datetime};
use typst::model::Document;
use typst::syntax::{FileId, Source};
use typst::text::{Font, FontBook};
use typst::{Library, World};

use crate::util;

/// An error which may occur during compilation. This struct only exists to
/// implement [`Error`][trait@std::error::Error].
#[derive(Debug, Clone, Error)]
#[error("compilation failed with {} {}", .0.len(), util::fmt::plural(.0.len(), "error"))]
pub struct Error(pub EcoVec<SourceDiagnostic>);

/// Compiles a source with the given global world.
pub fn compile(source: Source, world: &dyn World, tracer: &mut Tracer) -> Result<Document, Error> {
    typst::compile(&TestWorld::new(&source, world), tracer).map_err(Error)
}

/// Provides a [`World`] implementation which treats a [`Test`] as main, but
/// otherwise delegates to a global world.
struct TestWorld<'s, 'w> {
    source: &'s Source,
    global: &'w dyn World,
}

impl<'s, 'w> TestWorld<'s, 'w> {
    fn new(source: &'s Source, world: &'w dyn World) -> Self {
        Self {
            source,
            global: world,
        }
    }
}

impl World for TestWorld<'_, '_> {
    fn library(&self) -> &Prehashed<Library> {
        self.global.library()
    }

    fn book(&self) -> &Prehashed<FontBook> {
        self.global.book()
    }

    fn main(&self) -> Source {
        self.source.clone()
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        self.global.source(id)
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

// TODO: better tests
#[cfg(test)]
mod tests {
    use super::*;
    use crate::_dev::GlobalTestWorld;

    #[test]
    fn test_compile() {
        let world = GlobalTestWorld::default();
        let source = Source::detached("Hello World");

        compile(source, &world, &mut Tracer::new()).unwrap();
    }

    #[test]
    #[should_panic]
    fn test_compile_failure() {
        let world = GlobalTestWorld::default();
        let source = Source::detached("#panic()");

        compile(source, &world, &mut Tracer::new()).unwrap();
    }
}
