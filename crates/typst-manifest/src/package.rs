//! Typst package metadata.

use std::path::PathBuf;

use semver::Version;
use serde::{Deserialize, Serialize};

/// The `package` key in the manifest, storing a package's metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Package {
    /// The name of the package.
    pub name: String,

    /// The current verison of the packge.
    pub version: Version,

    /// The primary module of the package.
    pub entrypoint: PathBuf,

    /// The authors of the package.
    pub authors: Vec<String>,

    /// The license expression for the package.
    pub license: String,

    /// The description of the package.
    pub description: String,

    /// The homepage URL of the package.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,

    /// The repository URL of the package.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,

    /// The keywords for the package.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,

    /// The excluded paths of this package. This paths are ignored by the
    /// package manager's bundler.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub exclude: Vec<String>,

    /// The minimum compiler version for the package.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compiler: Option<Version>,
}
