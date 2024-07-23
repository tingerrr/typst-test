use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::path::{Path, PathBuf};

use tempdir::TempDir;

use crate::util;

pub struct TempEnv {
    root: TempDir,
    found: BTreeMap<PathBuf, Option<Vec<u8>>>,
    expected: BTreeMap<PathBuf, Option<Vec<u8>>>,
}

/// Set up the project structure.
pub struct Setup(TempEnv);

impl Setup {
    pub fn setup_dir<P: AsRef<Path>>(&mut self, path: P) -> &mut Self {
        let abs_path = self.0.root.path().join(path.as_ref());
        util::fs::create_dir(abs_path, true).unwrap();
        self
    }

    pub fn setup_file<P: AsRef<Path>>(&mut self, path: P, content: impl AsRef<[u8]>) -> &mut Self {
        let abs_path = self.0.root.path().join(path.as_ref());
        let parent = abs_path.parent().unwrap();
        if parent != self.0.root.path() {
            util::fs::create_dir(parent, true).unwrap();
        }

        let content = content.as_ref();
        std::fs::write(&abs_path, content).unwrap();
        self
    }

    pub fn setup_file_empty<P: AsRef<Path>>(&mut self, path: P) -> &mut Self {
        let abs_path = self.0.root.path().join(path.as_ref());
        let parent = abs_path.parent().unwrap();
        if parent != self.0.root.path() {
            util::fs::create_dir(parent, true).unwrap();
        }

        std::fs::write(&abs_path, "").unwrap();
        self
    }
}

/// Specify what you expect to see after the test concluded.
pub struct Expect(TempEnv);

impl Expect {
    pub fn expect_dir<P: AsRef<Path>>(&mut self, path: P) -> &mut Self {
        self.0.add_expected(path.as_ref().to_path_buf(), None);
        self
    }

    pub fn expect_file<P: AsRef<Path>>(&mut self, path: P, content: impl AsRef<[u8]>) -> &mut Self {
        let content = content.as_ref();
        self.0
            .add_expected(path.as_ref().to_path_buf(), Some(content.to_owned()));
        self
    }

    pub fn expect_file_empty<P: AsRef<Path>>(&mut self, path: P) -> &mut Self {
        self.0.add_expected(path.as_ref().to_path_buf(), None);
        self
    }
}

impl TempEnv {
    pub fn run(
        setup: impl FnOnce(&mut Setup) -> &mut Setup,
        test: impl FnOnce(&Path),
        expect: impl FnOnce(&mut Expect) -> &mut Expect,
    ) {
        let dir = Self {
            root: TempDir::new("typst-test").unwrap(),
            found: BTreeMap::new(),
            expected: BTreeMap::new(),
        };

        let mut s = Setup(dir);
        setup(&mut s);
        let Setup(dir) = s;

        test(dir.root.path());

        let mut e = Expect(dir);
        expect(&mut e);
        let Expect(mut dir) = e;

        dir.collect();
        dir.assert();
    }

    pub fn run_no_check(setup: impl FnOnce(&mut Setup) -> &mut Setup, test: impl FnOnce(&Path)) {
        let dir = Self {
            root: TempDir::new("typst-test").unwrap(),
            found: BTreeMap::new(),
            expected: BTreeMap::new(),
        };

        let mut s = Setup(dir);
        setup(&mut s);
        let Setup(dir) = s;

        test(dir.root.path());
    }
}

impl TempEnv {
    fn add_expected(&mut self, expected: PathBuf, content: Option<Vec<u8>>) {
        for ancestor in expected.ancestors() {
            self.expected.insert(ancestor.to_path_buf(), None);
        }
        self.expected.insert(expected, content);
    }

    fn add_found(&mut self, found: PathBuf, content: Option<Vec<u8>>) {
        for ancestor in found.ancestors() {
            self.found.insert(ancestor.to_path_buf(), None);
        }
        self.found.insert(found, content);
    }

    fn read(&mut self, path: PathBuf) {
        let rel = path.strip_prefix(self.root.path()).unwrap().to_path_buf();
        if path.metadata().unwrap().is_file() {
            let content = std::fs::read(&path).unwrap();
            self.add_found(rel, Some(content));
        } else {
            let mut empty = true;
            for entry in path.read_dir().unwrap() {
                let entry = entry.unwrap();
                self.read(entry.path());
                empty = false;
            }

            if empty && self.root.path() != path {
                self.add_found(rel, None);
            }
        }
    }

    fn collect(&mut self) {
        self.read(self.root.path().to_path_buf())
    }

    fn assert(mut self) {
        let mut not_found = BTreeSet::new();
        let mut not_matched = BTreeMap::new();
        for (expected_path, expected_value) in self.expected {
            if let Some(found) = self.found.remove(&expected_path) {
                let expected = expected_value.unwrap_or_default();
                let found = found.unwrap_or_default();
                if expected != found {
                    not_matched.insert(expected_path, (found, expected));
                }
            } else {
                not_found.insert(expected_path);
            }
        }

        let not_expected: BTreeSet<_> = self.found.into_keys().collect();

        let mut mismatch = false;
        let mut msg = String::new();
        if !not_found.is_empty() {
            mismatch = true;
            writeln!(&mut msg, "\n=== Not found ===").unwrap();
            for not_found in not_found {
                writeln!(&mut msg, "/{}", not_found.display()).unwrap();
            }
        }

        if !not_expected.is_empty() {
            mismatch = true;
            writeln!(&mut msg, "\n=== Not expected ===").unwrap();
            for not_expected in not_expected {
                writeln!(&mut msg, "/{}", not_expected.display()).unwrap();
            }
        }

        if !not_matched.is_empty() {
            mismatch = true;
            writeln!(&mut msg, "\n=== Content matched ===").unwrap();
            for (path, (found, expected)) in not_matched {
                writeln!(&mut msg, "/{}", path.display()).unwrap();
                match (std::str::from_utf8(&found), std::str::from_utf8(&expected)) {
                    (Ok(found), Ok(expected)) => {
                        writeln!(&mut msg, "=== Expected ===\n>>>\n{}\n<<<\n", expected).unwrap();
                        writeln!(&mut msg, "=== Found ===\n>>>\n{}\n<<<\n", found).unwrap();
                    }
                    _ => {
                        writeln!(&mut msg, "Binary data differed").unwrap();
                    }
                }
            }
        }

        if mismatch {
            panic!("{msg}")
        }
    }
}
