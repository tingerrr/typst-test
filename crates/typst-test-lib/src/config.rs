use std::path::{Path, PathBuf};

/// The default tests root path relative to the _project root_.
pub const DEFAULT_TESTS_ROOT: &str = "tests";

/// The default template path relative to the _tests root_.
pub const DEFAULT_TEMPLATE: &str = "tests/template.typ";

/// The default [`Config::tests_root`].
pub fn default_tests_root() -> PathBuf {
    DEFAULT_TESTS_ROOT.into()
}

/// The default [`Config::template`].
pub fn default_template() -> PathBuf {
    DEFAULT_TEMPLATE.into()
}

/// The config which can be read from the `tool.typst-test` section of a
/// `typst.toml` manifest. The default values for [`Self::test_root`] and
/// [`Self::template`] are given by [`DEFAULT_TESTS_ROOT`] and
/// [`DEFAULT_TEMPLATE`] respectively, note that.
///
/// All paths are relative to the _project root_.
///
/// Prepare and clean up hooks should be run using the [`hook`][crate::hook]
/// API.
///
/// This struct deliberately only supports deserialization and will be phased
/// out in favor of a toml-edit solution.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    /// The path pointing to the root directory of all tests.
    ///
    /// Defaults to [`DEFAULT_TEST_ROOT`].
    #[serde(rename = "tests", default = "default_tests_root")]
    pub tests_root: PathBuf,

    /// The path pointing to the template test script.
    ///
    /// Defaults to [`DEFAULT_TEMPLATE`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<PathBuf>,

    /// The path to a prepare hook.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prepare: Option<PathBuf>,

    /// The path to a prepare hook.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prepare_each: Option<PathBuf>,

    /// The path to the clean up hook.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cleanup: Option<PathBuf>,

    /// The path to a clean up hook.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cleanup_each: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        let tests_root = Path::new(DEFAULT_TESTS_ROOT).to_path_buf();
        let template = tests_root.join("template.typ");

        Self {
            tests_root,
            template: Some(template),
            prepare: None,
            prepare_each: None,
            cleanup: None,
            cleanup_each: None,
        }
    }
}
