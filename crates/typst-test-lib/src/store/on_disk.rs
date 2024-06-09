use std::collections::BTreeMap;
use std::fmt::Debug;
use std::io;
use std::path::Path;

use thiserror::Error;
use tiny_skia::Pixmap;

pub trait DiskFormat: Sized {
    type LoadError: std::error::Error + 'static;
    type SaveError: std::error::Error + 'static;
    type Type;
    const EXTENSION: &'static str;

    fn load_page(path: &Path) -> Result<Self::Type, Self::LoadError>;
    fn save_page(value: &Self::Type, path: &Path) -> Result<(), Self::SaveError>;
}

pub struct Png(());

impl DiskFormat for Png {
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

#[derive(Error)]
pub enum LoadFailure<F: DiskFormat> {
    #[error("an io error occured")]
    Io(#[from] io::Error),

    #[error("a page could not be loaded")]
    Format(#[source] F::LoadError),
}

impl<F: DiskFormat> Debug for LoadFailure<F>
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

#[derive(Error)]
pub enum SaveFailure<F: DiskFormat> {
    #[error("an io error occured")]
    Io(#[from] io::Error),

    #[error("a page could not be saved")]
    Format(#[source] F::SaveError),
}

impl<F: DiskFormat> Debug for SaveFailure<F>
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

#[tracing::instrument]
pub fn probe_pages<F: DiskFormat>(dir: &Path) -> Result<usize, LoadFailure<F>> {
    let mut count = 0;

    load_pages_internal(dir, |_, _| -> Result<_, F::LoadError> {
        count += 1;
        Ok(())
    })?;

    Ok(count)
}

#[tracing::instrument]
pub fn load_pages<F: DiskFormat>(dir: &Path) -> Result<Vec<F::Type>, LoadFailure<F>> {
    let mut values = BTreeMap::new();

    load_pages_internal(dir, |path, page| -> Result<_, F::LoadError> {
        values.insert(page, F::load_page(path)?);
        Ok(())
    })?;

    Ok(values.into_values().collect())
}

fn load_pages_internal<F: DiskFormat>(
    dir: &Path,
    mut loader: impl FnMut(&Path, usize) -> Result<(), F::LoadError>,
) -> Result<(), LoadFailure<F>> {
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
            loader(&path, page).map_err(LoadFailure::Format)?;
        }
    }

    Ok(())
}

#[tracing::instrument]
fn save_pages<F: DiskFormat>(dir: &Path, pages: &[F::Type]) -> Result<(), SaveFailure<F>>
where
    F::Type: Debug,
{
    for (idx, page) in pages.iter().enumerate() {
        let path = dir.join(idx.to_string()).with_extension(F::EXTENSION);
        F::save_page(page, &path).map_err(SaveFailure::Format)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use tiny_skia::PremultipliedColorU8;

    use super::*;

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
        save_pages::<Png>(
            Path::new("../../assets/test-assets/store/save"),
            &[create_pixmap()],
        )
        .unwrap();
    }

    #[test]
    fn test_load_png() {
        let pages = load_pages::<Png>(Path::new("../../assets/test-assets/store/load")).unwrap();

        assert_eq!(pages.len(), 3);

        for (a, b) in pages[0].pixels().iter().zip(create_pixmap().pixels()) {
            assert_eq!(a, b);
        }
    }

    #[test]
    fn test_probe_png() {
        assert_eq!(
            probe_pages::<Png>(Path::new("../../assets/test-assets/store/load")).unwrap(),
            3
        );
    }
}
