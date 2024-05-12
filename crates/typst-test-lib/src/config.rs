use std::path::PathBuf;

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub tests: PathBuf,
    pub template: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        let tests: PathBuf = "tests".into();
        let template = Some(tests.join("template.typ"));

        Self { tests, template }
    }
}
