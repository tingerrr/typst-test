use std::path::Path;

use crate::test::id::Identifier;

pub mod v1;

/// The target a [`Project`] must resolve for a given [`Identifier`].
#[derive(Debug)]
pub enum TestTarget {
    /// The test directory, this generally corresponds to the path matching the
    /// identifier of a test rooted in the test root directory.
    TestDir,

    /// The test script.
    TestScript,

    /// The optional reference store directory, if this test is ephemeral this
    /// is a temporary directory, otherwise it's a persistent store directory.
    RefDir,

    /// The optional reference script for ephemeral tests.
    RefScript,

    /// The temporary output directory, this stores the output of the test
    /// script.
    OutDir,

    /// The temporary diff directory, this stores generated diffs bwteeen test
    /// and reference output.
    DiffDir,
}

/// A type which resolves and stores commonly accessed paths to tests of a
/// project.
pub trait Resolver {
    /// The reserved path names for this project.
    const RESERVED: &'static [&'static str];

    /// Returns the project root.
    fn project_root(&self) -> &Path;

    /// Resolves the test root within a project.
    fn test_root(&self) -> &Path;

    /// Resolves a path within a project for the given test identifier and test target.
    fn resolve(&self, id: &Identifier, target: TestTarget) -> &Path;
}
