//! On-disk management of reference documents reeference documents are stored as
//! individual pages in PNG format.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::{fs, io, iter};

use ecow::EcoVec;
use thiserror::Error;
use tiny_skia::Pixmap;
use typst::diag::Warned;
use typst::model::Document as TypstDocument;
use typst::syntax::Source;
use typst::World;

use self::compare::Strategy;
use self::render::Origin;

pub mod compare;
pub mod compile;
pub mod render;

/// The extension used in the page storage, each page is stored separately with it.
pub const PAGE_EXTENSION: &str = "png";

/// A document that was rendered from an in-memory compilation, or loaded from disk.
#[derive(Debug, Clone)]
pub struct Document {
    doc: Option<TypstDocument>,
    buffers: EcoVec<Pixmap>,
}

impl Document {
    /// Creates a new document from the given buffers.
    pub fn new<I: IntoIterator<Item = Pixmap>>(buffers: I) -> Self {
        Self {
            doc: None,
            buffers: buffers.into_iter().collect(),
        }
    }

    /// Compiles and renders a new document from the given source.
    pub fn compile(
        source: Source,
        world: &dyn World,
        pixel_per_pt: f32,
    ) -> Warned<Result<Self, compile::Error>> {
        let Warned { output, warnings } = compile::compile(source, world);

        Warned {
            output: output.map(|doc| Self::render(doc, pixel_per_pt)),
            warnings,
        }
    }

    /// Creates a new rendered document from a compiled one.
    pub fn render(doc: TypstDocument, pixel_per_pt: f32) -> Self {
        let buffers = doc
            .pages
            .iter()
            .map(|page| typst_render::render(page, pixel_per_pt))
            .collect();

        Self {
            doc: Some(doc),
            buffers,
        }
    }

    /// Renders a diff from the given documents pixel buffers, the resulting new
    /// document will have no inner document set because it was created only
    /// from pixel buffers.
    ///
    /// Diff images are created pair-wise in order using [`render::page_diff`].
    pub fn render_diff(base: &Self, change: &Self, origin: Origin) -> Self {
        let buffers = iter::zip(&base.buffers, &change.buffers)
            .map(|(base, change)| render::page_diff(base, change, origin))
            .collect();

        Self { doc: None, buffers }
    }

    /// Collects the reference document in the given directory.
    pub fn load<P: AsRef<Path>>(dir: P) -> Result<Self, LoadError> {
        let mut buffers = BTreeMap::new();

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if !entry.file_type()?.is_file() {
                tracing::trace!(entry = ?path, "ignoring non-file entry in reference directory");
                continue;
            }

            if path.extension().is_none()
                || path.extension().is_some_and(|ext| ext != PAGE_EXTENSION)
            {
                tracing::trace!(entry = ?path, "ignoring non-PNG entry in reference directory");
                continue;
            }

            let Some(page) = path
                .file_stem()
                .and_then(|s| s.to_str())
                .and_then(|s| s.parse().ok())
                .filter(|&num| num != 0)
            else {
                tracing::trace!(
                    entry = ?path,
                    "ignoring non-numeric or invalid filename in reference directory",
                );
                continue;
            };

            buffers.insert(page, Pixmap::load_png(path)?);
        }

        // check we got pages starting at 1
        match buffers.first_key_value() {
            Some((min, _)) if *min != 1 => {
                return Err(LoadError::MissingPages(buffers.into_keys().collect()));
            }
            Some(_) => {}
            None => {
                return Err(LoadError::MissingPages(buffers.into_keys().collect()));
            }
        }

        // check we got pages ending in the page count
        match buffers.last_key_value() {
            Some((max, _)) if *max != buffers.len() => {
                return Err(LoadError::MissingPages(buffers.into_keys().collect()));
            }
            Some(_) => {}
            None => {
                return Err(LoadError::MissingPages(buffers.into_keys().collect()));
            }
        }

