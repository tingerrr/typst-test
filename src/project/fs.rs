use std::collections::HashSet;
use std::fmt::Debug;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::{fs, io};

use oxipng::{InFile, Options, OutFile};
use rayon::prelude::*;

use super::test::Test;
use super::ScaffoldMode;
use crate::util;

pub const DEFAULT_TEST_INPUT: &str = include_str!("../../assets/default-test/test.typ");
pub const DEFAULT_TEST_OUTPUT: &[u8] = include_bytes!("../../assets/default-test/test.png");
pub const DEFAULT_GIT_IGNORE_LINES: &[&str] = &["out/**\n", "diff/**\n"];

const TEST_DIR: &str = "tests";
const TEST_SCRIPT_DIR: &str = "typ";
const REF_SCRIPT_DIR: &str = "ref";
const OUT_SCRIPT_DIR: &str = "out";
const DIFF_SCRIPT_DIR: &str = "diff";

#[tracing::instrument]
pub fn is_project_root(dir: &Path) -> io::Result<bool> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let typ = entry.file_type()?;
        let name = entry.file_name();

        if typ.is_file() && name == "typst.toml" {
            return Ok(true);
        }
    }

    Ok(false)
}

#[tracing::instrument]
pub fn try_find_project_root(pwd: &Path) -> io::Result<Option<PathBuf>> {
    for ancestor in pwd.ancestors() {
        if is_project_root(ancestor)? {
            return Ok(Some(ancestor.to_path_buf()));
        }
    }

    Ok(None)
}

#[derive(Debug)]
pub struct Fs {
    root: PathBuf,
    required_created: AtomicBool,
}

impl Fs {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            required_created: AtomicBool::new(false),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn test_dir(&self) -> PathBuf {
        util::fs::dir_in_root(&self.root, [TEST_DIR])
    }

    pub fn script_dir(&self) -> PathBuf {
        util::fs::dir_in_root(&self.root, [TEST_DIR, TEST_SCRIPT_DIR])
    }

    pub fn ref_dir(&self) -> PathBuf {
        util::fs::dir_in_root(&self.root, [TEST_DIR, REF_SCRIPT_DIR])
    }

    pub fn out_dir(&self) -> PathBuf {
        util::fs::dir_in_root(&self.root, [TEST_DIR, OUT_SCRIPT_DIR])
    }

    pub fn diff_dir(&self) -> PathBuf {
        util::fs::dir_in_root(&self.root, [TEST_DIR, DIFF_SCRIPT_DIR])
    }

    #[tracing::instrument(skip(self))]
    fn ensure_root(&self) -> Result<(), Error> {
        if self.root.try_exists()? {
            Ok(())
        } else {
            Err(Error::RootNotFound(self.root.clone()))
        }
    }

    #[tracing::instrument(skip(self))]
    fn ensure_init(&self) -> Result<(), Error> {
        self.ensure_root()?;

        if self.script_dir().try_exists()? {
            Ok(())
        } else {
            Err(Error::InitNeeded)
        }
    }

