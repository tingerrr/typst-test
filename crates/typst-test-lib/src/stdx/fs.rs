//! Helper functions and types for managing and manipulating the filesystem.

use std::io::ErrorKind;
use std::path::Path;
use std::{fs, io};

use crate::stdx::result::ResultEx;

/// Creates a new directory and its parent directories if `all` is specified,
/// but doesn't fail if it already exists.
///
/// # Example
/// ```no_run
/// # use typst_test_lib::stdx::fs::create_dir;
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

/// Removes a file, but doesn't fail if it doesn't exist.
///
/// # Example
/// ```no_run
/// # use typst_test_lib::stdx::fs::remove_file;
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

/// Removes a directory, but doesn't fail if it doesn't exist.
///
/// # Example
/// ```no_run
/// # use typst_test_lib::stdx::fs::remove_dir;
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
/// # use typst_test_lib::stdx::fs::create_empty_dir;
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
/// # use typst_test_lib::stdx::fs::common_ancestor;
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
/// # use typst_test_lib::stdx::fs::is_ancestor_of;
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
