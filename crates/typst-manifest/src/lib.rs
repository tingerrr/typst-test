use std::fmt::Debug;
use std::path::Path;
use std::{fs, io};

use serde::{Deserialize, Serialize};
use toml::Table;

use self::error::{DeserializeError, Error};
use self::package::Package;
use self::tool::Tool;

pub mod error;
pub mod package;
pub mod tool;

// re-export as those are part of the public API
pub use {serde, toml};

/// The name of the typst manifest file.
pub const MANIFEST_NAME: &str = "typst.toml";

/// A typst.toml manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// The `package` key, storing a package's metadata.
    pub package: Package,

    /// The `tool` key, storing 3rd-party configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<Tool>,
}

impl Manifest {
    /// Deserializes a manifest from a [`Value`][toml::Value].
    ///
    /// Returns a error if deserialization fails.
    ///
    /// # Examples
    /// ```
    /// use typst_manifest::Manifest;
    /// use toml::{toml, Value};
    ///
    /// let toml = toml! {
    ///     [package]
    ///     name = "Foo"
    ///     version = "0.1.0"
    ///     entrypoint = "/src/lib.typ"
    ///     authors = ["tingerrr <me@tinger.dev>"]
    ///     license = "MIT"
    ///     description = "Bar"
    /// };
    ///
    /// let manifest = Manifest::from_value(toml)?;
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn from_value(toml: Table) -> Result<Self, DeserializeError> {
        Self::deserialize(toml)
    }

    /// Deserializes a manifest from the contents of a manifest file.
    ///
    /// Returns a error if deserialization fails.
    ///
    /// # Examples
    /// ```
    /// use typst_manifest::Manifest;
    ///
    /// let toml = r#"
    ///     [package]
    ///     name = "Foo"
    ///     version = "0.1.0"
    ///     entrypoint = "src/lib.typ"
    ///     authors = ["John Doe <john@doe.com>"]
    ///     license = "MIT"
    ///     description = "Bar"
    /// "#;
    ///
    /// let manifest = Manifest::from_str(toml)?;
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn from_str(toml: &str) -> Result<Self, DeserializeError> {
        toml::from_str(toml)
    }
}

/// Checks if a directory is a package root directory, i.e. if it contains a
/// package manifest.
///
/// Returns an error if [read_dir][fs::read_dir] fails.
///
/// # Examples
/// ```no_run
/// use typst_manifest::is_package_root;
/// use std::env::current_dir;
///
/// let pwd = current_dir()?;
/// if is_package_root(pwd)? {
///     println!("PWD is package root");
/// } else {
///     println!("PWD is not package root");
/// }
/// # Ok::<_, Box<dyn std::error::Error>>(())
/// ```
pub fn is_package_root<P: AsRef<Path>>(path: P) -> io::Result<bool> {
    fn inner(path: &Path) -> io::Result<bool> {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let typ = entry.file_type()?;
            let name = entry.file_name();

            if typ.is_file() && name == MANIFEST_NAME {
                return Ok(true);
            }
        }

        Ok(false)
    }

    inner(path.as_ref())
}

/// Recursively looks up the ancestors of `path` until it finds a package root
/// directory. If `path` is relative, then it may not discover the package root,
/// if it lies above the relative root. See [is_package_root] for more info on
/// when a directory is a packge root.
///
/// Returns `None` if no root can be found, returns an error if
/// [is_package_root] fails.
///
/// # Examples
/// ```no_run
/// use typst_manifest::try_find_package_root;
/// use std::env::current_dir;
///
/// let pwd = current_dir()?;
/// match try_find_package_root(&pwd)? {
///     Some(root) => println!("Found package root: {root:?}"),
///     None => println!("No package root found"),
/// }
/// # Ok::<_, Box<dyn std::error::Error>>(())
/// ```
pub fn try_find_package_root(path: &Path) -> io::Result<Option<&Path>> {
    for ancestor in path.ancestors() {
        if is_package_root(ancestor)? {
            return Ok(Some(ancestor));
        }
    }

    Ok(None)
}

/// Tries to find the manifest for the project containing `path`.If `path` is
/// relative, then it may not discover the package root, if it lies above the
/// relative root. see [try_find_package_root] for mor info on how the manifest
/// is discovered.
///
/// Returns `None` if no manifest could be found, returns an error if
/// [try_find_package_root] fails, or if a manifest was foud but could not be
/// parsed.
///
/// # Examples
/// ```no_run
/// use typst_manifest::try_find_manifest;
/// use std::env::current_dir;
///
/// let pwd = current_dir()?;
/// match try_find_manifest(pwd)? {
///     Some(manifest) => println!("Manifest found: {manifest:#?}"),
///     None => println!("No manifest found"),
/// }
/// # Ok::<_, Box<dyn std::error::Error>>(())
/// ```
pub fn try_find_manifest<P: AsRef<Path>>(path: P) -> Result<Option<Manifest>, Error> {
    fn inner(path: &Path) -> Result<Option<Manifest>, Error> {
        let Some(root) = try_find_package_root(path)? else {
            return Ok(None);
        };

        let content = fs::read_to_string(root.join(MANIFEST_NAME))?;
        let manifest = Manifest::from_str(&content)?;

        Ok(Some(manifest))
    }

    inner(path.as_ref())
}
