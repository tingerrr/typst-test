//! Version control support. Contains a git and no-vcs implementation.

use std::fs::File;
use std::io;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};

use super::project::{Resolver, TestTarget};
use crate::test::id::Identifier;

/// A trait for version control systems, this is primarily used to ensure that
/// temporary storage directories are not tracked by the vcs.
pub trait Vcs {
    /// Ignore the given path within the project.
    fn ignore(&self, path: &Path) -> io::Result<()>;

    /// No longer ignore the given path within the project.
    fn unignore(&self, path: &Path) -> io::Result<()>;

    /// Ignore the given test target within the project.
    fn ignore_target(
        &self,
        project: &dyn Resolver,
        id: &Identifier,
        target: TestTarget,
    ) -> io::Result<()> {
        self.ignore(project.resolve(id, target))
    }

    /// No longer ignore the given test target within the project.
    fn unignore_target(
        &self,
        project: &dyn Resolver,
        id: &Identifier,
        target: TestTarget,
    ) -> io::Result<()> {
        self.unignore(project.resolve(id, target))
    }
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

    pub fn ensure_no_escape(&self, in_root: &Path) -> io::Result<()> {
        in_root.strip_prefix(&self.root).map_err(|_| {
            io::Error::new(io::ErrorKind::Other, "Cannot ignore paths outside root")
        })?;
        Ok(())
    }
}

const GITIGNORE_NAME: &str = ".gitignore";
const EXPECT_NOT_ROOT_MSG: &str = "cannot (un)ignore the root directory";

impl Vcs for Git {
    fn ignore(&self, path: &Path) -> io::Result<()> {
        let path = path.canonicalize()?;
        let is_dir = path.metadata()?.is_dir();
        let parent = path.parent().expect(EXPECT_NOT_ROOT_MSG);
        self.ensure_no_escape(parent)?;

        let gitignore = if is_dir {
            path.join(GITIGNORE_NAME)
        } else {
            parent.join(GITIGNORE_NAME)
        };

        let file = File::options()
            .read(true)
            .append(true)
            .create(true)
            .open(gitignore)?;

        let mut reader = BufReader::new(file);

        let rel = Path::new(path.file_name().expect(EXPECT_NOT_ROOT_MSG));
        let pattern = if is_dir { Path::new("**") } else { rel };

        let mut exists = false;
        for line in reader.by_ref().lines() {
            let line = line?;
            if pattern.as_os_str() == line.as_str() {
                exists = true;
                break;
            }
        }

        if !exists {
            // NOTE: we use to_str because the OsStr encoding is not stable and we assume it's Some as we realistically only write utf-8 path patterns into this file, otherwise BufRead::lines would fail later on anyway
            let mut buf = String::new();

            // we add a defensive newline to ensure
            buf.push('\n');
            buf.push_str(pattern.to_str().unwrap());
            buf.push('\n');
            reader.into_inner().write_all(buf.as_bytes())?;
        }

        Ok(())
    }

    fn unignore(&self, path: &Path) -> io::Result<()> {
        let path = path.canonicalize()?;
        let is_dir = path.metadata()?.is_dir();
        let parent = path.parent().expect(EXPECT_NOT_ROOT_MSG);
        self.ensure_no_escape(parent)?;

        let gitignore = if is_dir {
            path.join(GITIGNORE_NAME)
        } else {
            parent.join(GITIGNORE_NAME)
        };

        let file = match File::options().read(true).open(&gitignore) {
            Ok(file) => file,
            // if the file doesn't exist, we assume it's not ignored
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                return Ok(());
            }
            Err(err) => Err(err)?,
        };

        let rel = Path::new(path.file_name().expect(EXPECT_NOT_ROOT_MSG));
        let pattern = if is_dir { Path::new("**") } else { rel };

