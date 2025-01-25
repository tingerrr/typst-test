//! Common report PODs for stable JSON representation of internal entities.

use lib::project::Project;
use lib::test::{Suite, Test};
use serde::Serialize;
use typst_syntax::package::PackageVersion;

#[derive(Debug, Serialize)]
pub struct ProjectJson<'p, 's> {
    pub package: Option<PackageJson<'p>>,
    pub vcs: Option<String>,
    pub tests: Vec<TestJson<'s>>,
    pub is_template: bool,
}

impl<'p, 's> ProjectJson<'p, 's> {
    pub fn new(project: &'p Project, suite: &'s Suite) -> Self {
        Self {
            package: project.manifest().map(|m| PackageJson {
                name: &m.package.name,
                version: &m.package.version,
            }),
            vcs: project.vcs().map(|vcs| vcs.to_string()),
            tests: suite.matched().values().map(TestJson::new).collect(),
            is_template: project.manifest_template_info().is_some(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PackageJson<'p> {
    pub name: &'p str,
    pub version: &'p PackageVersion,
}

#[derive(Debug, Serialize)]
pub struct TestJson<'t> {
    pub id: &'t str,
    pub kind: &'static str,
}

impl<'t> TestJson<'t> {
    pub fn new(test: &'t Test) -> Self {
        Self {
            id: test.id().as_str(),
            kind: test.kind().as_str(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct FontVariantJson {
    pub style: &'static str,
    pub weight: u16,
    pub stretch: f64,
}

#[derive(Debug, Serialize)]
pub struct FontJson<'f> {
    pub name: &'f str,
    pub variants: Vec<FontVariantJson>,
}

#[derive(Serialize)]
pub struct FailedJson {
    pub compilation: usize,
    pub comparison: usize,
    pub otherwise: usize,
}

#[derive(Serialize)]
pub struct DurationJson {
    pub seconds: u64,
    pub nanoseconds: u32,
}
