//! 3rd-party tooling configuration.

use std::collections::BTreeMap;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use toml::Table;

use crate::error::DeserializeError;

/// The `tool` key in the manifest, this key may contain any configuration
/// given by 3rd-party tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Tool {
    /// The individual tool keys, these are commonly given in the following
    /// form:
    /// ```toml
    /// [tool.typst-test]
    /// # ...
    ///
    /// [tool.packager]
    /// # ...
    /// ```
    pub keys: BTreeMap<String, Table>,
}

impl Tool {
    /// Get a tool section with the given key.
    ///
    /// Returns `None` if the key doesn't exist, returns an error if the key
    /// exists but cannot be parsed into `T`.
    pub fn get_section<T: DeserializeOwned>(
        &self,
        tool: &str,
    ) -> Result<Option<T>, DeserializeError> {
        self.keys.get(tool).cloned().map(T::deserialize).transpose()
    }
}
