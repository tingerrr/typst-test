pub mod fs {
    use std::fs::DirEntry;
    use std::io::ErrorKind;
    use std::path::{Path, PathBuf};
    use std::{fs, io};

    fn ignore_subset<T: Default>(
        result: io::Result<T>,
        check: impl FnOnce(&io::Error) -> io::Result<bool>,
    ) -> io::Result<T> {
        match result {
            Err(err) if check(&err)? => Ok(Default::default()),
            x => x,
        }
    }

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

        ignore_subset(inner(path.as_ref(), all), |e| {
            Ok(e.kind() == ErrorKind::AlreadyExists)
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

        ignore_subset(inner(path.as_ref(), all), |e| {
            Ok(if e.kind() == ErrorKind::NotFound {
                let parent_exists = path
                    .as_ref()
                    .parent()
                    .map(|p| p.try_exists())
                    .transpose()?
                    .is_some_and(|b| b);

                if !parent_exists {
                    tracing::error!(
                        path = ?path.as_ref(),
                        "tried removing dir, but parent did not exist",
                    );
                }

                parent_exists
            } else {
                false
            })
        })
    }

    pub fn create_empty_dir<P: AsRef<Path>>(path: P, all: bool) -> io::Result<()> {
        fn inner(path: &Path, all: bool) -> io::Result<()> {
            let res = remove_dir(path, true);
            if all {
                ignore_subset(res, |e| Ok(e.kind() == io::ErrorKind::NotFound))?;
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

        short
            .ancestors()
            .find(|ancestor| long.starts_with(ancestor))
    }

    pub fn is_ancestor_of<'a>(base: &'a Path, path: &'a Path) -> bool {
        common_ancestor(base, path).is_some_and(|a| a == base)
    }
}

pub mod term {
    use std::io::{self, IsTerminal};

    use termcolor::{ColorChoice, StandardStream};

    pub fn color_stream(color: clap::ColorChoice, stderr: bool) -> StandardStream {
        let choice = match color {
            clap::ColorChoice::Auto => {
                let stream_is_term = if stderr {
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

        if stderr {
            StandardStream::stderr(choice)
        } else {
            StandardStream::stdout(choice)
        }
    }
}
