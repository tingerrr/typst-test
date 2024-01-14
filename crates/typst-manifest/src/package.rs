use semver::Version;
use serde::{Deserialize, Serialize};

/// The `package` key in the manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Package {
    pub name: String,
    pub version: Version,
    pub entrypoint: String,
    pub authors: Vec<String>,
    pub license: String,
    pub description: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,

    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,

    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub exclude: Vec<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub compiler: Option<Version>,
}
