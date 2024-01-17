use std::collections::HashSet;
use std::fmt::Debug;
use std::fs::File;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};

use oxipng::{InFile, Options, OutFile};
use rayon::prelude::*;
use typst_manifest::Manifest;
use walkdir::WalkDir;

use super::test::Test;
use super::ScaffoldMode;
use crate::report::Reporter;
use crate::util;

pub const DEFAULT_TEST_INPUT: &str = include_str!("../../../../assets/default-test/test.typ");
pub const DEFAULT_TEST_OUTPUT: &[u8] = include_bytes!("../../../../assets/default-test/test.png");
pub const DEFAULT_GIT_IGNORE_LINES: &[&str] = &["out/**\n", "diff/**\n"];

const REF_DIR: &str = "ref";
const OUT_DIR: &str = "out";
const DIFF_DIR: &str = "diff";

#[tracing::instrument]
pub fn try_open_manifest(root: &Path) -> io::Result<Option<Manifest>> {
    if is_project_root(root)? {
        let content = std::fs::read_to_string(root.join(typst_manifest::MANIFEST_NAME))?;

        // TODO: better error handling
        let manifest =
            Manifest::from_str(&content).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        Ok(Some(manifest))
    } else {
        Ok(None)
    }
}

#[tracing::instrument]
pub fn is_project_root(path: &Path) -> io::Result<bool> {
    typst_manifest::is_package_root(path)
}

#[tracing::instrument]
pub fn try_find_project_root(path: &Path) -> io::Result<Option<&Path>> {
    typst_manifest::try_find_package_root(path)
}

#[derive(Debug)]
pub struct Fs {
    root: PathBuf,
    tests_root: PathBuf,
    reporter: Reporter,
}

