//! Version control support. Contains a default git implementation.

use std::fmt::{Debug, Display};
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::{fs, io};

use super::project::{Resolver, TestTarget};
use crate::test::id::Identifier;

/// A trait for version control systems, this is primarily used to ensure that
/// temporary storage directories are not tracked by the vcs.
pub trait Vcs: Debug + Display {
    /// Returns the root of the repository.
    fn root(&self) -> &Path;

    /// Ignore the given test target within the project.
    fn ignore(
        &self,
        resolver: &dyn Resolver,
        id: &Identifier,
        target: TestTarget,
    ) -> io::Result<()>;

    /// No longer ignore the given test target within the project.
    fn unignore(
        &self,
        resolver: &dyn Resolver,
        id: &Identifier,
        target: TestTarget,
    ) -> io::Result<()>;
}

/// A [`Vcs`] implementation for git. This will ignore paths by creating or
/// amending to a `.gitignore` file in the parent directory of a given path.
/// Edits by the user should be discouraged.
#[derive(Debug, Clone)]
pub struct Git {
    root: PathBuf,
}

impl Git {
    /// Creates a new git vcs abstraction with the given git root directory.
    pub fn new<P: Into<PathBuf>>(root: P) -> io::Result<Self> {
        Ok(Self {
            root: root.into().canonicalize()?,
        })
    }
}

impl Display for Git {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Git")
    }
}

const GITIGNORE_NAME: &str = ".gitignore";
const EXPECT_NOT_ESCAPED: &str = "cannot (un)ignore outside the git root";

impl Vcs for Git {
    fn root(&self) -> &Path {
        &self.root
    }

    fn ignore(
        &self,
        resolver: &dyn Resolver,
        id: &Identifier,
        target: TestTarget,
    ) -> io::Result<()> {
        let path = resolver.resolve(id, target);
        let parent = path.parent().expect(EXPECT_NOT_ESCAPED);

        let gitignore = parent.join(GITIGNORE_NAME);

        let file = File::options()
            .read(true)
            .append(true)
            .create(true)
            .open(gitignore)?;

        let mut reader = BufReader::new(file);

        // NOTE: we know that the file names created by the resolver must be
        // valid UTF-8
        let target = path.file_name().unwrap().to_str().unwrap();

        let mut exists = false;
        for line in reader.by_ref().lines() {
            let line = line?;
            if target == line {
                exists = true;
                break;
            }
        }

        if !exists {
            let mut buf = String::new();

            // we add a defensive newline to ensure
            buf.push('\n');
            buf.push_str(target);
            buf.push('\n');
            reader.into_inner().write_all(buf.as_bytes())?;
        }

        Ok(())
    }

    fn unignore(
        &self,
        resolver: &dyn Resolver,
        id: &Identifier,
        target: TestTarget,
    ) -> io::Result<()> {
        let path = resolver.resolve(id, target);
        let parent = path.parent().expect(EXPECT_NOT_ESCAPED);

        let gitignore = parent.join(GITIGNORE_NAME);

        let file = match File::options().read(true).open(&gitignore) {
            Ok(file) => file,
            // if the file doesn't exist, we assume it's not ignored
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                return Ok(());
            }
            Err(err) => Err(err)?,
        };

        // NOTE: see above for unwraps
        let pattern = path.file_name().unwrap().to_str().unwrap();

        let mut buf = String::new();
        for line in BufReader::new(file).lines() {
            let line = line?;
            if pattern != line {
                buf.push_str(&line);
                buf.push('\n');
            }
        }