        Ok(Self {
            doc: None,
            // NOTE(tinger): the pages are ordered by key and must not have any
            // page keys missing
            buffers: buffers.into_values().collect(),
        })
    }

    /// Saves a single page within the given directory with the given 1-based page
    /// number.
    ///
    /// # Panics
    /// Panics if `num == 0`.
    pub fn save<P: AsRef<Path>>(&self, dir: P) -> Result<(), SaveError> {
        for (num, page) in self
            .buffers
            .iter()
            .enumerate()
            .map(|(idx, page)| (idx + 1, page))
        {
            page.save_png(
                dir.as_ref()
                    .join(num.to_string())
                    .with_extension(PAGE_EXTENSION),
            )?;
        }

        Ok(())
    }
}

impl Document {
    /// The inner document if this was created from an in-mmeory compilation.
    pub fn doc(&self) -> Option<&TypstDocument> {
        self.doc.as_ref()
    }

    /// The pixel buffers of the rendered pages in this document.
    pub fn buffers(&self) -> &[Pixmap] {
        &self.buffers
    }
}

impl Document {
    /// Compares two documents using the given strategy. May not return all
    /// errors if `fail_fast == true`.
    ///
    /// Comparisons are created pair-wise in order using [`compare::page`].
    pub fn compare(
        outputs: Self,
        references: Self,
        strategy: Strategy,
        fail_fast: bool,
    ) -> Result<(), compare::Error> {
        let output_len = outputs.buffers.len();
        let reference_len = references.buffers.len();

        let max_cap = Ord::min(output_len, reference_len);

        let mut page_errors = if !fail_fast || max_cap <= 32 {
            Vec::with_capacity(max_cap)
        } else {
            vec![]
        };

        for (idx, (a, b)) in iter::zip(&outputs.buffers, &references.buffers).enumerate() {
            if let Err(err) = compare::page(a, b, strategy) {
                page_errors.push((idx, err));

                if fail_fast {
                    break;
                }
            }
        }

        if !page_errors.is_empty() || output_len != reference_len {
            page_errors.shrink_to_fit();
            return Err(compare::Error {
                output: output_len,
                reference: reference_len,
                pages: page_errors,
            });
        }

        Ok(())
    }
}
/// Returned by [`Document::load`].
#[derive(Debug, Error)]
pub enum LoadError {
    /// One or more pages were missing, contains the physical page numbers which
    /// were found.
    #[error("one or more pages were missing, found: {0:?}")]
    MissingPages(BTreeSet<usize>),

    /// A page could not be decoded.
    #[error("a page could not be decoded")]
    Page(#[from] png::DecodingError),

    /// An io error occurred.
    #[error("an io error occurred")]
    Io(#[from] io::Error),
}

/// Returned by [`Document::save`].
#[derive(Debug, Error)]
pub enum SaveError {
    /// A page could not be encoded.
    #[error("a page could not be encoded")]
    Page(#[from] png::EncodingError),

    /// An io error occurred.
    #[error("an io error occurred")]
    Io(#[from] io::Error),
}

#[cfg(test)]
mod tests {
    use ecow::eco_vec;

    use super::*;
    use crate::_dev;

    #[test]
    fn test_document_save() {
        let doc = Document {
            doc: None,
            buffers: eco_vec![Pixmap::new(10, 10).unwrap(); 3],
        };

        _dev::fs::TempEnv::run(
            |root| root,
            |root| {
                doc.save(root).unwrap();
            },
            |root| {
                root.expect_file("1.png", doc.buffers[0].encode_png().unwrap())
                    .expect_file("2.png", doc.buffers[1].encode_png().unwrap())
                    .expect_file("3.png", doc.buffers[2].encode_png().unwrap())
            },
        );
    }

    #[test]
    fn test_document_load() {
        let buffers = eco_vec![Pixmap::new(10, 10).unwrap(); 3];

        _dev::fs::TempEnv::run_no_check(
            |root| {
                root.setup_file("1.png", buffers[0].encode_png().unwrap())
                    .setup_file("2.png", buffers[1].encode_png().unwrap())
                    .setup_file("3.png", buffers[2].encode_png().unwrap())
            },
            |root| {
                let doc = Document::load(root).unwrap();

                assert_eq!(doc.buffers[0], buffers[0]);
                assert_eq!(doc.buffers[1], buffers[1]);
                assert_eq!(doc.buffers[2], buffers[2]);
            },
        );
    }
}
