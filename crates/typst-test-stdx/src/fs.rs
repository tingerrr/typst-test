//! Helper functions and types for managing and manipulating the filesystem.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::{fs, io};

use tempdir::TempDir;

use crate::result::ResultEx;

/// Creates a new directory and its parent directories if `all` is specified,
/// but doesn't fail if it already exists.
///
/// # Example
/// ```no_run
/// # use typst_test_stdx::fs::create_dir;
/// create_dir("foo", true)?;
/// create_dir("foo", true)?; // second time doesn't fail
/// # Ok::<_, Box<dyn std::error::Error>>(())
/// ```
pub fn create_dir<P>(path: P, all: bool) -> io::Result<()>
where
    P: AsRef<Path>,
{
    fn inner(path: &Path, all: bool) -> io::Result<()> {
        let res = if all {
            fs::create_dir_all(path)
        } else {
            fs::create_dir(path)
        };
        res.ignore_default(|e| e.kind() == ErrorKind::AlreadyExists)
    }

    inner(path.as_ref(), all)
}

/// Removes a file, but doesn't fail if it doens't exist.
///
/// # Example
/// ```no_run
/// # use typst_test_stdx::fs::remove_file;
/// remove_file("foo.txt")?;
/// remove_file("foo.txt")?; // second time doesn't fail
/// # Ok::<_, Box<dyn std::error::Error>>(())
/// ```
pub fn remove_file<P>(path: P) -> io::Result<()>
where
    P: AsRef<Path>,
{
    fn inner(path: &Path) -> io::Result<()> {
        std::fs::remove_file(path).ignore_default(|e| e.kind() == ErrorKind::NotFound)
    }

    inner(path.as_ref())
}

