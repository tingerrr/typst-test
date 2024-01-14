use std::collections::HashSet;

use typst_manifest::Manifest;

use self::test::Test;

pub mod fs;
pub mod test;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScaffoldMode {
    WithExample,
    NoExample,
}

#[derive(Debug, Clone)]
pub struct Project {
    manifest: Option<Manifest>,
    loaded_tests: HashSet<Test>,
}

impl Project {
    pub fn new(manifest: Option<Manifest>) -> Self {
        Self {
            manifest,
            loaded_tests: HashSet::new(),
        }
    }

    pub fn name(&self) -> &str {
        self.manifest
            .as_ref()
            .map(|m| &m.package.name[..])
            .unwrap_or("<unknown package>")
    }

    pub fn manifest(&self) -> Option<&Manifest> {
        self.manifest.as_ref()
    }

    pub fn tests(&self) -> &HashSet<Test> {
        &self.loaded_tests
    }

    pub fn add_tests<I: IntoIterator<Item = Test>>(&mut self, tests: I) {
        self.loaded_tests.extend(tests)
    }
}
