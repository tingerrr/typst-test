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

#[derive(Clone)]
pub struct Output {
    document: Document,
    tracer: Tracer,
}

impl Output {
    pub fn new(document: Document, tracer: Tracer) -> Self {
        Self { document, tracer }
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn tracer(&self) -> &Tracer {
        &self.tracer
    }
}

impl Debug for Output {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Output")
            .field("document", &self.document)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Error)]
#[error("compilation failed")]
pub struct Failure {
    errors: EcoVec<SourceDiagnostic>,
    tracer: Tracer,
}

impl Debug for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Failure")
            .field("errors", &self.errors)
            .finish_non_exhaustive()
    }
}

pub fn compile<W: World>(source: Source, world: &W) -> Result<Output, Failure> {
    let world = TestWorld::new(&source, world);
    let mut tracer = Tracer::new();

    match typst::compile(&world, &mut tracer) {
        Ok(document) => Ok(Output { document, tracer }),
        Err(errors) => Err(Failure { errors, tracer }),
    }
}

/// Provides a [`World`] implementation which treats a [`Test`] as main, but otherwise delegates to
/// a global world.
struct TestWorld<'s, 'w, W> {
    source: &'s Source,
    global: &'w W,
}

impl<'s, 'w, W> TestWorld<'s, 'w, W>
where
    W: World,
{
    fn new(source: &'s Source, world: &'w W) -> Self {
        // if let Err(err) = world.source(source.id()) {
        //     panic!("world did not know test source {err:?}");
        // }

        Self {
            source,
            global: world,
        }
    }
}

impl<W> World for TestWorld<'_, '_, W>
where
    W: World,
{
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

        compile(source, &world).unwrap();
    }

    #[test]
    #[should_panic]
    fn test_compile_failure() {
        let world = GlobalTestWorld::default();
        let source = Source::detached("#panic()");

        compile(source, &world).unwrap();
    }
}
