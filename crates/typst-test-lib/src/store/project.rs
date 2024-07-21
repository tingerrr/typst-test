//! Project and test path resolving.

use std::path::Path;

use crate::test::id::Identifier;

pub mod v1;

/// The target a [`Resolver`] must resolve for a given [`Identifier`].
#[derive(Debug)]
pub enum TestTarget {
    /// The test directory, this generally corresponds to the path matching the
    /// identifier of a test rooted in the test root directory.
    TestDir,

    /// The test script, usually located within the test directory as a
    /// `test.typ` file.
    TestScript,

    /// The optional reference store directory.
    ///
    /// - If a test is ephemeral, this is a temporary directory
    /// - If a test is persistent, this is a persistent store directory
    /// - If a test is compile-only, this directory doesn't exist
    RefDir,

    /// The optional reference script for ephemeral tests. Does not exist for
    /// persistent or compile-only tests.
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
    /// The reserved path names for this project, these will be ignored when
    /// loading.
    const RESERVED: &'static [&'static str];

    /// Returns the project root.
    fn project_root(&self) -> &Path;

    /// Resolves the test root within the project.
    fn test_root(&self) -> &Path;

    /// Resolves a path within the project for the given test identifier and
    /// target.
    fn resolve(&self, id: &Identifier, target: TestTarget) -> &Path;
}
