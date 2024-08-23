//! Reading and correctly interpreting user configuration.

use std::path::PathBuf;

use thiserror::Error;
use toml_edit::{Decor, DocumentMut, RawString};

/// All valid keys for this config.
pub static KEYS: &[&str] = &[
    "tests",
    "vcs",
    "template",
    "prepare",
    "prepare-each",
    "cleanup",
    "cleanup-each",
];

/// The key used to configure typst-test in the manifest tool config.
pub const MANIFEST_TOOL_KEY: &str = "typst-test";

/// The default tests root path relative to the _project root_.
pub const DEFAULT_TESTS_ROOT: &str = "tests";

/// The default template path relative to the _project root_.
pub const DEFAULT_TEMPLATE: &str = "tests/template.typ";

/// The default vcs used within the project.
pub const DEFAULT_VCS: &str = "git";

/// The default [`Config::tests_root`], see also [`DEFAULT_TESTS_ROOT`].
pub fn default_tests_root() -> PathBuf {
    DEFAULT_TESTS_ROOT.into()
}

/// The default [`Config::template`], see also [`DEFAULT_TEMPLATE`].
pub fn default_template() -> PathBuf {
    DEFAULT_TEMPLATE.into()
}

/// The default [`Config::vcs`], see also [`DEFAULT_VCS`].
pub fn default_vcs() -> String {
    DEFAULT_VCS.into()
}

/// The config which can be read from the `tool.typst-test` section of a
/// `typst.toml` manifest. The default values for [`Self::tests_root`] and
/// [`Self::template`] are given by [`DEFAULT_TESTS_ROOT`] and
/// [`DEFAULT_TEMPLATE`] respectively.
///
/// All paths are relative to the _project root_.
///
/// Prepare and clean up hooks should be run using the [`hook`][crate::hook]
/// API.
///
/// This struct deliberately only supports deserialization and will be phased
/// out in favor of a toml-edit solution.
#[derive(Debug, Default, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    /// The path pointing to the root directory of all tests.
    ///
    /// Defaults to [`DEFAULT_TESTS_ROOT`].
    #[serde(rename = "tests")]
    pub tests_root: Option<String>,

    /// The path pointing to the template test script.
    ///
    /// Defaults to [`DEFAULT_TEMPLATE`].
    pub template: Option<String>,

    /// The vcs to use, supports `git` or `none`.
    ///
    /// Defaults to [`DEFAULT_VCS`].
    pub vcs: Option<String>,

    /// The path to a prepare hook.
    pub prepare: Option<String>,

    /// The path to a prepare hook.
    pub prepare_each: Option<String>,

    /// The path to the clean up hook.
    pub cleanup: Option<String>,

    /// The path to a clean up hook.
    pub cleanup_each: Option<String>,
}

// ensure we don't delete any decor by setting tables to be implcit
fn is_significant_decor(decor: &Decor) -> bool {
    decor
        .prefix()
        .and_then(RawString::as_str)
        .is_some_and(|d| !d.trim().is_empty())
        || decor
            .prefix()
            .and_then(RawString::as_str)
            .is_some_and(|d| !d.trim().is_empty())
}

#[derive(Debug, Error)]
pub enum ConfigError {
    /// The given key is not valid or the config.
    #[error("unknown key {key:?}")]
    UnknownKey { key: String },

    /// The given section was not of the expected type.
    #[error("`{section}` wasn't a {typ}")]
    IncorrectType {
        /// The section for which the type was incorrect.
        section: String,

        /// The expected type.
        typ: &'static str,
    },
}

impl Config {
    /// Sets the fallbacks of `tests_root`, `template` and `vcs`, if they are
    /// `None`.
    pub fn set_fallbacks(&mut self) {
        self.tests_root.get_or_insert(DEFAULT_TESTS_ROOT.to_owned());
        self.template.get_or_insert(DEFAULT_TEMPLATE.to_owned());
        self.vcs.get_or_insert(DEFAULT_VCS.to_owned());
    }

    /// Returns the test root, or the default fallback value.
    pub fn tests_root_fallback(&self) -> &str {
        self.tests_root.as_deref().unwrap_or(DEFAULT_TESTS_ROOT)
    }

    /// Returns the template path, or the default fallback value.
    pub fn template_fallback(&self) -> &str {
        self.template.as_deref().unwrap_or(DEFAULT_TEMPLATE)
    }

    /// Returns the vcs, or the default fallback value.
    pub fn vcs_fallback(&self) -> &str {
        self.vcs.as_deref().unwrap_or(DEFAULT_VCS)
    }

    /// Returns a reference to the value with the given key.
    ///
    /// # Errors
    /// Returns an error if this key doesn't exist.
    pub fn get(&self, key: &str) -> Result<&Option<String>, ConfigError> {
        Ok(match key {
            "tests" => &self.tests_root,
            "vcs" => &self.vcs,
            "template" => &self.template,
            "prepare" => &self.prepare,
            "prepare-each" => &self.prepare_each,
            "cleanup" => &self.cleanup,
            "cleanup-each" => &self.cleanup_each,
            _ => {
                return Err(ConfigError::UnknownKey {
                    key: key.to_owned(),
                })
            }
        })
    }

