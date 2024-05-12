use std::io;
use std::path::Path;

use ecow::{eco_vec, EcoVec};
use oxipng::PngError;
use thiserror::Error;
use tiny_skia::Pixmap;

// TODO: if we can, allow deserializing the document from disk
#[derive(Debug, Clone)]
pub enum Output {
    Png(EcoVec<Pixmap>),
}

impl Output {
    fn format(&self) -> Format {
        match self {
            Output::Png(_) => Format::Png,
        }
    }
}

// TODO: native format (see above)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Format {
    Pdf,
    Png,
    Svg,
}

#[derive(Debug, Clone, Error)]
pub enum UpdateFailure {
    #[error("png optimization failed")]
    Optimize {
        #[from]
        error: PngError,
    },
}

#[derive(Debug, Clone, Error)]
pub enum LoadFailure {}

// TODO: better error handling + tests
pub fn load_from_disk(path: &Path, format: Format) -> io::Result<Output> {
    match format {
        Format::Pdf => panic!("can't load PDF output yet"),
        Format::Svg => panic!("can't load SVG output yet"),
        Format::Png => {}
    }

    let metadata = path.metadata()?;
    if metadata.is_dir() {
        let mut loaded = eco_vec![];
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;

            // TODO: use dir_entry_ext2 for non alloc file_name
            if entry.file_type()?.is_file()
                && entry
                    .file_name()
                    .as_os_str()
                    .to_str()
                    .is_some_and(|n| n.ends_with(".png"))
            {
                loaded.push(Pixmap::load_png(entry.path()).unwrap());
            }
        }

        Ok(Output::Png(loaded))
    } else if metadata.is_file() {
        Ok(Output::Png(eco_vec![Pixmap::load_png(path).unwrap()]))
    } else {
        panic!("invalid output path, must be file or directory")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_png_single() {
        load_from_disk(
            Path::new("../../assets/test-references/persistent.png"),
            Format::Png,
        )
        .unwrap();
    }

    #[test]
    #[should_panic]
    fn test_load_png_single_not_found() {
        load_from_disk(
            Path::new("../../assets/test-references/doesnt-exist.png"),
            Format::Png,
        )
        .unwrap();
    }

    #[test]
    fn test_load_png_many() {
        load_from_disk(Path::new("../../assets/test-references/"), Format::Png).unwrap();
    }

    #[test]
    #[should_panic]
    fn test_load_svg_unsupported_format() {
        load_from_disk(Path::new("./"), Format::Svg).unwrap();
    }

    #[test]
    #[should_panic]
    fn test_load_pdf_unsupported_format() {
        load_from_disk(Path::new("./"), Format::Pdf).unwrap();
    }
}