        // if the buffer is empty we removed the last pattern, remove the empty file
        if !buf.trim().is_empty() {
            File::options()
                .write(true)
                .truncate(true)
                .open(&gitignore)?
                .write_all(buf.as_bytes())?;
        } else {
            fs::remove_file(gitignore)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::_dev;
    use crate::store::project::v1::ResolverV1;

    #[test]
    fn test_git_ignore_dir_non_existent() {
        _dev::fs::TempEnv::run(
            |root| root.setup_dir("tests/fancy"),
            |root| {
                let resolver = ResolverV1::new(root, "tests");
                let vcs = Git::new(root).unwrap();
                vcs.ignore(
                    &resolver,
                    &Identifier::new("fancy").unwrap(),
                    TestTarget::OutDir,
                )
                .unwrap();
            },
            |root| root.expect_file("tests/fancy/.gitignore", b"\nout\n"),
        );
    }

    #[test]
    fn test_git_ignore_dir_no_file() {
        _dev::fs::TempEnv::run(
            |root| root.setup_dir("tests/fancy/out"),
            |root| {
                let resolver = ResolverV1::new(root, "tests");
                let vcs = Git::new(root).unwrap();
                vcs.ignore(
                    &resolver,
                    &Identifier::new("fancy").unwrap(),
                    TestTarget::OutDir,
                )
                .unwrap();
            },
            |root| {
                root.expect_dir("tests/fancy/out")
                    .expect_file("tests/fancy/.gitignore", b"\nout\n")
            },
        );
    }

    #[test]
    fn test_git_ignore_dir_append() {
        _dev::fs::TempEnv::run(
            |root| {
                root.setup_dir("tests/fancy/out")
                    .setup_file("tests/fancy/.gitignore", "ref.pdf")
            },
            |root| {
                let resolver = ResolverV1::new(root, "tests");
                let vcs = Git::new(root).unwrap();
                vcs.ignore(
                    &resolver,
                    &Identifier::new("fancy").unwrap(),
                    TestTarget::OutDir,
                )
                .unwrap();
            },
            |root| {
                root.expect_dir("tests/fancy/out")
                    .expect_file("tests/fancy/.gitignore", b"ref.pdf\nout\n")
            },
        );
    }

    #[test]
    fn test_git_ignore_dir_no_op() {
        _dev::fs::TempEnv::run(
            |root| {
                root.setup_dir("tests/fancy/out")
                    .setup_file("tests/fancy/.gitignore", "out")
            },
            |root| {
                let resolver = ResolverV1::new(root, "tests");
                let vcs = Git::new(root).unwrap();
                vcs.ignore(
                    &resolver,
                    &Identifier::new("fancy").unwrap(),
                    TestTarget::OutDir,
                )
                .unwrap();
            },
            |root| {
                root.expect_dir("tests/fancy/out")
                    .expect_file("tests/fancy/.gitignore", b"out")
            },
        );
    }

    #[test]
    fn test_git_ignore_file_non_existent() {
        _dev::fs::TempEnv::run(
            |root| {
                root.setup_file_empty("tests/fancy/ref.typ")
                    .setup_file("tests/fancy/.gitignore", b"ref.pdf")
            },
            |root| {
                let resolver = ResolverV1::new(root, "tests");
                let vcs = Git::new(root).unwrap();
                vcs.ignore(
                    &resolver,
                    &Identifier::new("fancy").unwrap(),
                    TestTarget::RefScript,
                )
                .unwrap();
            },
            |root| {
                root.expect_file_empty("tests/fancy/ref.typ")
                    .expect_file("tests/fancy/.gitignore", b"ref.pdf\nref.typ\n")
            },
        );
    }

    #[test]
    fn test_git_ignore_file_no_ignore_file() {
        _dev::fs::TempEnv::run(
            |root| root.setup_file_empty("tests/fancy/ref.typ"),
            |root| {
                let resolver = ResolverV1::new(root, "tests");
                let vcs = Git::new(root).unwrap();
                vcs.ignore(
                    &resolver,
                    &Identifier::new("fancy").unwrap(),
                    TestTarget::RefScript,
                )
                .unwrap();
            },
            |root| {
                root.expect_file_empty("tests/fancy/ref.typ")
                    .expect_file("tests/fancy/.gitignore", b"\nref.typ\n")
            },
        );
    }

    #[test]
    fn test_git_ignore_file_append() {
        _dev::fs::TempEnv::run(
            |root| {
                root.setup_file_empty("tests/fancy/ref.typ")
                    .setup_file("tests/fancy/.gitignore", "ref.pdf")
            },
            |root| {
                let resolver = ResolverV1::new(root, "tests");
                let vcs = Git::new(root).unwrap();
                vcs.ignore(
                    &resolver,
                    &Identifier::new("fancy").unwrap(),
                    TestTarget::RefScript,
                )
                .unwrap();
            },
            |root| {
                root.expect_file_empty("tests/fancy/ref.typ")
                    .expect_file("tests/fancy/.gitignore", b"ref.pdf\nref.typ\n")
            },
        );
    }

    #[test]
    fn test_git_ignore_file_no_op() {
        _dev::fs::TempEnv::run(
            |root| {
                root.setup_file_empty("tests/fancy/ref.typ")
                    .setup_file("tests/fancy/.gitignore", "ref.typ")
            },
            |root| {
                let resolver = ResolverV1::new(root, "tests");
                let vcs = Git::new(root).unwrap();
                vcs.ignore(
                    &resolver,
                    &Identifier::new("fancy").unwrap(),
                    TestTarget::RefScript,
                )
                .unwrap();
            },
            |root| {
                root.expect_file_empty("tests/fancy/ref.typ")
                    .expect_file("tests/fancy/.gitignore", b"ref.typ")
            },
        );
    }

    #[test]
    fn test_git_unignore_dir_non_existent() {
        _dev::fs::TempEnv::run(
            |root| root.setup_dir("tests/fancy"),
            |root| {
                let resolver = ResolverV1::new(root, "tests");
                let vcs = Git::new(root).unwrap();
                vcs.unignore(
                    &resolver,
                    &Identifier::new("fancy").unwrap(),
                    TestTarget::OutDir,
                )
                .unwrap();
            },
            |root| root.expect_dir("tests/fancy"),
        );
    }

    #[test]
    fn test_git_unignore_dir() {
        _dev::fs::TempEnv::run(
            |root| {
                root.setup_dir("tests/fancy/out")
                    .setup_file("tests/fancy/.gitignore", "ref.pdf\nout")
            },
            |root| {
                let resolver = ResolverV1::new(root, "tests");
                let vcs = Git::new(root).unwrap();
                vcs.unignore(
                    &resolver,
                    &Identifier::new("fancy").unwrap(),
                    TestTarget::OutDir,
                )
                .unwrap();
            },
            |root| {
                root.expect_dir("tests/fancy/out")
                    .expect_file("tests/fancy/.gitignore", b"ref.pdf\n")
            },
        );
    }

    #[test]
    fn test_git_unignore_dir_remove_empty() {
        _dev::fs::TempEnv::run(
            |root| {
                root.setup_dir("tests/fancy/out")
                    .setup_file("tests/fancy/.gitignore", "out")
            },
            |root| {
                let resolver = ResolverV1::new(root, "tests");
                let vcs = Git::new(root).unwrap();
                vcs.unignore(
                    &resolver,
                    &Identifier::new("fancy").unwrap(),
                    TestTarget::OutDir,
                )
                .unwrap();
            },
            |root| root.expect_dir("tests/fancy/out"),
        );
    }

    #[test]
    fn test_git_unignore_dir_no_op() {
        _dev::fs::TempEnv::run(
            |root| {
                root.setup_dir("tests/fancy/out")
                    .setup_file("tests/fancy/.gitignore", "ref.pdf\n")
            },
            |root| {
                let resolver = ResolverV1::new(root, "tests");
                let vcs = Git::new(root).unwrap();
                vcs.unignore(
                    &resolver,
                    &Identifier::new("fancy").unwrap(),
                    TestTarget::OutDir,
                )
                .unwrap();
            },
            |root| {
                root.expect_dir("tests/fancy/out")
                    .expect_file("tests/fancy/.gitignore", b"ref.pdf\n")
            },
        );
    }

    #[test]
    fn test_git_unignore_dir_no_ignore_file() {
        _dev::fs::TempEnv::run(
            |root| root.setup_dir("tests/fancy/out"),
            |root| {
                let resolver = ResolverV1::new(root, "tests");
                let vcs = Git::new(root).unwrap();
                vcs.unignore(
                    &resolver,
                    &Identifier::new("fancy").unwrap(),
                    TestTarget::OutDir,
                )
                .unwrap();
            },
            |root| root.expect_dir("tests/fancy/out"),
        );
    }

    #[test]
    fn test_git_unignore_file_non_existent() {
        _dev::fs::TempEnv::run(
            |root| root.setup_file("tests/fancy/.gitignore", "ref.pdf\nref.typ"),
            |root| {
                let resolver = ResolverV1::new(root, "tests");
                let vcs = Git::new(root).unwrap();
                vcs.unignore(
                    &resolver,
                    &Identifier::new("fancy").unwrap(),
                    TestTarget::RefScript,
                )
                .unwrap();
            },
            |root| root.expect_file("tests/fancy/.gitignore", b"ref.pdf\n"),
        );
    }

    #[test]
    fn test_git_unignore_file() {
        _dev::fs::TempEnv::run(
            |root| {
                root.setup_file("tests/fancy/.gitignore", "ref.pdf\nref.typ")
                    .setup_file_empty("tests/fancy/ref.typ")
            },
            |root| {
                let resolver = ResolverV1::new(root, "tests");
                let vcs = Git::new(root).unwrap();
                vcs.unignore(
                    &resolver,
                    &Identifier::new("fancy").unwrap(),
                    TestTarget::RefScript,
                )
                .unwrap();
            },
            |root| {
                root.expect_file("tests/fancy/.gitignore", b"ref.pdf\n")
                    .expect_file_empty("tests/fancy/ref.typ")
            },
        );
    }

    #[test]
    fn test_git_unignore_file_remove_empty() {
        _dev::fs::TempEnv::run(
            |root| {
                root.setup_file("tests/fancy/.gitignore", "ref.typ")
                    .setup_file_empty("tests/fancy/ref.typ")
            },
            |root| {
                let resolver = ResolverV1::new(root, "tests");
                let vcs = Git::new(root).unwrap();
                vcs.unignore(
                    &resolver,
                    &Identifier::new("fancy").unwrap(),
                    TestTarget::RefScript,
                )
                .unwrap();
            },
            |root| root.expect_file_empty("tests/fancy/ref.typ"),
        );
    }

    #[test]
    fn test_git_unignore_file_no_op() {
        _dev::fs::TempEnv::run(
            |root| {
                root.setup_file_empty("tests/fancy/ref.typ")
                    .setup_file("tests/fancy/.gitignore", "ref.pdf\n")
            },
            |root| {
                let resolver = ResolverV1::new(root, "tests");
                let vcs = Git::new(root).unwrap();
                vcs.unignore(
                    &resolver,
                    &Identifier::new("fancy").unwrap(),
                    TestTarget::RefScript,
                )
                .unwrap();
            },
            |root| {
                root.expect_file_empty("tests/fancy/ref.typ")
                    .expect_file("tests/fancy/.gitignore", b"ref.pdf\n")
            },
        );
    }

    #[test]
    fn test_git_unignore_file_no_ignore_file() {
        _dev::fs::TempEnv::run(
            |root| root.setup_file_empty("tests/fancy/ref.typ"),
            |root| {
                let resolver = ResolverV1::new(root, "tests");
                let vcs = Git::new(root).unwrap();
                vcs.unignore(
                    &resolver,
                    &Identifier::new("fancy").unwrap(),
                    TestTarget::RefScript,
                )
                .unwrap();
            },
            |root| root.expect_file_empty("tests/fancy/ref.typ"),
        );
    }
}