    /// Returns a mutable reference to the value with the given key.
    ///
    /// # Errors
    /// Returns an error if this key doesn't exist.
    pub fn get_mut(&mut self, key: &str) -> Result<&mut Option<String>, ConfigError> {
        Ok(match key {
            "tests" => &mut self.tests_root,
            "vcs" => &mut self.vcs,
            "template" => &mut self.template,
            "prepare" => &mut self.prepare,
            "prepare-each" => &mut self.prepare_each,
            "cleanup" => &mut self.cleanup,
            "cleanup-each" => &mut self.cleanup_each,
            _ => {
                return Err(ConfigError::UnknownKey {
                    key: key.to_owned(),
                })
            }
        })
    }

    /// Writes the current config into the given manifest document overriding
    /// any previously set values, but keeping comments and non config values
    /// intact.
    ///
    /// This returns an error if the `tool` or `tool.typst-test` sections are
    /// given and not tables.
    pub fn write_into(&self, doc: &mut DocumentMut) -> Result<(), ConfigError> {
        match doc.get_key_value_mut("tool") {
            Some((k, v)) => {
                if !v.is_table_like() {
                    return Err(ConfigError::IncorrectType {
                        section: "tool".into(),
                        typ: "table",
                    });
                }

                if let Some(tool) = v.as_table_mut() {
                    if !is_significant_decor(k.leaf_decor())
                        && !is_significant_decor(k.dotted_decor())
                        && !is_significant_decor(tool.decor())
                    {
                        tool.set_implicit(true);
                    }
                };
            }
            None => {
                let mut tool = toml_edit::Table::new();
                tool.set_implicit(true);
                doc["tool"] = toml_edit::Item::Table(tool);
            }
        }

        match doc["tool"]
            .as_table_like_mut()
            .unwrap()
            .get_key_value_mut(MANIFEST_TOOL_KEY)
        {
            Some((k, v)) => {
                if !v.is_table_like() {
                    return Err(ConfigError::IncorrectType {
                        section: format!("tool.{}", MANIFEST_TOOL_KEY),
                        typ: "table",
                    });
                }

                if let Some(tt) = v.as_table_mut() {
                    if !is_significant_decor(k.leaf_decor())
                        && !is_significant_decor(k.dotted_decor())
                        && !is_significant_decor(tt.decor())
                    {
                        tt.set_implicit(true);
                    }
                };
            }
            None => {
                let mut tool = toml_edit::Table::new();
                tool.set_implicit(true);
                doc["tool"][MANIFEST_TOOL_KEY] = toml_edit::Item::Table(tool);
            }
        }

        let tt = &mut doc["tool"][MANIFEST_TOOL_KEY];

        for key in KEYS {
            if let Some(val) = self.get(key).unwrap() {
                tt[key] = toml_edit::value(val);
            } else {
                tt[key] = toml_edit::Item::None;
            }
        }

        Ok(())
    }

    /// Returns an iterator over key value pairs of this config.
    pub fn pairs(&self) -> impl Iterator<Item = (&'static str, &'_ Option<String>)> + '_ {
        KEYS.iter().map(|&k| (k, self.get(k).unwrap()))
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use indoc::{formatdoc, indoc};

    use super::*;

    #[test]
    fn test_write_into_empty() {
        let mut doc = DocumentMut::new();
        Config::default().write_into(&mut doc).unwrap();
        assert_eq!(doc.to_string(), "");
    }

    #[test]
    fn test_write_into_implicit() {
        let mut doc = DocumentMut::from_str(indoc! {r#"
            [tool.foo]
            foo = 'var'
        "#})
        .unwrap();

        Config::default().write_into(&mut doc).unwrap();
        assert_eq!(
            doc.to_string(),
            indoc! {r#"
                [tool.foo]
                foo = 'var'
            "#}
        );
    }

    #[test]
    fn test_write_into_implicit_comment() {
        let mut doc = DocumentMut::from_str(&formatdoc! {r#"
            [tool.foo]
            foo = 'var'

            # comment
            [tool.{}]
        "#, MANIFEST_TOOL_KEY})
        .unwrap();

        Config::default().write_into(&mut doc).unwrap();
        assert_eq!(
            doc.to_string(),
            formatdoc! {r#"
                [tool.foo]
                foo = 'var'

                # comment
                [tool.{}]
            "#, MANIFEST_TOOL_KEY},
        );
    }

    #[test]
    fn test_write_into_new() {
        let mut doc = DocumentMut::from_str(&formatdoc! {r#"
            [tool.foo]
            foo = 'var'
        "#})
        .unwrap();

        let config = Config {
            tests_root: Some("tests".into()),
            ..Config::default()
        };
        config.write_into(&mut doc).unwrap();
        assert_eq!(
            doc.to_string(),
            formatdoc! {r#"
                [tool.foo]
                foo = 'var'

                [tool.{}]
                tests = "tests"
            "#, MANIFEST_TOOL_KEY},
        );
    }

    #[test]
    fn test_write_into_unset() {
        let mut doc = DocumentMut::from_str(&formatdoc! {r#"
            [tool.foo]
            foo = 'var'

            [tool.{}]
            tests = "tests"
        "#, MANIFEST_TOOL_KEY})
        .unwrap();

        Config::default().write_into(&mut doc).unwrap();
        assert_eq!(
            doc.to_string(),
            indoc! {r#"
                [tool.foo]
                foo = 'var'
            "#},
        );
    }
}
