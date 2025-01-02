//! Reading and writing configuration from TOML files.

use std::collections::BTreeMap;
use std::{fs, io};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use typst::syntax::package::PackageManifest;

use crate::stdx::result::ResultEx;

// TODO: add proper test set collecting and parsing, test sets should be
// overridable in local configs but still fail on duplicate definitions.

/// All valid keys for this config.
pub static KEYS: &[&str] = &["test-set"];

/// The key used to configure typst-test in the manifest tool config.
pub const MANIFEST_TOOL_KEY: &str = crate::TOOL_NAME;

/// The directory name for in which the user config can be found.
pub const CONFIG_SUB_DIRECTORY: &str = crate::TOOL_NAME;

/// A set of config layers used to retrieve options, configs are looked up in
/// the following order:
/// - `override`: supplied at runtime from the command line, envars or other
///   means
/// - `project`: found in the typst.toml manifest
/// - `user`: found in a user config directory
///
/// If none of these configs contain a setting the default is used.
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash)]
pub struct Config {
    /// The override config.
    pub override_: Option<ConfigLayer>,

    /// The project config.
    pub project: Option<ConfigLayer>,

    /// The user config.
    pub user: Option<ConfigLayer>,
}

impl Config {
    /// Create a new config with the given overrides.
    pub fn new(override_: Option<ConfigLayer>) -> Self {
        Self {
            override_,
            project: None,
            user: None,
        }
    }
}

/// A single layer within all configs, a set of values which can be
/// overridden by more granular configs.
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct ConfigLayer {
    /// Custom test set definitions.
    pub test_sets: Option<BTreeMap<String, String>>,
}

impl ConfigLayer {
    /// Reads the user config at its predefined location.
    ///
    /// The location used is [`dirs::config_dir()`].
    pub fn collect_user() -> Result<Option<Self>, ReadError> {
        let Some(config_dir) = dirs::config_dir() else {
            tracing::warn!("couldn't retrieve user config home");
            return Ok(None);
        };

        let config = config_dir.join(CONFIG_SUB_DIRECTORY).join("config.toml");
        let Some(content) =
            fs::read_to_string(config).ignore(|err| err.kind() == io::ErrorKind::NotFound)?
        else {
            return Ok(None);
        };

        Ok(toml::from_str(&content)?)
    }

    /// Parses a config from the tool section of a manifest.
    pub fn from_manifest(manifest: &PackageManifest) -> Result<Option<Self>, ReadError> {
        let Some(section) = manifest.tool.sections.get(MANIFEST_TOOL_KEY) else {
            return Ok(None);
        };

        Self::deserialize(section.clone())
            .map(Some)
            .map_err(ReadError::Toml)
    }
}

/// Returned by [`ConfigLayer::collect_user`] and
/// [`ConfigLayer::from_manifest`].
#[derive(Debug, Error)]
pub enum ReadError {
    /// The given key is not valid or the config.
    #[error("a toml parsing error occurred")]
    Toml(#[from] toml::de::Error),

    /// An io error occurred.
    #[error("an io error occurred")]
    Io(#[from] io::Error),
}
