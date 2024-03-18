use std::collections::BTreeMap;
use std::fmt::Debug;
use std::fs::{self, File};
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};

use rayon::prelude::*;
use typst_project::manifest::Manifest;
use walkdir::WalkDir;

use self::test::Test;
use crate::config::Config;
use crate::util;

pub mod test;

const DEFAULT_TEST_INPUT: &str = include_str!("../../../../assets/default-test/test.typ");
const DEFAULT_TEST_OUTPUT: &[u8] = include_bytes!("../../../../assets/default-test/test.png");
const DEFAULT_IGNORE_LINES: &[&str] = &["**.png\n", "**.svg\n", "**.pdf\n"];
const DEFAULT_GIT_IGNORE_LINES: &[&str] = &["**/out/\n", "**/diff/\n"];

#[tracing::instrument]
pub fn try_open_manifest(root: &Path) -> io::Result<Option<Manifest>> {
    if is_project_root(root)? {
        let content = std::fs::read_to_string(root.join(typst_project::heuristics::MANIFEST_FILE))?;

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
    typst_project::is_project_root(path)
}

#[tracing::instrument]
pub fn try_find_project_root(path: &Path) -> io::Result<Option<&Path>> {
    typst_project::try_find_project_root(path)
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ScaffoldOptions: u32 {
        /// Create a default example test.
        const EXAMPLE = 0;

        /// Create a default .ignore file.
        const IGNORE = 1 << 0;

        /// Create a default .gitignore file.
        const GITIGNORE = 1 << 1;
    }
}

#[derive(Debug)]
pub struct Project {
    config: Config,
    manifest: Option<Manifest>,
    root: PathBuf,
    tests: BTreeMap<String, Test>,
    template: Option<String>,
}

impl Project {
    pub fn new(root: PathBuf, config: Config, manifest: Option<Manifest>) -> Self {
        Self {
            config,
            manifest,
            tests: BTreeMap::new(),
            root,
            template: None,
        }
    }

    pub fn name(&self) -> &str {
        self.manifest
            .as_ref()
            .map(|m| &m.package.name[..])
            .unwrap_or("<unknown package>")
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn manifest(&self) -> Option<&Manifest> {
        self.manifest.as_ref()
    }

    pub fn tests(&self) -> &BTreeMap<String, Test> {
        &self.tests
    }

    pub fn tests_mut(&mut self) -> &mut BTreeMap<String, Test> {
        &mut self.tests
    }

    pub fn template_path(&self) -> Option<PathBuf> {
        self.config.template.as_ref().map(|t| self.root.join(t))
    }

    pub fn template(&self) -> Option<&str> {
        self.template.as_deref()
    }

    pub fn tests_root_dir(&self) -> PathBuf {
        self.root.join(&self.config.tests)
    }

    pub fn root_exists(&self) -> io::Result<bool> {
        self.root.try_exists()
    }

    #[tracing::instrument(skip(self))]
    fn ensure_root(&self) -> Result<(), Error> {
        if self.root_exists()? {
            Ok(())
        } else {
            Err(Error::RootNotFound(self.root.clone()))
        }
    }

    pub fn is_init(&self) -> io::Result<bool> {
        self.tests_root_dir().try_exists()
    }

    #[tracing::instrument(skip(self))]
    fn ensure_init(&self) -> Result<(), Error> {
        self.ensure_root()?;

        if self.is_init()? {
            Ok(())
        } else {
            Err(Error::InitNeeded)
        }
    }

    #[tracing::instrument(skip(self))]
    pub fn init(&mut self, options: ScaffoldOptions) -> Result<Option<&Test>, Error> {
        #[cfg(debug_assertions)]
        self.ensure_root()?;

        let test_dir = self.tests_root_dir();
        if test_dir.try_exists()? {
            tracing::warn!(path = ?test_dir, "test dir already exists");
            return Err(Error::DoubleInit);
        }

        let tests_root_dir = self.tests_root_dir();
        tracing::trace!(path = ?tests_root_dir, "creating tests root dir");
        util::fs::create_dir(&tests_root_dir, false)?;

        let create_ignore = |file, option, lines: &[&str]| -> Result<(), Error> {
            if options.contains(option) {
                let gitignore = tests_root_dir.join(file);
                tracing::debug!(path = ?gitignore, "writing {file}");
                let mut gitignore = File::options()
                    .write(true)
                    .create_new(true)
                    .open(gitignore)?;

                gitignore.write_all(b"# added by typst-test\n")?;
                for pattern in lines {
                    gitignore.write_all(pattern.as_bytes())?;
                }
            } else {
                tracing::debug!("skipping default {file}");
            }

            Ok(())
        };

        create_ignore(".ignore", ScaffoldOptions::IGNORE, DEFAULT_IGNORE_LINES)?;
        create_ignore(
            ".gitignore",
            ScaffoldOptions::GITIGNORE,
            DEFAULT_GIT_IGNORE_LINES,
        )?;

        if options.contains(ScaffoldOptions::EXAMPLE) {
            tracing::debug!("adding default test");
            let (test, _) = self.create_test("example")?;
            Ok(Some(test))
        } else {
            tracing::debug!("skipping default test");
            Ok(None)
        }
    }

    #[tracing::instrument(skip(self))]
    pub fn uninit(&self) -> Result<(), Error> {
        #[cfg(debug_assertions)]
        self.ensure_init()?;

        util::fs::remove_dir(self.tests_root_dir(), true)?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub fn clean_artifacts(&self) -> Result<(), Error> {
        #[cfg(debug_assertions)]
        self.ensure_init()?;

        self.tests.par_iter().try_for_each(|(_, test)| {
            util::fs::remove_dir(test.out_dir(self), true)?;
            util::fs::remove_dir(test.diff_dir(self), true)?;
            Ok(())
        })
    }

    #[tracing::instrument(skip(self))]
    pub fn load_template(&mut self) -> Result<(), Error> {
        #[cfg(debug_assertions)]
        self.ensure_init()?;

        match fs::read_to_string(self.tests_root_dir().join("template.typ")) {
            Ok(template) => self.template = Some(template),
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => return Err(Error::Io(err)),
        }

        Ok(())
    }

    pub fn get_test(&self, test: &str) -> Option<&Test> {
        self.tests.get(test)
    }

    pub fn find_test(&self, test: &str) -> Result<&Test, Error> {
        self.get_test(test)
            .ok_or_else(|| Error::TestUnknown(test.to_owned()))
    }

    #[tracing::instrument(skip(self))]
    pub fn create_test(&mut self, test: &str) -> Result<(&Test, bool), Error> {
        #[cfg(debug_assertions)]
        self.ensure_init()?;

        if self.get_test(test).is_some() {
            return Err(Error::TestsAlreadyExists(test.to_owned()));
        }

        let test = Test::new(test.to_owned());

        let test_dir = test.test_dir(self);
        tracing::trace!(path = ?test_dir, "creating test dir");
        util::fs::create_dir(&test_dir, true)?;

        let test_script = test.test_file(self);
        tracing::trace!(path = ?test_script , "creating test script");
        let mut test_script = File::options()
            .write(true)
            .create_new(true)
            .open(test_script)?;
        test_script.write_all(
            self.template
                .as_deref()
                .unwrap_or(DEFAULT_TEST_INPUT)
                .as_bytes(),
        )?;

        if self.template.is_none() {
            let ref_dir = test.ref_dir(self);
            tracing::trace!(path = ?ref_dir, "creating ref dir");
            util::fs::create_empty_dir(&ref_dir, false)?;

            let test_ref = ref_dir.join("1").with_extension("png");
            tracing::trace!(path = ?test_ref, "creating ref image");
            let mut test_ref = File::options()
                .write(true)
                .create_new(true)
                .open(test_ref)?;
            test_ref.write_all(DEFAULT_TEST_OUTPUT)?;
            let test = self.tests.entry(test.name().to_owned()).or_insert(test);
            Ok((test, true))
        } else {
            let test = self.tests.entry(test.name().to_owned()).or_insert(test);
            Ok((test, false))
        }
    }

    #[tracing::instrument(skip(self))]
    pub fn remove_test(&self, test: &str) -> Result<(), Error> {
        #[cfg(debug_assertions)]
        self.ensure_init()?;

        let Some(test) = self.get_test(test) else {
            return Err(Error::TestUnknown(test.to_owned()));
        };

        let test_dir = test.test_dir(self);
        tracing::trace!(path = ?test_dir, "removing test dir");
        util::fs::remove_dir(test_dir, true)?;

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub fn discover_tests(&mut self) -> Result<(), Error> {
        #[cfg(debug_assertions)]
        self.ensure_init()?;

        let root = self.tests_root_dir();
        for entry in WalkDir::new(&root).min_depth(1) {
            let entry = entry?;
            let typ = entry.file_type();
            let name = entry.file_name();

            if !typ.is_file() {
                continue;
            }

            if name != "test.typ" {
                tracing::debug!(?name, "skipping file");
                continue;
            }

            // isolate the dir path of the test script relative to the tests root dir
            let relative = entry
                .path()
                .parent()
                .and_then(|p| p.strip_prefix(&root).ok())
                .expect("we have at one depth of directories (./tests/<x>/test.typ)");

            let Some(name) = relative.to_str() else {
                tracing::error!(?name, "couldn't convert path into UTF-8, skipping");
                continue;
            };

            let test = Test::new(name.to_owned());
            tracing::debug!(name = ?test.name(), "loaded test");
            self.tests.insert(test.name().to_owned(), test);
        }

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("project not found: {0:?}")]
    RootNotFound(PathBuf),

    #[error("project is not initalized")]
    InitNeeded,

    #[error("project is already initialized")]
    DoubleInit,

    #[error("unknown test: {0:?}")]
    TestUnknown(String),

    #[error("test already exsits: {0:?}")]
    TestsAlreadyExists(String),

    #[error("an error occured while traversing directories")]
    WalkDir(#[from] walkdir::Error),

    #[error("an io error occurred")]
    Io(#[from] io::Error),
}
