use std::borrow::Cow;
use std::collections::HashMap;
use std::fs as stdfs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use comemo::Prehashed;
use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime};
use typst::syntax::package::PackageSpec;
use typst::syntax::{FileId, Source};
use typst::text::{Font, FontBook};
use typst::{Library, World};

pub mod fs;

/// The file system path for a file ID.
fn system_path(id: FileId, root: &Path) -> FileResult<PathBuf> {
    let root = match id.package() {
        Some(spec) => &package_root(spec),
        None => root,
    };

    id.vpath().resolve(root).ok_or(FileError::AccessDenied)
}

fn package_root(spec: &PackageSpec) -> PathBuf {
    let subdir = format!(
        "typst/packages/{}/{}/{}",
        spec.namespace, spec.name, spec.version
    );
    let root = dirs::cache_dir().unwrap().join(subdir);

    if !root.try_exists().unwrap() {
        panic!("Can't download package: {spec} to {root:?}");
    }

    root
}

/// Read a file.
fn read(path: &Path) -> FileResult<Cow<'static, [u8]>> {
    // Resolve asset.
    if let Ok(suffix) = path.strip_prefix("assets/") {
        return typst_dev_assets::get(&suffix.to_string_lossy())
            .map(Cow::Borrowed)
            .ok_or_else(|| FileError::NotFound(path.into()));
    }

    let f = |e| FileError::from_io(e, path);
    if stdfs::metadata(path).map_err(f)?.is_dir() {
        Err(FileError::IsDirectory)
    } else {
        stdfs::read(path).map(Cow::Owned).map_err(f)
    }
}

#[derive(Debug, Clone)]
struct FileSlot {
    id: FileId,
    source: OnceLock<FileResult<Source>>,
    file: OnceLock<FileResult<Bytes>>,
}

impl FileSlot {
    /// Create a new file slot.
    fn new(id: FileId) -> Self {
        Self {
            id,
            file: OnceLock::new(),
            source: OnceLock::new(),
        }
    }

    /// Retrieve the source for this file.
    fn source(&mut self, root: &Path) -> FileResult<Source> {
        self.source
            .get_or_init(|| {
                let buf = read(&system_path(self.id, root)?)?;
                let text = String::from_utf8(buf.into_owned())?;
                Ok(Source::new(self.id, text))
            })
            .clone()
    }

    /// Retrieve the file's bytes.
    fn file(&mut self, root: &Path) -> FileResult<Bytes> {
        self.file
            .get_or_init(|| {
                read(&system_path(self.id, root)?).map(|cow| match cow {
                    Cow::Owned(buf) => buf.into(),
                    Cow::Borrowed(buf) => Bytes::from_static(buf),
                })
            })
            .clone()
    }
}

#[derive(Debug)]
pub struct GlobalTestWorld {
    pub root: PathBuf,
    pub lib: Prehashed<Library>,
    pub book: Prehashed<FontBook>,
    pub fonts: Vec<Font>,
    slots: Mutex<HashMap<FileId, FileSlot>>,
}

impl GlobalTestWorld {
    pub fn new(root: PathBuf, library: Library) -> Self {
        let fonts: Vec<_> = typst_assets::fonts()
            .chain(typst_dev_assets::fonts())
            .flat_map(|data| Font::iter(Bytes::from_static(data)))
            .collect();

        GlobalTestWorld {
            root,
            lib: Prehashed::new(library),
            book: Prehashed::new(FontBook::from_fonts(&fonts)),
            fonts,
            slots: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for GlobalTestWorld {
    fn default() -> Self {
        Self::new("".into(), Library::default())
    }
}

impl World for GlobalTestWorld {
    fn library(&self) -> &Prehashed<Library> {
        &self.lib
    }

    fn book(&self) -> &Prehashed<FontBook> {
        &self.book
    }

    fn main(&self) -> Source {
        panic!(
            "Global World does not contain a main file, it only provides the base implementation for Test Worlds."
        )
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        let mut map = self.slots.lock().unwrap();
        FileSlot::source(
            map.entry(id).or_insert_with(|| FileSlot::new(id)),
            &self.root,
        )
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        let mut map = self.slots.lock().unwrap();
        FileSlot::file(
            map.entry(id).or_insert_with(|| FileSlot::new(id)),
            &self.root,
        )
    }

    fn font(&self, index: usize) -> Option<Font> {
        Some(self.fonts[index].clone())
    }

    fn today(&self, _offset: Option<i64>) -> Option<Datetime> {
        Some(Datetime::from_ymd(1970, 1, 1).unwrap())
    }
}
