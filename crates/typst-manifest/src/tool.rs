use std::collections::BTreeMap;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};

/// The `tool` key in the manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Tool {
    pub keys: BTreeMap<String, toml::Table>,
}

impl Tool {
    /// Get a tool section with the given key.
    ///
    /// Returns an `None` if the key doesn't exist, returns an error if the key
    /// exists but cannot be parsed into `T`.
    pub fn get_section<T: DeserializeOwned>(
        &self,
        tool: &str,
    ) -> Result<Option<T>, <toml::Value as Deserializer>::Error> {
        self.keys.get(tool).cloned().map(T::deserialize).transpose()
    }
}
