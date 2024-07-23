//! Reading and correctly interpreting user configuration.

use std::path::{Path, PathBuf};

/// The default tests root path relative to the _project root_.
pub const DEFAULT_TESTS_ROOT: &str = "tests";

/// The default template path relative to the _project root_.
pub const DEFAULT_TEMPLATE: &str = "tests/template.typ";

/// The default [`Config::tests_root`], see also [`DEFAULT_TESTS_ROOT`].
pub fn default_tests_root() -> PathBuf {
    DEFAULT_TESTS_ROOT.into()
}

/// The default [`Config::template`], see also [`DEFAULT_TEMPLATE`].
pub fn default_template() -> PathBuf {
    DEFAULT_TEMPLATE.into()
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
#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    /// The path pointing to the root directory of all tests.
    ///
    /// Defaults to [`DEFAULT_TESTS_ROOT`].
    #[serde(rename = "tests")]
    pub tests_root: Option<PathBuf>,

    /// The path pointing to the template test script.
    ///
    /// Defaults to [`DEFAULT_TEMPLATE`].
    pub template: Option<PathBuf>,

    /// The path to a prepare hook.
    pub prepare: Option<PathBuf>,

    /// The path to a prepare hook.
    pub prepare_each: Option<PathBuf>,

    /// The path to the clean up hook.
    pub cleanup: Option<PathBuf>,

    /// The path to a clean up hook.
    pub cleanup_each: Option<PathBuf>,
}

impl Config {
    /// Returns the test root, or the default fallback value.
    pub fn tests_root_fallback(&self) -> &Path {
        self.tests_root
            .as_deref()
            .unwrap_or(Path::new(DEFAULT_TESTS_ROOT))
    }

    /// Returns the template path, or the default fallback value.
    pub fn template_fallback(&self) -> &Path {
        self.template
            .as_deref()
            .unwrap_or(Path::new(DEFAULT_TEMPLATE))
    }
}