        let mut buf = String::new();
        for line in BufReader::new(file).lines() {
            let line = line?;
            if pattern.as_os_str() != line.as_str() {
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
            std::fs::remove_file(gitignore)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::_dev;

    #[test]
    fn test_git_escape_from_root() {
        _dev::fs::TempEnv::run(
            |root| root,
            |root| {
                let vcs = Git::new(root).unwrap();
                assert_eq!(vcs.ignore(root).unwrap_err().kind(), io::ErrorKind::Other);
            },
            |root| root,
        );
    }

    #[test]
    fn test_git_ignore_dir_no_file() {
        _dev::fs::TempEnv::run(
            |root| root.setup_dir("fancy/out"),
            |root| {
                let vcs = Git::new(root).unwrap();
                vcs.ignore(&root.join("fancy/out")).unwrap();
            },
            |root| {
                root.expect_dir("fancy/out")
                    .expect_file("fancy/out/.gitignore", b"\n**\n")
            },
        );
    }

    #[test]
    fn test_git_ignore_dir_append() {
        _dev::fs::TempEnv::run(
            |root| {
                root.setup_dir("fancy/out")
                    .setup_file("fancy/out/.gitignore", "ref.pdf")
            },
            |root| {
                let vcs = Git::new(root).unwrap();
                vcs.ignore(&root.join("fancy/out")).unwrap();
            },
            |root| {
                root.expect_dir("fancy/out")
                    .expect_file("fancy/out/.gitignore", b"ref.pdf\n**\n")
            },
        );
    }

    #[test]
    fn test_git_ignore_dir_no_op() {
        _dev::fs::TempEnv::run(
            |root| {
                root.setup_dir("fancy/out")
                    .setup_file("fancy/out/.gitignore", "**")
            },
            |root| {
                let vcs = Git::new(root).unwrap();
                vcs.ignore(&root.join("fancy/out")).unwrap();
            },
            |root| {
                root.expect_dir("fancy/out")
                    .expect_file("fancy/out/.gitignore", b"**")
            },
        );
    }

    #[test]
    fn test_git_ignore_file_no_file() {
        _dev::fs::TempEnv::run(
            |root| root.setup_file_empty("fancy/out.txt"),
            |root| {
                let vcs = Git::new(root).unwrap();
                vcs.ignore(&root.join("fancy/out.txt")).unwrap();
            },
            |root| {
                root.expect_dir("fancy/out.txt")
                    .expect_file("fancy/.gitignore", b"\nout.txt\n")
            },
        );
    }

    #[test]
    fn test_git_ignore_file_append() {
        _dev::fs::TempEnv::run(
            |root| {
                root.setup_file_empty("fancy/out.txt")
                    .setup_file("fancy/.gitignore", "ref.pdf")
            },
            |root| {
                let vcs = Git::new(root).unwrap();
                vcs.ignore(&root.join("fancy/out.txt")).unwrap();
            },
            |root| {
                root.expect_dir("fancy/out.txt")
                    .expect_file("fancy/.gitignore", b"ref.pdf\nout.txt\n")
            },
        );
    }

    #[test]
    fn test_git_ignore_file_no_op() {
        _dev::fs::TempEnv::run(
            |root| {
                root.setup_file_empty("fancy/out.txt")
                    .setup_file("fancy/.gitignore", "out.txt")
            },
            |root| {
                let vcs = Git::new(root).unwrap();
                vcs.ignore(&root.join("fancy/out.txt")).unwrap();
            },
            |root| {
                root.expect_dir("fancy/out.txt")
                    .expect_file("fancy/.gitignore", b"out.txt")
            },
        );
    }

    #[test]
    fn test_git_unignore_dir() {
        _dev::fs::TempEnv::run(
            |root| {
                root.setup_dir("fancy/out")
                    .setup_file("fancy/out/.gitignore", "ref.pdf\n**")
            },
            |root| {
                let vcs = Git::new(root).unwrap();
                vcs.unignore(&root.join("fancy/out")).unwrap();
            },
            |root| {
                root.expect_dir("fancy/out")
                    .expect_file("fancy/out/.gitignore", b"ref.pdf\n")
            },
        );
    }

    #[test]
    fn test_git_unignore_dir_remove_empty() {
        _dev::fs::TempEnv::run(
            |root| {
                root.setup_dir("fancy/out")
                    .setup_file("fancy/out/.gitignore", "**")
            },
            |root| {
                let vcs = Git::new(root).unwrap();
                vcs.unignore(&root.join("fancy/out")).unwrap();
            },
            |root| root.expect_dir("fancy/out"),
        );
    }

    #[test]
    fn test_git_unignore_dir_no_op() {
        _dev::fs::TempEnv::run(
            |root| {
                root.setup_dir("fancy/out")
                    .setup_file("fancy/out/.gitignore", "ref.pdf\n")
            },
            |root| {
                let vcs = Git::new(root).unwrap();
                vcs.unignore(&root.join("fancy/out")).unwrap();
            },
            |root| {
                root.expect_dir("fancy/out")
                    .expect_file("fancy/out/.gitignore", b"ref.pdf\n")
            },
        );
    }

    #[test]
    fn test_git_unignore_dir_no_file() {
        _dev::fs::TempEnv::run(
            |root| root.setup_dir("fancy/out"),
            |root| {
                let vcs = Git::new(root).unwrap();
                vcs.unignore(&root.join("fancy/out")).unwrap();
            },
            |root| root.expect_dir("fancy/out"),
        );
    }

    #[test]
    fn test_git_unignore_file() {
        _dev::fs::TempEnv::run(
            |root| {
                root.setup_file("fancy/.gitignore", "ref.pdf\nout.txt")
                    .setup_file_empty("fancy/out.txt")
            },
            |root| {
                let vcs = Git::new(root).unwrap();
                vcs.unignore(&root.join("fancy/out.txt")).unwrap();
            },
            |root| {
                root.expect_file("fancy/.gitignore", b"ref.pdf\n")
                    .expect_file_empty("fancy/out.txt")
            },
        );
    }

    #[test]
    fn test_git_unignore_file_remove_empty() {
        _dev::fs::TempEnv::run(
            |root| {
                root.setup_file("fancy/.gitignore", "out.txt")
                    .setup_file_empty("fancy/out.txt")
            },
            |root| {
                let vcs = Git::new(root).unwrap();
                vcs.unignore(&root.join("fancy/out.txt")).unwrap();
            },
            |root| root.expect_file_empty("fancy/out.txt"),
        );
    }

    #[test]
    fn test_git_unignore_file_no_op() {
        _dev::fs::TempEnv::run(
            |root| {
                root.setup_file_empty("fancy/out.txt")
                    .setup_file("fancy/.gitignore", "ref.pdf\n")
            },
            |root| {
                let vcs = Git::new(root).unwrap();
                vcs.unignore(&root.join("fancy/out.txt")).unwrap();
            },
            |root| {
                root.expect_file_empty("fancy/out.txt")
                    .expect_file("fancy/.gitignore", b"ref.pdf\n")
            },
        );
    }

    #[test]
    fn test_git_unignore_file_no_file() {
        _dev::fs::TempEnv::run(
            |root| root.setup_file_empty("fancy/out.txt"),
            |root| {
                let vcs = Git::new(root).unwrap();
                vcs.unignore(&root.join("fancy/out.txt")).unwrap();
            },
            |root| root.expect_dir("fancy/out.txt"),
        );
    }
}
