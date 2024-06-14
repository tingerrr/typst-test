use std::path::Path;

use crate::test::id::Identifier;

pub mod v1;

#[derive(Debug)]
pub enum TestTarget {
    TestDir,
    TestScript,
    RefDir,
    RefScript,
    OutDir,
    DiffDir,
}

pub trait Project {
    /// The reserved path names for this project.
    const RESERVED: &'static [&'static str];

    /// Resolves the test root within a project.
    fn test_root(&self) -> &Path;

    /// Resolves a path within a project for the given test identifier and test target.
    fn resolve(&self, id: &Identifier, target: TestTarget) -> &Path;
}
