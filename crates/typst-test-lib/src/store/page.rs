//! Management of page based reference documents.

use std::collections::BTreeMap;
use std::fmt::Debug;
use std::io;
use std::path::Path;

use thiserror::Error;
use tiny_skia::Pixmap;

/// A format which saves multiple pages of a document as individual files.
pub trait PageFormat {
    /// The error type returned when loading a page fails.
    type LoadError: std::error::Error + 'static;

    /// The error type returned when saving a page fails.
    type SaveError: std::error::Error + 'static;

    /// The type returned after the format is fully loaded.
    type Type: Debug;

    /// The extension used for this format.
    const EXTENSION: &'static str;

    /// Loads a single page from the given path.
    fn load_page(path: &Path) -> Result<Self::Type, Self::LoadError>;

    /// Saves a single page at the given path.
    fn save_page(value: &Self::Type, path: &Path) -> Result<(), Self::SaveError>;
}

/// The PNG file format used for storing rendered apges for visual comparison.
pub struct Png(());

impl PageFormat for Png {
    type LoadError = png::DecodingError;
    type SaveError = png::EncodingError;

    type Type = Pixmap;

    const EXTENSION: &'static str = "png";

    fn load_page(path: &Path) -> Result<Self::Type, Self::LoadError> {
        Pixmap::load_png(path)
    }

    fn save_page(value: &Self::Type, path: &Path) -> Result<(), Self::SaveError> {
        value.save_png(path)
    }
}

/// A generic loading error for a format.
#[derive(Error)]
pub enum LoadError<F: PageFormat> {
    #[error("an io error occured")]
    Io(#[from] io::Error),

    #[error("a page could not be loaded")]
    Format(#[source] F::LoadError),
}

impl<F: PageFormat> Debug for LoadError<F>
where
    F::LoadError: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(inner) => f.debug_tuple("Io").field(inner).finish(),
            Self::Format(inner) => f.debug_tuple("Format").field(inner).finish(),
        }
    }
}

/// A generic saving error for a format.
#[derive(Error)]
pub enum SaveError<F: PageFormat> {
    #[error("an io error occured")]
    Io(#[from] io::Error),

    #[error("a page could not be saved")]
    Format(#[source] F::SaveError),
}

impl<F: PageFormat> Debug for SaveError<F>
where
    F::SaveError: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(inner) => f.debug_tuple("Io").field(inner).finish(),
            Self::Format(inner) => f.debug_tuple("Format").field(inner).finish(),
        }
    }
}

/// Counts all pages in the given direcory of a given format. Any files which do
/// not have ascii numerals are their file name and the format extension are
/// ignored.
pub fn count_pages<F: PageFormat>(dir: &Path) -> Result<usize, LoadError<F>> {
    let mut count = 0;

    load_pages_internal(dir, |_, _| -> Result<_, F::LoadError> {
        count += 1;
        Ok(())
    })?;

    Ok(count)
}

/// Loads all pages in the given direcory of a given format. Any files which do
/// not have ascii numerals are their file name and the format extension are
/// ignored.
pub fn load_pages<F: PageFormat>(dir: &Path) -> Result<Vec<F::Type>, LoadError<F>> {
    let mut values = BTreeMap::new();

    load_pages_internal(dir, |path, page| -> Result<_, F::LoadError> {
        values.insert(page, F::load_page(path)?);
        Ok(())
    })?;

    Ok(values.into_values().collect())
}

fn load_pages_internal<F: PageFormat>(
    dir: &Path,
    mut loader: impl FnMut(&Path, usize) -> Result<(), F::LoadError>,
) -> Result<(), LoadError<F>> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;

        let name = entry.file_name();
        if !entry.file_type()?.is_file() {
            tracing::trace!(entry = ?name, "ignoring non-file entry");
            continue;
        }

        let path = entry.path();
        let Some(page) = path
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.parse().ok())
        else {
            tracing::trace!(entry = ?name, "ignoring non-numeric filename");
            continue;
        };

        if path.extension().is_some_and(|ext| ext == F::EXTENSION) {
            loader(&path, page).map_err(LoadError::Format)?;
        }
    }

    Ok(())
}

/// Loads all pages in the given direcory in the given format. The file names
/// for the indiviual pages are their 1-based index without any 0-padding.
pub fn save_pages<'p, F>(
    dir: &Path,
    pages: impl IntoIterator<Item = &'p F::Type>,
) -> Result<(), SaveError<F>>
where
    F: PageFormat,
    F::Type: 'p,
{
    for (idx, page) in pages
        .into_iter()
        .enumerate()
        .map(|(idx, page)| (idx + 1, page))
    {
        let path = dir.join(idx.to_string()).with_extension(F::EXTENSION);
        F::save_page(page, &path).map_err(SaveError::Format)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use tempdir::TempDir;
    use tiny_skia::PremultipliedColorU8;

    use super::*;
    use crate::_dev;

    fn create_pixmap() -> Pixmap {
        let mut page = Pixmap::new(500, 500).unwrap();

        for y in 200..=300 {
            for x in 200..=300 {
                page.pixels_mut()[y * 500 + x] =
                    PremultipliedColorU8::from_rgba(255, 255, 255, 255).unwrap();
            }
        }

        page
    }

    #[test]
    fn test_store_png() {
        let pixmaps = [create_pixmap(), create_pixmap(), create_pixmap()];

        _dev::fs::TempEnv::run(
            |root| root,
            |root| {
                save_pages::<Png>(root, &pixmaps).unwrap();
            },
            |root| {
                root.expect_file("1.png", pixmaps[0].encode_png().unwrap())
                    .expect_file("2.png", pixmaps[1].encode_png().unwrap())
                    .expect_file("3.png", pixmaps[2].encode_png().unwrap())
            },
        );
    }

    #[test]
    fn test_load_png() {
        let root = TempDir::new("typst-test").unwrap();

        let pixmaps = [create_pixmap(), create_pixmap(), create_pixmap()];
        save_pages::<Png>(root.path(), &pixmaps).unwrap();

        let pages = load_pages::<Png>(root.path()).unwrap();

        assert_eq!(pages.len(), 3);

        for (a, b) in pages[0].pixels().iter().zip(create_pixmap().pixels()) {
            assert_eq!(a, b);
        }
    }

    #[test]
    fn test_count_png() {
        let root = TempDir::new("typst-test").unwrap();

        let pixmaps = [create_pixmap(), create_pixmap(), create_pixmap()];
        save_pages::<Png>(root.path(), &pixmaps).unwrap();

        assert_eq!(count_pages::<Png>(root.path()).unwrap(), 3);
    }
}