/// Removes a directory, but doesn't fail if it doens't exist.
///
/// # Example
/// ```no_run
/// # use typst_test_stdx::fs::remove_dir;
/// remove_dir("foo", true)?;
/// remove_dir("foo", true)?; // second time doesn't fail
/// # Ok::<_, Box<dyn std::error::Error>>(())
/// ```
pub fn remove_dir<P>(path: P, all: bool) -> io::Result<()>
where
    P: AsRef<Path>,
{
    fn inner(path: &Path, all: bool) -> io::Result<()> {
        let res = if all {
            fs::remove_dir_all(path)
        } else {
            fs::remove_dir(path)
        };
        res.ignore_default(|e| {
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

    inner(path.as_ref(), all)
}

/// Creates an empty directory, removing any content if it exists. The `all`
/// argument is passed through to [`create_dir`].
///
/// # Example
/// ```no_run
/// # use typst_test_stdx::fs::create_empty_dir;
/// create_empty_dir("foo", true)?;
/// # Ok::<_, Box<dyn std::error::Error>>(())
/// ```
pub fn create_empty_dir<P>(path: P, all: bool) -> io::Result<()>
where
    P: AsRef<Path>,
{
    fn inner(path: &Path, all: bool) -> io::Result<()> {
        let res = remove_dir(path, true);
        if all {
            // if there was nothing to clear, then we simply go on to creation
            res.ignore_default(|e| e.kind() == io::ErrorKind::NotFound)?;
        } else {
            res?;
        }
        create_dir(path, all)
    }

    inner(path.as_ref(), all)
}

/// Returns the lexical common ancestor of two paths if there is any.
///
/// # Example
/// ```no_run
/// # use std::path::Path;
/// # use typst_test_stdx::fs::common_ancestor;
/// assert_eq!(
///     common_ancestor(Path::new("foo/bar"), Path::new("foo/baz")),
///     Some(Path::new("foo")),
/// );
/// assert_eq!(
///     common_ancestor(Path::new("foo/bar"), Path::new("/foo/baz")),
///     None,
/// );
/// # Ok::<_, Box<dyn std::error::Error>>(())
/// ```
pub fn common_ancestor<'a>(p: &'a Path, q: &'a Path) -> Option<&'a Path> {
    let mut paths = [p, q];
    paths.sort_by_key(|p| p.as_os_str().len());
    let [short, long] = paths;

    // find the longest match where long starts with short
    short.ancestors().find(|a| long.starts_with(a))
}

/// Returns whether `base` is an ancestor of `path` lexically.
///
/// # Example
/// ```no_run
/// # use typst_test_stdx::fs::is_ancestor_of;
/// assert_eq!(is_ancestor_of("foo/", "foo/baz"), true);
/// assert_eq!(is_ancestor_of("foo/", "/foo/baz"), false);
/// # Ok::<_, Box<dyn std::error::Error>>(())
/// ```
pub fn is_ancestor_of<P: AsRef<Path>, Q: AsRef<Path>>(base: P, path: Q) -> bool {
    fn inner(base: &Path, path: &Path) -> bool {
        common_ancestor(base, path).is_some_and(|ca| ca == base)
    }

    inner(base.as_ref(), path.as_ref())
}

/// Manages a temporary directory and the expected and found directories for
/// testing file system manipulation. This will prepare and assert the state of
/// a temporary directory structure.
///
/// # Examples
/// ```no_run
/// # use typst_test_stdx::fs::TempEnv;
/// TempEnv::run(
///     |test| {
///         // prepare the test directory sructure
///         // creates an empty files and all their parents
///         test.setup_file_empty("foo/bar/empty.txt")
///             .setup_file_empty("foo/baz/other.txt")
///     },
///     |root| {
///         // here we test our code
///         std::fs::remove_file(root.join("foo/bar/empty.txt")).unwrap();
///     },
///     |test| {
///         // assert the structure after the test
///         // ensure there is a directory
///         test.expect_dir("foo/bar/")
///
///         // assertion panics because we didn't assert foo/baz/other.txt
///     },
/// );
/// ```
pub struct TempEnv {
    root: TempDir,
    found: BTreeMap<PathBuf, Option<Vec<u8>>>,
    expected: BTreeMap<PathBuf, Option<Vec<u8>>>,
}

/// Set up the temporary directory structure. This is passed to the first
/// closure in [`TempEnv::run`].
pub struct Setup(TempEnv);

impl Setup {
    /// Create a directory at the given path below the test root.
    ///
    /// # Panics
    /// Panics if the given path lexically escapes the temporary root.
    ///
    /// # Examples
    /// ```no_run
    /// # use typst_test_stdx::fs::TempEnv;
    /// TempEnv::run(
    ///     |test| test.setup_dir("foo/bar/"),
    ///     // test ...
    ///     // assertion ...
    /// #    |root| {},
    /// #    |test| test,
    /// );
    /// ```
    pub fn setup_dir<P: AsRef<Path>>(&mut self, path: P) -> &mut Self {
        let abs_path = self.0.root.path().join(path.as_ref());
        assert!(is_ancestor_of(self.0.root.path(), &abs_path));

        create_dir(abs_path, true).unwrap();
        self
    }

    /// Create a directory at the given path below the test root.
    ///
    /// # Panics
    /// Panics if the given path lexically escapes the temporary root.
    ///
    /// # Examples
    /// ```no_run
    /// # use typst_test_stdx::fs::TempEnv;
    /// TempEnv::run(
    ///     |test| test.setup_file("foo/bar/empty.txt", b"Hello World\n"),
    ///     // test ...
    ///     // assertion ...
    /// #    |root| {},
    /// #    |test| test,
    /// );
    /// ```
    pub fn setup_file<P: AsRef<Path>>(&mut self, path: P, content: impl AsRef<[u8]>) -> &mut Self {
        let abs_path = self.0.root.path().join(path.as_ref());
        assert!(is_ancestor_of(self.0.root.path(), &abs_path));

        let parent = abs_path.parent().unwrap();
        if parent != self.0.root.path() {
            create_dir(parent, true).unwrap();
        }

        let content = content.as_ref();
        std::fs::write(&abs_path, content).unwrap();
        self
    }

    /// Create a directory at the given path below the test root.
    ///
    /// # Panics
    /// Panics if the given path lexically escapes the temporary root.
    ///
    /// # Examples
    /// ```no_run
    /// # use typst_test_stdx::fs::TempEnv;
    /// TempEnv::run(
    ///     |test| test.setup_file_empty("foo/bar/empty.txt"),
    ///     // test ...
    ///     // assertion ...
    /// #    |root| {},
    /// #    |test| test,
    /// );
    /// ```
    pub fn setup_file_empty<P: AsRef<Path>>(&mut self, path: P) -> &mut Self {
        let abs_path = self.0.root.path().join(path.as_ref());
        assert!(is_ancestor_of(self.0.root.path(), &abs_path));

        let parent = abs_path.parent().unwrap();
        if parent != self.0.root.path() {
            create_dir(parent, true).unwrap();
        }

        std::fs::write(&abs_path, "").unwrap();
        self
    }
}

/// Specify what you expect to see after the test concluded. This is passed to
/// the third closure in [`TempEnv::run`].
pub struct Expect(TempEnv);

impl Expect {
    /// Assert the existence of a diretory and all its parent directories.
    ///
    /// # Panics
    /// Panics if the given path lexically escapes the temporary root.
    ///
    /// # Examples
    /// ```no_run
    /// # use typst_test_stdx::fs::TempEnv;
    /// TempEnv::run(
    /// #    |test| test,
    /// #    |root| {},
    ///     // setup ...
    ///     // test ...
    ///     |test| test.expect_dir("foo/bar"),
    /// );
    /// ```
    pub fn expect_dir<P: AsRef<Path>>(&mut self, path: P) -> &mut Self {
        self.0.add_expected(path.as_ref().to_path_buf(), None);
        self
    }

    /// Assert the existence of a file, its content and all its parent
    /// directories.
    ///
    /// # Panics
    /// Panics if the given path lexically escapes the temporary root.
    ///
    /// # Examples
    /// ```no_run
    /// # use typst_test_stdx::fs::TempEnv;
    /// TempEnv::run(
    /// #    |test| test,
    /// #    |root| {},
    ///     // setup ...
    ///     // test ...
    ///     |test| test.expect_file("foo/bar", b"Hello World\n"),
    /// );
    /// ```
    pub fn expect_file<P: AsRef<Path>>(&mut self, path: P, content: impl AsRef<[u8]>) -> &mut Self {
        let content = content.as_ref();
        self.0
            .add_expected(path.as_ref().to_path_buf(), Some(content.to_owned()));
        self
    }

    /// Assert the existence of a file, its content and all its parent
    /// directories.
    ///
    /// # Panics
    /// Panics if the given path lexically escapes the temporary root.
    ///
    /// # Examples
    /// ```no_run
    /// # use typst_test_stdx::fs::TempEnv;
    /// TempEnv::run(
    /// #    |test| test,
    /// #    |root| {},
    ///     // setup ...
    ///     // test ...
    ///     |test| test.expect_file_empty("foo/bar"),
    /// );
    /// ```
    pub fn expect_file_empty<P: AsRef<Path>>(&mut self, path: P) -> &mut Self {
        self.0.add_expected(path.as_ref().to_path_buf(), None);
        self
    }
}

impl TempEnv {
    /// Runs a test within a temporary directory. This receives three closures:
    /// 1. The preparation which creates the directory structure before running
    ///    the test.
    /// 1. The the actual test which receives the test root path and may return
    ///    a value.
    /// 1. The assertion which sets how the directory should look after the test
    ///    succesfully took place.
    ///
    /// Whether or not the test completes or panics, the temporary directory is
    /// cleaned up afterwards.
    ///
    /// # Panics
    /// Panics if the directory structure did not match the expected setup after
    /// the test completed successfully.
    ///
    /// # Examples
    /// ```no_run
    /// # use typst_test_stdx::fs::TempEnv;
    /// TempEnv::run(
    ///     |test| {
    ///         // prepare the test directory sructure
    ///         test.setup_file_empty("foo/bar/empty.txt")
    ///             .setup_file_empty("foo/baz/other.txt")
    ///     },
    ///     |root| {
    ///         // here we test our code
    ///         std::fs::remove_file(root.join("foo/bar/empty.txt")).unwrap();
    ///     },
    ///     |test| {
    ///         // assert the structure after the test
    ///         test.expect_dir("foo/bar/")
    ///             .expect_file_empty("foo/baz/other.txt")
    ///     },
    /// );
    /// ```
    pub fn run<R>(
        setup: impl FnOnce(&mut Setup) -> &mut Setup,
        test: impl FnOnce(&Path) -> R,
        expect: impl FnOnce(&mut Expect) -> &mut Expect,
    ) -> R {
        let dir = Self {
            root: TempDir::new("typst-test-stdx__fs").unwrap(),
            found: BTreeMap::new(),
            expected: BTreeMap::new(),
        };

        let mut s = Setup(dir);
        setup(&mut s);
        let Setup(dir) = s;

        let res = test(dir.root.path());

        let mut e = Expect(dir);
        expect(&mut e);
        let Expect(mut dir) = e;

        dir.collect();
        dir.assert();
        res
    }

    /// Runs a test within a temporary directory. This is a shorthand for
    /// [`TempEnv::run`] without running an assertion post check.
    ///
    /// Whether or not the test completes or panics, the temporary directory is
    /// cleaned up afterwards.
    ///
    /// # Examples
    /// ```no_run
    /// # use typst_test_stdx::fs::TempEnv;
    /// TempEnv::run_no_check(
    ///     |test| {
    ///         // prepare the test directory sructure
    ///         test.setup_file_empty("foo/bar/empty.txt")
    ///             .setup_file_empty("foo/baz/other.txt")
    ///     },
    ///     |root| {
    ///         // here we test our code
    ///         std::fs::remove_file(root.join("foo/bar/empty.txt")).unwrap();
    ///     },
    /// );
    /// ```
    pub fn run_no_check(setup: impl FnOnce(&mut Setup) -> &mut Setup, test: impl FnOnce(&Path)) {
        let dir = Self {
            root: TempDir::new("typst-test").unwrap(),
            found: BTreeMap::new(),
            expected: BTreeMap::new(),
        };

        let mut s = Setup(dir);
        setup(&mut s);
        let Setup(dir) = s;

        test(dir.root.path());
    }
}

impl TempEnv {
    fn add_expected(&mut self, expected: PathBuf, content: Option<Vec<u8>>) {
        for ancestor in expected.ancestors() {
            self.expected.insert(ancestor.to_path_buf(), None);
        }
        self.expected.insert(expected, content);
    }

    fn add_found(&mut self, found: PathBuf, content: Option<Vec<u8>>) {
        for ancestor in found.ancestors() {
            self.found.insert(ancestor.to_path_buf(), None);
        }
        self.found.insert(found, content);
    }

    fn read(&mut self, path: PathBuf) {
        let rel = path.strip_prefix(self.root.path()).unwrap().to_path_buf();
        if path.metadata().unwrap().is_file() {
            let content = std::fs::read(&path).unwrap();
            self.add_found(rel, Some(content));
        } else {
            let mut empty = true;
            for entry in path.read_dir().unwrap() {
                let entry = entry.unwrap();
                self.read(entry.path());
                empty = false;
            }

            if empty && self.root.path() != path {
                self.add_found(rel, None);
            }
        }
    }

    fn collect(&mut self) {
        self.read(self.root.path().to_path_buf())
    }

    fn assert(mut self) {
        let mut not_found = BTreeSet::new();
        let mut not_matched = BTreeMap::new();
        for (expected_path, expected_value) in self.expected {
            if let Some(found) = self.found.remove(&expected_path) {
                let expected = expected_value.unwrap_or_default();
                let found = found.unwrap_or_default();
                if expected != found {
                    not_matched.insert(expected_path, (found, expected));
                }
            } else {
                not_found.insert(expected_path);
            }
        }

        let not_expected: BTreeSet<_> = self.found.into_keys().collect();

        let mut mismatch = false;
        let mut msg = String::new();
        if !not_found.is_empty() {
            mismatch = true;
            writeln!(&mut msg, "\n=== Not found ===").unwrap();
            for not_found in not_found {
                writeln!(&mut msg, "/{}", not_found.display()).unwrap();
            }
        }

        if !not_expected.is_empty() {
            mismatch = true;
            writeln!(&mut msg, "\n=== Not expected ===").unwrap();
            for not_expected in not_expected {
                writeln!(&mut msg, "/{}", not_expected.display()).unwrap();
            }
        }

        if !not_matched.is_empty() {
            mismatch = true;
            writeln!(&mut msg, "\n=== Content matched ===").unwrap();
            for (path, (found, expected)) in not_matched {
                writeln!(&mut msg, "/{}", path.display()).unwrap();
                match (std::str::from_utf8(&found), std::str::from_utf8(&expected)) {
                    (Ok(found), Ok(expected)) => {
                        writeln!(&mut msg, "=== Expected ===\n>>>\n{}\n<<<\n", expected).unwrap();
                        writeln!(&mut msg, "=== Found ===\n>>>\n{}\n<<<\n", found).unwrap();
                    }
                    _ => {
                        writeln!(&mut msg, "Binary data differed").unwrap();
                    }
                }
            }
        }

        if mismatch {
            panic!("{msg}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_temp_env_run() {
        TempEnv::run(
            |test| {
                test.setup_file_empty("foo/bar/empty.txt")
                    .setup_file_empty("foo/baz/other.txt")
            },
            |root| {
                std::fs::remove_file(root.join("foo/bar/empty.txt")).unwrap();
            },
            |test| {
                test.expect_dir("foo/bar/")
                    .expect_file_empty("foo/baz/other.txt")
            },
        );
    }

    #[test]
    #[should_panic]
    fn test_temp_env_run_panic() {
        TempEnv::run(
            |test| {
                test.setup_file_empty("foo/bar/empty.txt")
                    .setup_file_empty("foo/baz/other.txt")
            },
            |root| {
                std::fs::remove_file(root.join("foo/bar/empty.txt")).unwrap();
            },
            |test| test.expect_dir("foo/bar/"),
        );
    }

    #[test]
    fn test_temp_env_run_no_check() {
        TempEnv::run_no_check(
            |test| {
                test.setup_file_empty("foo/bar/empty.txt")
                    .setup_file_empty("foo/baz/other.txt")
            },
            |root| {
                std::fs::remove_file(root.join("foo/bar/empty.txt")).unwrap();
            },
        );
    }
}
