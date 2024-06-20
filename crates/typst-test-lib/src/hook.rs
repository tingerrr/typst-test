//! Hooks are scripts or programs which are run before or after tests. There are
//! currently four hooks:
//! - prepare: run once before all tests
//! - cleanup: run once after all tests
//! - prepare-each: run once before each test, receives the test directory as
//!   it's first argument
//! - cleanup-each: run once after each test, receives the test directory as
//!   it's first argument
//!
//! All hooks have the test root set as their working directory.
//! When a hook returns non-zero exist status the test it was run for is
//! considered failed.

use std::io;
use std::path::Path;
use std::process::{Command, Output};

use crate::store::project::{Project, TestTarget};
use crate::test::id::Identifier;

/// Creates a [`Command`] to a hook, if a test is passed it's directory path is resolved
/// and passed as an argument. The test root is set as the working directory.
pub fn prepare<P: Project>(path: &Path, test: Option<&Identifier>, project: &P) -> Command {
    let mut cmd = Command::new(path);

    cmd.current_dir(project.test_root());

    if let Some(id) = test {
        cmd.arg(project.resolve(id, TestTarget::TestDir));
    }

    cmd
}

/// Runs a hook to completion, collecting it's output.
pub fn run<P: Project>(path: &Path, test: Option<&Identifier>, project: &P) -> io::Result<Output> {
    prepare(path, test, project).output()
}

// TODO: tests
