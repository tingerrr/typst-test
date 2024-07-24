//! Hooks are scripts or programs which are run before or after tests. There are
//! currently four hooks:
//! - prepare: run once before all tests
//! - cleanup: run once after all tests
//! - prepare-each: run once before each test, receives the test directory as
//!   its first argument
//! - cleanup-each: run once after each test, receives the test directory as
//!   its first argument
//!
//! All hooks have the test root set as their working directory.
//! When a hook returns non-zero exist status the test it was run for is
//! considered a failure.

use std::io;
use std::path::Path;
use std::process::{Command, Output};

use thiserror::Error;

use crate::store::project::{Resolver, TestTarget};
use crate::test::id::Identifier;

#[derive(Debug, Error)]
pub enum Error {
    /// The hook returned a failure exit code.
    #[error("the hook did not run successfully (exit code: {:?})", .0.status)]
    Hook(Output),

    /// An io error occurred.
    #[error("an io error occurred")]
    Io(#[from] io::Error),
}

/// Creates a [`Command`] to a hook, if a test is passed, its directory path is
/// resolved and passed as the first and only argument. The test root is set as
/// the working directory.
///
/// All paths are canonicalized, if this fails an error is returned.
pub fn prepare<R: Resolver>(
    path: &Path,
    test: Option<&Identifier>,
    resolver: &R,
) -> io::Result<Command> {
    let mut cmd = Command::new(path.canonicalize()?);

    cmd.current_dir(resolver.test_root().canonicalize()?);

    if let Some(id) = test {
        cmd.arg(resolver.resolve(id, TestTarget::TestDir).canonicalize()?);
    }

    Ok(cmd)
}

/// Runs a hook to completion, collecting it's output if there was an error.
pub fn run<R: Resolver>(path: &Path, test: Option<&Identifier>, resolver: &R) -> Result<(), Error> {
    let output = prepare(path, test, resolver)?.output()?;

    if !output.status.success() {
        return Err(Error::Hook(output));
    }

    Ok(())
}

// TODO: tests
