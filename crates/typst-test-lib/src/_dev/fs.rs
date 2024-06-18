macro_rules! assert_tmp_dir {
    ($root:expr; $($path:literal $(=> $content:expr)?),* $(,)?) => {
        let mut dir = $crate::_dev::fs::Comparer::new($root);

        $(
            #[allow(unused_mut)]
            #[allow(unused_assignments)]
            let mut content = None;
            $(content = Some($content[..].to_owned());)?

            dir.add_expected(::std::path::Path::new($path).to_path_buf(), content);
        )*

        dir.collect();
        dir.assert();
    };
}

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::path::PathBuf;

pub(crate) use assert_tmp_dir;
use tempdir::TempDir;

pub struct Comparer {
    root: TempDir,
    found: BTreeMap<PathBuf, Option<Vec<u8>>>,
    expected: BTreeMap<PathBuf, Option<Vec<u8>>>,
}

impl Comparer {
    pub fn new(root: TempDir) -> Self {
        Self {
            root,
            found: BTreeMap::new(),
            expected: BTreeMap::new(),
        }
    }

    pub fn add_expected(&mut self, expected: PathBuf, content: Option<Vec<u8>>) {
        for ancestor in expected.ancestors() {
            self.expected.insert(ancestor.to_path_buf(), None);
        }
        self.expected.insert(expected, content);
    }

    pub fn add_found(&mut self, found: PathBuf, content: Option<Vec<u8>>) {
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

            if empty {
                self.add_found(rel, None);
            }
        }
    }

    pub fn collect(&mut self) {
        self.read(self.root.path().to_path_buf())
    }

    pub fn assert(mut self) {
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
