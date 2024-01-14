use std::path::Path;
use std::{fs, io};

use serde::{Deserialize, Serialize};

use self::package::Package;
use self::tool::Tool;

mod package;
mod tool;

/// The name of the typst manifest file.
pub const MANIFEST_NAME: &str = "typst.toml";

/// A typst.toml manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// The `package` key.
    pub package: Package,

    /// The `tool` key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<Tool>,
}

impl Manifest {
    /// Deserializes a manifest from a [`Value`][toml::Value].
    ///
    /// Returns a error if deserialization fails.
    pub fn from_value(value: toml::Value) -> Result<Self, toml::de::Error> {
        Self::deserialize(value)
    }

    /// Deserializes a manifest from the contents of a manifest file.
    ///
    /// Returns a error if deserialization fails.
    pub fn from_str(toml: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(toml)
    }
}

/// Checks if a directory is a package root directory, i.e. if it contains a
/// package manifest.
///
/// Returns an error if [read_dir][fs::read_dir] fails.
pub fn is_package_root(dir: &Path) -> io::Result<bool> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let typ = entry.file_type()?;
        let name = entry.file_name();

        if typ.is_file() && name == MANIFEST_NAME {
            return Ok(true);
        }
    }

    Ok(false)
}

/// Recursively looks up the ancestors of `path` until it finds a package root
/// directory. If `path` is relative, then it may not discover the package root,
/// if it lies above the relative root. See [is_package_root] for more info on
/// when a directory is a packge root.
///
/// Returns `None` if no root can be found, returns an error if
/// [is_package_root] fails.
pub fn try_find_package_root(path: &Path) -> io::Result<Option<&Path>> {
    for ancestor in path.ancestors() {
        if is_package_root(ancestor)? {
            return Ok(Some(ancestor));
        }
    }

    Ok(None)
}
