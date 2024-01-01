use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::{fs, io};

use self::test::Test;

pub mod test;

pub fn is_dir_project_root(dir: &Path) -> io::Result<bool> {
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
        if is_dir_project_root(ancestor)? {
            return Ok(Some(ancestor.to_path_buf()));
        }
    }

    Ok(None)
}

#[derive(Debug, Clone)]
pub struct Project {
    root: PathBuf,
    test_dir: PathBuf,
    tests: HashSet<Test>,
}

impl Project {
    pub fn new(root: PathBuf, test_dir: Option<PathBuf>) -> Self {
        Self {
            root,
            test_dir: test_dir.unwrap_or_else(|| "tests".into()),
            tests: HashSet::new(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn test_dir(&self) -> PathBuf {
        self.root.join(&self.test_dir)
    }

    pub fn tests(&self) -> &HashSet<Test> {
        &self.tests
    }

    #[tracing::instrument(skip_all)]
    pub fn load_tests(&mut self) -> anyhow::Result<()> {
        for entry in fs::read_dir(self.test_dir().join("typ"))? {
            let entry = entry?;
            let typ = entry.file_type()?;
            let name = entry.file_name();

            let Some(name) = name.to_str().map(ToOwned::to_owned) else {
                anyhow::bail!("Couldn't convert {name:?} into UTF-8 test name");
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

            let test = Test::new(name, typ.is_dir());
            tracing::debug!(name = ?test.name(), folder = ?test.folder(), "loaded test");
            self.tests.insert(test);
        }

        Ok(())
    }
}
