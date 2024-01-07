use std::collections::HashSet;

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
    name: String,
    loaded_tests: HashSet<Test>,
}

impl Project {
    pub fn new(name: String) -> Self {
        Self {
            name,
            loaded_tests: HashSet::new(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn tests(&self) -> &HashSet<Test> {
        &self.loaded_tests
    }

    pub fn add_tests<I: IntoIterator<Item = Test>>(&mut self, tests: I) {
        self.loaded_tests.extend(tests)
    }

    pub fn filter_tests(&self, filter: &str) -> HashSet<&Test> {
        self.loaded_tests
            .iter()
            .filter(|t| t.name().contains(filter))
            .collect()
    }
}
