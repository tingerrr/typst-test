use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use thiserror::Error;
use tiny_skia::Pixmap;

use super::{Format, Resource};

#[derive(Debug, Error)]
pub enum LoadFailure {
    #[error("an io error occured")]
    Io(#[from] io::Error),

    #[error("a page could not be decoded")]
    Png(#[from] png::DecodingError),
}

#[derive(Debug, Error)]
pub enum SaveFailure {
    #[error("an io error occured")]
    Io(#[from] io::Error),

    #[error("a page could not be encoded")]
    Png(#[from] png::EncodingError),
}

#[tracing::instrument]
pub fn probe_pages(dir: &Path) -> Result<(usize, Option<Format>), LoadFailure> {
    let mut count = 0;
    let mut format = None;

    load_pages_internal(
        dir,
        |path| path.extension().is_some_and(|ext| ext == "png"),
        |_, _| -> Result<_, LoadFailure> {
            format = Some(Format::Png);
            count += 1;
            Ok(())
        },
    )?;

    Ok((count, format))
}

#[tracing::instrument]
pub fn load_pages(dir: &Path, format: Format) -> Result<Resource, LoadFailure> {
    match format {
        Format::Native => panic!("can't load native output yet"),
        Format::Pdf => panic!("can't load PDF output yet"),
        Format::Svg => panic!("can't load SVG output yet"),
        Format::Png => {
            let mut values = BTreeMap::new();

            load_pages_internal(
                dir,
                |path| path.extension().is_some_and(|ext| ext == "png"),
                |path, page| -> Result<_, png::DecodingError> {
                    values.insert(page, Pixmap::load_png(path)?);
                    Ok(())
                },
            )?;

            Ok(Resource::Png(values.into_values().collect()))
        }
    }
}

fn load_pages_internal<E>(
    dir: &Path,
    matcher: impl Fn(&Path) -> bool,
    mut loader: impl FnMut(&Path, usize) -> Result<(), E>,
) -> Result<(), LoadFailure>
where
    LoadFailure: From<E>,
{
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

        if matcher(&path) {
            loader(&path, page)?;
        }
    }

    Ok(())
}

#[tracing::instrument]
fn save_pages(dir: &Path, pages: Resource) -> Result<(), SaveFailure> {
    match pages {
        Resource::Png(pages) => {
            save_pages_internal(dir, &pages, "png", |page, path| page.save_png(path))
        }
    }
}

fn save_pages_internal<T, E>(
    dir: &Path,
    pages: &[T],
    ext: &str,
    saver: impl Fn(&T, &Path) -> Result<(), E>,
) -> Result<(), SaveFailure>
where
    SaveFailure: From<E>,
{
    for (idx, page) in pages.iter().enumerate() {
        let path = dir.join(idx.to_string()).with_extension(ext);
        saver(page, &path)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use ecow::eco_vec;
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
        save_pages(
            Path::new("../../assets/test-assets/store/save"),
            Resource::Png(eco_vec![create_pixmap()]),
        )
        .unwrap();
    }

    #[test]
    fn test_load_png() {
        let Resource::Png(pages) = load_pages(
            Path::new("../../assets/test-assets/store/load"),
            Format::Png,
        )
        .unwrap();

        assert_eq!(pages.len(), 3);

        for (a, b) in pages[0].pixels().iter().zip(create_pixmap().pixels()) {
            assert_eq!(a, b);
        }
    }

    #[test]
    #[should_panic]
    fn test_load_svg_unsupported_format() {
        load_pages(Path::new("./"), Format::Svg).unwrap();
    }

    #[test]
    #[should_panic]
    fn test_load_pdf_unsupported_format() {
        load_pages(Path::new("./"), Format::Pdf).unwrap();
    }

    #[test]
    fn test_probe_png() {
        assert_eq!(
            probe_pages(Path::new("../../assets/test-assets/store/load")).unwrap(),
            (3, Some(Format::Png))
        );
    }
}
