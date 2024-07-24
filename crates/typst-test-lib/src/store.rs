//! Project management and test loading.

use std::io;
use std::path::Path;

use ecow::EcoVec;
use tiny_skia::Pixmap;

use crate::render;
use crate::render::Strategy;

pub mod page;
pub mod project;
pub mod test;
pub mod vcs;

/// An error that may occur during saving of a document.
#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    #[error("an io error occurred")]
    Io(#[from] io::Error),

    #[error("a page error occurred")]
    Page(#[from] page::SaveError<page::Png>),
}

/// An error that may occur during loading of a document.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("an io error occurred")]
    Io(#[from] io::Error),

    #[error("a page error occurred")]
    Page(#[from] page::LoadError<page::Png>),
}

/// In-memory representation of a Typst document which can be saved and loaded
/// from disk.
#[derive(Debug, Clone)]
pub struct Document {
    pages: EcoVec<Pixmap>,
}

impl Document {
    /// Creates a new document from the given pages.
    pub fn new<P: Into<EcoVec<Pixmap>>>(pages: P) -> Self {
        Self {
            pages: pages.into(),
        }
    }

    /// Fully renders a typst [`Document`][typst::model::Document].
    pub fn render(document: &typst::model::Document, strategy: Strategy) -> Self {
        Self {
            pages: render::render_document(document, strategy).collect(),
        }
    }

    /// Fully renders the diff of two typst [`Document`][typst::model::Document]s.
    pub fn render_diff(
        base: &typst::model::Document,
        change: &typst::model::Document,
        strategy: Strategy,
    ) -> Self {
        Self {
            pages: render::render_document_diff(base, change, strategy).collect(),
        }
    }

    /// Returns a reference to the pages in this document.
    pub fn pages(&self) -> &[Pixmap] {
        &self.pages
    }

    /// Save this document in the given directory, this will any files that
    /// previously existed with the generated names.
    pub fn save(&self, dir: &Path) -> Result<(), SaveError> {
        page::save_pages::<page::Png>(dir, self.pages.iter())?;

        Ok(())
    }

    /// Load this document's pages from the given directory, this will return
    /// an empty document if no files with the default names exist.
    pub fn load(dir: &Path) -> Result<Self, LoadError> {
        let pages = page::load_pages::<page::Png>(dir)?;

        Ok(Self {
            pages: pages.into(),
        })
    }
}