    #[tracing::instrument(skip(self))]
    fn create_required(&self) -> Result<(), Error> {
        if self.required_created.swap(true, Ordering::SeqCst) {
            return Ok(());
        }

        let folders = [
            ("script", self.script_dir()),
            ("ref", self.ref_dir()),
            ("out", self.out_dir()),
            ("diff", self.diff_dir()),
        ];

        for (name, path) in folders {
            tracing::trace!(path = ?path, "ensuring {name} dir");
            util::fs::create_dir(path, false)?;
        }

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub fn init(&self, mode: ScaffoldMode) -> Result<bool, Error> {
        self.ensure_root()?;

        let test_dir = self.test_dir();
        if test_dir.try_exists()? {
            tracing::warn!(path = ?test_dir, "test dir already exists");
            return Ok(false);
        }

        let test_dir = self.test_dir();
        let script_dir = self.script_dir();

        for (name, path) in [("test", &test_dir), ("script", &script_dir)] {
            tracing::trace!(?path, "creating {name} dir");
            util::fs::create_dir(path, false)?;
        }

        let gitignore = test_dir.join(".gitignore");
        tracing::debug!(path = ?gitignore, "writing .gitignore");
        let mut gitignore = File::options()
            .write(true)
            .create_new(true)
            .open(gitignore)?;

        gitignore.write_all(b"# added by typst-test, do not edit this line\n")?;
        for pattern in DEFAULT_GIT_IGNORE_LINES {
            gitignore.write_all(pattern.as_bytes())?;
        }

        if mode == ScaffoldMode::WithExample {
            tracing::debug!("adding default test");
            self.add_test("test".into(), false)?;
        } else {
            tracing::debug!("skipping default test");
        }

        Ok(true)
    }

    #[tracing::instrument(skip(self))]
    pub fn uninit(&self) -> Result<(), Error> {
        self.ensure_init()?;

        util::fs::remove_dir(self.test_dir(), true)?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub fn clean_artifacts(&self) -> Result<(), Error> {
        self.ensure_init()?;

        // TODO: remove unused refs

        util::fs::remove_dir(self.out_dir(), true)?;
        util::fs::remove_dir(self.diff_dir(), true)?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub fn add_test(&self, test: String, folder: bool) -> Result<Test, Error> {
        self.ensure_init()?;
        self.create_required()?;

        if self
            .load_tests()?
            .iter()
            .find(|t| t.name() == test)
            .is_some()
        {
            return Err(Error::TestsAlreadyExists(test));
        }

        let script_dir = self.script_dir().join(&test);
        let test_script = if folder {
            tracing::trace!(path = ?script_dir, "creating test dir");
            util::fs::create_empty_dir(&script_dir)?;
            script_dir.join("test")
        } else {
            script_dir
        };

        let test_script = test_script.with_extension("typ");
        tracing::trace!(path = ?test_script , "creating test script");
        let mut test_script = File::options()
            .write(true)
            .create_new(true)
            .open(test_script)?;
        test_script.write_all(DEFAULT_TEST_INPUT.as_bytes())?;

        let ref_dir = self.ref_dir().join(&test);
        tracing::trace!(path = ?ref_dir, "creating ref dir");
        util::fs::create_empty_dir(&ref_dir)?;

        let test_ref = ref_dir.join("1").with_extension("png");
        tracing::trace!(path = ?test_ref, "creating ref image");
        let mut test_ref = File::options()
            .write(true)
            .create_new(true)
            .open(test_ref)?;
        test_ref.write_all(DEFAULT_TEST_OUTPUT)?;

        Ok(Test::new(test, folder))
    }

    #[tracing::instrument(skip(self))]
    pub fn remove_test(&self, test: String) -> Result<(), Error> {
        self.ensure_init()?;
        if self
            .load_tests()?
            .iter()
            .find(|t| t.name() == test)
            .is_none()
        {
            return Err(Error::TestUnknown(test));
        }

        for (name, dir) in [
            ("out", self.out_dir()),
            ("ref", self.ref_dir()),
            ("diff", self.diff_dir()),
            ("script", self.script_dir()),
        ] {
            let path = dir.join(&test);
            tracing::trace!(?path, "deleting {name} dir");
            util::fs::remove_dir(path, true)?
        }

        let script_dir = self.script_dir().join(test);
        if script_dir.is_dir() {
            tracing::trace!(path = ?script_dir, "deleting script dir");
            util::fs::remove_dir(script_dir, true)?
        } else {
            util::fs::remove_file(script_dir.with_extension("typ"))?
        }

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub fn load_tests(&self) -> Result<HashSet<Test>, Error> {
        self.ensure_init()?;
        self.create_required()?;

        let typ_dir = self.script_dir();

        let mut tests = HashSet::new();
        for entry in fs::read_dir(typ_dir)? {
            let entry = entry?;
            let typ = entry.file_type()?;
            let name = entry.file_name();

            let Some(name) = name.to_str() else {
                tracing::warn!(?name, "couldn't convert path into UTF-8, skipping");
                continue;
            };

            if typ.is_dir() {
                if !entry.path().join("test.typ").try_exists()? {
                    tracing::debug!(?name, "skipping folder, no test.typ detected");
                    continue;
                }
            } else if !name.ends_with(".typ") {
                tracing::debug!(?name, "skipping file, not a typ file");
                continue;
            }

            let test = Test::new(name.trim_end_matches(".typ").into(), typ.is_dir());
            tracing::debug!(name = ?test.name(), "loaded test");
            tests.insert(test);
        }

        Ok(tests)
    }

    #[tracing::instrument(skip(self))]
    pub fn update_tests<'p, I: IntoParallelIterator<Item = &'p Test> + Debug>(
        &self,
        tests: I,
    ) -> Result<(), Error> {
        self.ensure_init()?;
        self.create_required()?;

        let options = Options::max_compression();

        let out_dir = self.out_dir();
        let ref_dir = self.ref_dir();

        tests.into_par_iter().map(Test::name).try_for_each(|test| {
            tracing::debug!(?test, "updating refs");
            let out_dir = out_dir.join(test);
            let ref_dir = ref_dir.join(test);

            tracing::trace!(path = ?out_dir, "creating out dir");
            util::fs::create_dir(&out_dir, true)?;

            tracing::trace!(path = ?ref_dir, "clearing ref dir");
            util::fs::create_empty_dir(&ref_dir)?;

            tracing::trace!(path = ?out_dir, "collecting new refs from out dir");
            let entries = util::fs::collect_dir_entries(&out_dir)?;

            for (idx, entry) in entries.into_iter().enumerate() {
                tracing::debug!(?test, "ref" = ?idx + 1, "writing optimized ref");
                let name = entry.file_name();

                // TODO: better error handling
                oxipng::optimize(
                    &InFile::Path(entry.path()),
                    &OutFile::from_path(ref_dir.join(name)),
                    &options,
                )
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            }

            Ok(())
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("project not found: {0:?}")]
    RootNotFound(PathBuf),

    #[error("project is not initalized")]
    InitNeeded,

    #[error("unknown test: {0:?}")]
    TestUnknown(String),

    #[error("test already exsits: {0:?}")]
    TestsAlreadyExists(String),

    #[error("an io error occurred")]
    Io(#[from] io::Error),
}
