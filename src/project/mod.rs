use std::collections::HashSet;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::{fs, io};

use oxipng::{InFile, Options, OutFile};

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

pub fn dir_in_root<P, I, T>(root: P, parts: I) -> PathBuf
where
    P: AsRef<Path>,
    I: IntoIterator<Item = T>,
    T: AsRef<Path>,
{
    let root: &Path = root.as_ref();
    let mut result = root.to_path_buf();
    result.extend(parts);

    debug_assert!(
        util::fs::is_ancestor_of(root, &result),
        "unintended escape from root"
    );
    result
}

pub fn test_dir<P: AsRef<Path>>(root: P) -> PathBuf {
    dir_in_root(root, [TEST_DIR])
}

pub fn test_script_dir<P: AsRef<Path>>(root: P) -> PathBuf {
    dir_in_root(root, [TEST_DIR, TEST_SCRIPT_DIR])
}

pub fn test_ref_dir<P: AsRef<Path>>(root: P) -> PathBuf {
    dir_in_root(root, [TEST_DIR, REF_SCRIPT_DIR])
}

pub fn test_out_dir<P: AsRef<Path>>(root: P) -> PathBuf {
    dir_in_root(root, [TEST_DIR, OUT_SCRIPT_DIR])
}

pub fn test_diff_dir<P: AsRef<Path>>(root: P) -> PathBuf {
    dir_in_root(root, [TEST_DIR, DIFF_SCRIPT_DIR])
}

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
    root: PathBuf,
    tests: HashSet<Test>,
}

impl Project {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            tests: HashSet::new(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn test_dir(&self) -> PathBuf {
        test_dir(&self.root)
    }

    pub fn tests(&self) -> &HashSet<Test> {
        &self.tests
    }

    #[tracing::instrument(skip_all)]
    pub fn create_tests_scaffold(&self, mode: ScaffoldMode) -> io::Result<()> {
        let test_dir = test_dir(&self.root);
        let typ_dir = test_script_dir(&self.root);

        // NOTE: we want to fail if `root` doesn't exist, so we create the test folder individually
        //       if this passed anything we create after this must have an existing root
        util::fs::ensure_dir(&test_dir, false)?;
        util::fs::ensure_dir(&typ_dir, true)?;

        let mut file = File::options()
            .read(true)
            .append(true)
            .create(true)
            .open(test_dir.join(".gitignore"))?;

        let mut buffer = String::new();
        file.read_to_string(&mut buffer)?;

        const INDICATOR: &str = "# added by typst-test, do not edit this line";
        let lines: HashSet<&str> = buffer.lines().collect();

        if lines.is_empty() || !lines.contains(INDICATOR) {
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
        let test_dir = test_dir(&self.root);
        util::fs::ensure_remove_dir(&test_dir, true)?;

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
                    tracing::debug!(?name, "loaded folder");
                    continue;
                }
            } else if !name.ends_with(".typ") {
                tracing::debug!(?name, "skipping file");
                continue;
            }

            let test = Test::new(name.trim_end_matches(".typ").into(), typ.is_dir());
            tracing::debug!(name = ?test.name(), folder = ?test.folder(), "loaded test");
            self.tests.insert(test);
        }

        Ok(())
    }

    #[tracing::instrument(skip_all)]
    pub fn update_tests(&self, filter: Option<String>) -> io::Result<()> {
        let filter = filter.as_deref().unwrap_or_default();
        let options = Options::max_compression();

        self.tests
            .iter()
            .map(Test::name)
            .filter(|test| test.contains(filter))
            .map(|test| {
                tracing::debug!(?test, "updating references");
                let out_dir = test_out_dir(&self.root).join(test);
                let ref_dir = test_ref_dir(&self.root).join(test);

                util::fs::ensure_dir(&out_dir, false)?;
                util::fs::ensure_remove_dir(&ref_dir, true)?;
                util::fs::ensure_dir(&ref_dir, false)?;

                for entry in util::fs::collect_dir_entries(&out_dir)? {
                    let name = entry.file_name();
                    oxipng::optimize(
                        &InFile::Path(entry.path()),
                        &OutFile::from_path(ref_dir.join(name)),
                        &options,
                    )
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                }

                Ok(())
            })
            .collect()
    }
}
