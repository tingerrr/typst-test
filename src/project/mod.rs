use std::collections::HashSet;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::{fs, io};

use oxipng::{InFile, Options, OutFile};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use self::test::Test;
use crate::util;

pub mod test;

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
    for entry in fs::read_dir(dir)? {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScaffoldMode {
    WithExample,
    NoExample,
}

#[derive(Debug, Clone)]
pub struct Project {
    name: String,
    root: PathBuf,
    tests: HashSet<Test>,
}

impl Project {
    pub fn new(root: PathBuf, name: String) -> Self {
        Self {
            name,
            root,
            tests: HashSet::new(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn test_dir(&self) -> PathBuf {
        util::fs::dir_in_root(&self.root, [TEST_DIR])
    }

    pub fn test_script_dir(&self) -> PathBuf {
        util::fs::dir_in_root(&self.root, [TEST_DIR, TEST_SCRIPT_DIR])
    }

    pub fn test_ref_dir(&self) -> PathBuf {
        util::fs::dir_in_root(&self.root, [TEST_DIR, REF_SCRIPT_DIR])
    }

    pub fn test_out_dir(&self) -> PathBuf {
        util::fs::dir_in_root(&self.root, [TEST_DIR, OUT_SCRIPT_DIR])
    }

    pub fn test_diff_dir(&self) -> PathBuf {
        util::fs::dir_in_root(&self.root, [TEST_DIR, DIFF_SCRIPT_DIR])
    }

    pub fn tests(&self) -> &HashSet<Test> {
        &self.tests
    }

    #[tracing::instrument(skip(self))]
    pub fn create_tests_scaffold(&self, mode: ScaffoldMode) -> io::Result<()> {
        let test_dir = self.test_dir();
        let typ_dir = self.test_script_dir();

        tracing::trace!(dir = ?test_dir, "ensuring tests dir");
        util::fs::ensure_dir(&test_dir, true)?;

        tracing::trace!(dir = ?test_dir, "ensuring test script dir");
        util::fs::ensure_dir(&typ_dir, true)?;

        let mut file = File::options()
            .read(true)
            .append(true)
            .create(true)
            .open(test_dir.join(".gitignore"))?;

        let mut buffer = String::new();
        file.read_to_string(&mut buffer)?;

        if buffer.is_empty() {
            tracing::debug!("opened/created empty .gitignore");
        } else {
            tracing::debug!("opened .gitignore");
        }

        const INDICATOR: &str = "# added by typst-test, do not edit this line";
        let lines: HashSet<&str> = buffer.lines().collect();

        if lines.is_empty() || !lines.contains(INDICATOR) {
            tracing::debug!("writing .gitignore");

            if !buffer.is_empty() {
                file.write_all(b"\n")?;
            }

            file.write_all(INDICATOR.as_bytes())?;
            file.write_all(b"\n")?;
            for pattern in DEFAULT_GIT_IGNORE_LINES {
                file.write_all(pattern.as_bytes())?;
            }
        }

        if mode == ScaffoldMode::WithExample {
            if fs::read_dir(&typ_dir)?.next().is_some_and(|r| r.is_ok()) {
                return Ok(());
            }

            tracing::debug!("adding example test");

            let example_input = typ_dir.join("test").with_extension("typ");
            let mut file = File::options()
                .write(true)
                .create_new(true)
                .open(example_input)?;
            file.write_all(DEFAULT_TEST_INPUT.as_bytes())?;

            let example_ref_dir = test_dir.join("ref").join("test");
            util::fs::ensure_dir(&example_ref_dir, true)?;

            let example_output = example_ref_dir.join("1").with_extension("png");
            let mut file = File::options()
                .write(true)
                .create_new(true)
                .open(example_output)?;
            file.write_all(DEFAULT_TEST_OUTPUT)?;
        }

        Ok(())
    }

    #[tracing::instrument(skip_all)]
    pub fn remove_tests_scaffold(&self) -> io::Result<()> {
        let test_dir = self.test_dir();
        util::fs::ensure_remove_dir(test_dir, true)?;

        Ok(())
    }

    #[tracing::instrument(skip_all)]
    pub fn load_tests(&mut self) -> io::Result<()> {
        let typ_dir = self.test_dir().join("typ");

        // TODO: return an error
        if !typ_dir.try_exists()? {
            return Ok(());
        }

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
            self.tests.insert(test);
        }

        Ok(())
    }

    #[tracing::instrument(skip_all)]
    pub fn update_tests(&self, filter: Option<String>) -> io::Result<()> {
        let filter = filter.as_deref().unwrap_or_default();
        let options = Options::max_compression();

        let out_dir = self.test_out_dir();
        let ref_dir = self.test_ref_dir();

        tracing::trace!(path = ?out_dir, "ensuring out dir");
        util::fs::ensure_dir(&out_dir, true)?;

        tracing::trace!(path = ?ref_dir, "ensuring empty ref dir");
        util::fs::ensure_empty_dir(&ref_dir, true)?;

        self.tests
            .par_iter()
            .map(Test::name)
            .filter(|test| test.contains(filter))
            .try_for_each(|test| {
                tracing::debug!(?test, "updating refs");
                let out_dir = out_dir.join(test);
                let ref_dir = ref_dir.join(test);

                tracing::trace!(path = ?out_dir, "ensuring test out dir");
                util::fs::ensure_dir(&out_dir, true)?;

                tracing::trace!(path = ?ref_dir, "ensuring empty test ref dir");
                util::fs::ensure_empty_dir(&ref_dir, true)?;

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