impl Fs {
    pub fn new(root: PathBuf, tests_dir: PathBuf, reporter: Reporter) -> Self {
        let tests_root = root.join(tests_dir);

        Self {
            root,
            tests_root,
            reporter,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn tests_root_dir(&self) -> &Path {
        &self.tests_root
    }

    pub fn test_dir(&self, test: &Test) -> PathBuf {
        util::fs::path_in_root(&self.tests_root, [test.name()])
    }

    pub fn ref_dir(&self, test: &Test) -> PathBuf {
        util::fs::path_in_root(&self.tests_root, [test.name(), REF_DIR])
    }

    pub fn out_dir(&self, test: &Test) -> PathBuf {
        util::fs::path_in_root(&self.tests_root, [test.name(), OUT_DIR])
    }

    pub fn diff_dir(&self, test: &Test) -> PathBuf {
        util::fs::path_in_root(&self.tests_root, [test.name(), DIFF_DIR])
    }

    pub fn test_file(&self, test: &Test) -> PathBuf {
        util::fs::path_in_root(&self.tests_root, [test.name(), "test"]).with_extension("typ")
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

        if self.tests_root_dir().try_exists()? {
            Ok(())
        } else {
            Err(Error::InitNeeded)
        }
    }

    #[tracing::instrument(skip(self))]
    pub fn init(&self, mode: ScaffoldMode) -> Result<bool, Error> {
        self.ensure_root()?;

        let test_dir = self.tests_root_dir();
        if test_dir.try_exists()? {
            tracing::warn!(path = ?test_dir, "test dir already exists");
            return Ok(false);
        }

        let test = Test::new("example".to_owned());

        let tests_root_dir = self.tests_root_dir();
        let test_dir = self.test_dir(&test);

        for (name, path) in [("tests root", tests_root_dir), ("example test", &test_dir)] {
            tracing::trace!(?path, "creating {name} dir");
            util::fs::create_dir(path, false)?;
        }

        let gitignore = tests_root_dir.join(".gitignore");
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
            self.add_test(&Test::new("test".to_owned()))?;
        } else {
            tracing::debug!("skipping default test");
        }

        Ok(true)
    }

    #[tracing::instrument(skip(self))]
    pub fn uninit(&self) -> Result<(), Error> {
        self.ensure_init()?;

        util::fs::remove_dir(self.tests_root_dir(), true)?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub fn clean_artifacts(&self) -> Result<(), Error> {
        self.ensure_init()?;

        self.load_tests()?.par_iter().try_for_each(|test| {
            util::fs::remove_dir(self.out_dir(test), true)?;
            util::fs::remove_dir(self.diff_dir(test), true)?;
            Ok(())
        })
    }

    #[tracing::instrument(skip(self))]
    pub fn get_test(&self, test: &str) -> Result<Option<Test>, Error> {
        // TODO: don't load all tests?
        Ok(self.load_tests()?.into_iter().find(|t| t.name() == test))
    }

    #[tracing::instrument(skip(self))]
    pub fn find_test(&self, test: &str) -> Result<Test, Error> {
        self.get_test(test)?
            .ok_or_else(|| Error::TestUnknown(test.to_owned()))
    }

    #[tracing::instrument(skip(self))]
    pub fn add_test(&self, test: &Test) -> Result<(), Error> {
        self.ensure_init()?;

        if self.get_test(test.name())?.is_some() {
            return Err(Error::TestsAlreadyExists(test.name().to_owned()));
        }

        let test_dir = self.test_dir(&test);
        tracing::trace!(path = ?test_dir, "creating test dir");
        util::fs::create_empty_dir(&test_dir)?;

        let test_script = self.test_file(&test);
        tracing::trace!(path = ?test_script , "creating test script");
        let mut test_script = File::options()
            .write(true)
            .create_new(true)
            .open(test_script)?;
        test_script.write_all(DEFAULT_TEST_INPUT.as_bytes())?;

        let ref_dir = self.ref_dir(&test);
        tracing::trace!(path = ?ref_dir, "creating ref dir");
        util::fs::create_empty_dir(&ref_dir)?;

        let test_ref = ref_dir.join("1").with_extension("png");
        tracing::trace!(path = ?test_ref, "creating ref image");
        let mut test_ref = File::options()
            .write(true)
            .create_new(true)
            .open(test_ref)?;
        test_ref.write_all(DEFAULT_TEST_OUTPUT)?;

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub fn remove_test(&self, test: &str) -> Result<(), Error> {
        self.ensure_init()?;

        let Some(test) = self.get_test(test)? else {
            return Err(Error::TestUnknown(test.to_owned()));
        };

        let test_dir = self.test_dir(&test);
        tracing::trace!(path = ?test_dir, "removing test dir");
        util::fs::remove_dir(test_dir, true)?;

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub fn load_tests(&self) -> Result<HashSet<Test>, Error> {
        self.ensure_init()?;

        let tests_dir = self.tests_root_dir();

        let mut tests = HashSet::new();
        for entry in WalkDir::new(&tests_dir).min_depth(1) {
            let entry = entry?;
            let typ = entry.file_type();
            let name = entry.file_name();

            if !typ.is_file() || name != "test.typ" {
                tracing::debug!(?name, "skipping file");
                continue;
            }

            // isolate the dir path of the test script relative to the tests root dir
            let relative = entry
                .path()
                .parent()
                .and_then(|p| p.strip_prefix(&tests_dir).ok())
                .expect("we have at one depth of directories (./tests/<x>/test.typ)");

            let Some(name) = relative.to_str() else {
                tracing::error!(?name, "couldn't convert path into UTF-8, skipping");
                continue;
            };

            let test = Test::new(name.to_owned());
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

        let options = Options::max_compression();

        tests.into_par_iter().try_for_each(|test| {
            tracing::debug!(?test, "updating refs");
            let out_dir = self.out_dir(test);
            let ref_dir = self.ref_dir(test);

            tracing::trace!(path = ?out_dir, "creating out dir");
            util::fs::create_dir(&out_dir, true)?;

            tracing::trace!(path = ?ref_dir, "clearing ref dir");
            util::fs::create_empty_dir(&ref_dir)?;

            tracing::trace!(path = ?out_dir, "collecting new refs from out dir");
            let entries = util::fs::collect_dir_entries(&out_dir)?;

            // TODO: this is rather crude, get the indices without enumerate to allow random access
            entries
                .into_iter()
                .enumerate()
                .par_bridge()
                .try_for_each(|(idx, entry)| {
                    tracing::debug!(?test, "ref" = ?idx + 1, "writing optimized ref");
                    let name = entry.file_name();

                    // TODO: better error handling
                    oxipng::optimize(
                        &InFile::Path(entry.path()),
                        &OutFile::from_path(ref_dir.join(name)),
                        &options,
                    )
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
                })?;

            self.reporter.test_success(test.name(), "updated")?;

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

    #[error("an error occured while traversing directories")]
    WalkDir(#[from] walkdir::Error),

    #[error("an io error occurred")]
    Io(#[from] io::Error),
}
