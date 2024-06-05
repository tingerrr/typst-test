pub mod fmt {
    use std::fmt::Display;

    struct Plural<'a> {
        term: &'a str,
        is_singular: bool,
    }

    impl Display for Plural<'_> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                f,
                "{}{}",
                self.term,
                if self.is_singular { "" } else { "s" }
            )
        }
    }

    pub fn plural(count: usize, term: &str) -> impl Display + '_ {
        Plural {
            term,
            is_singular: count == 1,
        }
    }
}

pub mod result {
    pub fn ignore<T: Default, E>(
        result: Result<T, E>,
        check: impl FnOnce(&E) -> bool,
    ) -> Result<T, E> {
        ignore_with(result, check, |_| T::default())
    }

    pub fn ignore_with<T, E>(
        result: Result<T, E>,
        check: impl FnOnce(&E) -> bool,
        value: impl FnOnce(&E) -> T,
    ) -> Result<T, E> {
        match result {
            Err(err) if check(&err) => Ok(value(&err)),
            x => x,
        }
    }
}

pub mod fs {
    use std::fs::DirEntry;
    use std::io::ErrorKind;
    use std::path::{Path, PathBuf};
    use std::{fs, io};

    use super::result;

    pub fn path_in_root<P, I, T>(root: P, parts: I) -> PathBuf
    where
        P: AsRef<Path>,
        I: IntoIterator<Item = T>,
        T: AsRef<Path>,
    {
        let root: &Path = root.as_ref();
        let mut result = root.to_path_buf();
        result.extend(parts);

        debug_assert!(
            is_ancestor_of(root, &result),
            "unintended escape from root, {result:?} is not inside {root:?}"
        );
        result
    }

    pub fn collect_dir_entries<P: AsRef<Path>>(path: P) -> io::Result<Vec<DirEntry>> {
        fs::read_dir(path)?.collect::<io::Result<Vec<DirEntry>>>()
    }

    pub fn create_dir<P: AsRef<Path>>(path: P, all: bool) -> io::Result<()> {
        fn inner(path: &Path, all: bool) -> io::Result<()> {
            if all {
                fs::create_dir_all(path)
            } else {
                fs::create_dir(path)
            }
        }

        result::ignore(inner(path.as_ref(), all), |e| {
            e.kind() == ErrorKind::AlreadyExists
        })
    }

    pub fn remove_dir<P: AsRef<Path>>(path: P, all: bool) -> io::Result<()> {
        fn inner(path: &Path, all: bool) -> io::Result<()> {
            if all {
                fs::remove_dir_all(path)
            } else {
                fs::remove_dir(path)
            }
        }

        let path = path.as_ref();

        result::ignore(inner(path, all), |e| {
            if e.kind() == ErrorKind::NotFound {
                let parent_exists = path
                    .parent()
                    .and_then(|p| p.try_exists().ok())
                    .is_some_and(|b| b);

                if !parent_exists {
                    tracing::error!(?path, "tried removing dir, but parent did not exist");
                }

                parent_exists
            } else {
                false
            }
        })
    }

    pub fn create_empty_dir<P: AsRef<Path>>(path: P, all: bool) -> io::Result<()> {
        fn inner(path: &Path, all: bool) -> io::Result<()> {
            let res = remove_dir(path, true);
            if all {
                // if there was nothing to clear, then we simply go on to creation
                result::ignore(res, |e| e.kind() == io::ErrorKind::NotFound)?;
            } else {
                res?;
            }
            create_dir(path, all)
        }

        inner(path.as_ref(), all)
    }

    pub fn common_ancestor<'a>(p: &'a Path, q: &'a Path) -> Option<&'a Path> {
        let mut paths = [p, q];
        paths.sort_by_key(|p| p.as_os_str().len());
        let [short, long] = paths;

        // find the longest match where long starts with short
        short.ancestors().find(|a| long.starts_with(a))
    }

    pub fn is_ancestor_of<'a>(base: &'a Path, path: &'a Path) -> bool {
        common_ancestor(base, path).is_some_and(|ca| ca == base)
    }
}

pub mod term {
    use std::io::{self, IsTerminal};

    use termcolor::{ColorChoice, StandardStream};

    pub fn color_stream(color: clap::ColorChoice, is_stderr: bool) -> StandardStream {
        let choice = match color {
            clap::ColorChoice::Auto => {
                let stream_is_term = if is_stderr {
                    io::stderr().is_terminal()
                } else {
                    io::stdout().is_terminal()
                };

                if stream_is_term {
                    ColorChoice::Auto
                } else {
                    ColorChoice::Never
                }
            }
            clap::ColorChoice::Always => ColorChoice::Always,
            clap::ColorChoice::Never => ColorChoice::Never,
        };

        if is_stderr {
            StandardStream::stderr(choice)
        } else {
            StandardStream::stdout(choice)
        }
    }

    pub fn color(color: clap::ColorChoice, is_stderr: bool) -> bool {
        match color {
            clap::ColorChoice::Auto => {
                if is_stderr {
                    io::stderr().is_terminal()
                } else {
                    io::stdout().is_terminal()
                }
            }
            clap::ColorChoice::Always => true,
            clap::ColorChoice::Never => false,
        }
    }
}

pub mod command {
    use std::ffi::OsStr;
    use std::process::Command;

    pub fn parse_stdout<R>(
        program: impl AsRef<OsStr>,
        args: &[&str],
        f: impl FnOnce(String) -> R,
    ) -> anyhow::Result<R> {
        let mut cmd = Command::new(program.as_ref());
        cmd.args(args);
        let output = cmd.output()?;
        if !output.status.success() {
            anyhow::bail!("failed to run {:?} {args:?}", program.as_ref());
        }
        Ok(f(String::from_utf8(output.stdout)?))
    }
}
