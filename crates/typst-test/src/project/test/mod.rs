use std::ffi::OsString;
use std::fmt::Display;
use std::path::PathBuf;
use std::process::Output;

use oxipng::PngError;

use crate::{util, Project};

pub mod context;

const REF_DIR: &str = "ref";
const OUT_DIR: &str = "out";
const DIFF_DIR: &str = "diff";

#[derive(Debug, Clone)]
pub enum Filter {
    Contains(String),
    Exact(String),
}

impl Filter {
    pub fn new(filter: String, exact: bool) -> Filter {
        if exact {
            Self::Exact(filter)
        } else {
            Self::Contains(filter)
        }
    }

    pub fn value(&self) -> &str {
        match self {
            Filter::Contains(f) => f,
            Filter::Exact(f) => f,
        }
    }

    #[allow(dead_code)]
    pub fn matches(&self, test: &Test) -> bool {
        match self {
            Filter::Contains(s) => test.name().contains(s),
            Filter::Exact(s) => test.name() == s,
        }
    }
}

#[derive(Debug)]
pub struct Test {
    name: String,
    // TODO: comparison
    // TODO: actions done before/after compiling/comparing
}

impl Test {
    pub fn new(name: String) -> Self {
        Self { name }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn test_dir(&self, project: &Project) -> PathBuf {
        util::fs::path_in_root(project.tests_root_dir(), [self.name()])
    }

    pub fn ref_dir(&self, project: &Project) -> PathBuf {
        util::fs::path_in_root(project.tests_root_dir(), [self.name(), REF_DIR])
    }

    pub fn out_dir(&self, project: &Project) -> PathBuf {
        util::fs::path_in_root(project.tests_root_dir(), [self.name(), OUT_DIR])
    }

    pub fn diff_dir(&self, project: &Project) -> PathBuf {
        util::fs::path_in_root(project.tests_root_dir(), [self.name(), DIFF_DIR])
    }

    pub fn test_file(&self, project: &Project) -> PathBuf {
        util::fs::path_in_root(project.tests_root_dir(), [self.name(), "test"])
            .with_extension("typ")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Stage {
    Preparation,
    Compilation,
    Comparison,
    #[allow(dead_code)]
    Update,
    #[allow(dead_code)]
    Cleanup,
}

impl Display for Stage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Stage::Preparation => "preparation",
                Stage::Compilation => "compilation",
                Stage::Comparison => "comparison",
                Stage::Update => "update",
                Stage::Cleanup => "cleanup",
            }
        )
    }
}

pub type TestResult<E = TestFailure> = Result<(), E>;

#[derive(Debug, Clone)]
pub enum TestFailure {
    Preparation(PrepareFailure),
    Compilation(CompileFailure),
    Comparison(CompareFailure),
    Update(UpdateFailure),
    Cleanup(CleanupFailure),
}

impl Display for TestFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "test failed")
    }
}

impl From<PrepareFailure> for TestFailure {
    fn from(value: PrepareFailure) -> Self {
        Self::Preparation(value)
    }
}

impl From<CompileFailure> for TestFailure {
    fn from(value: CompileFailure) -> Self {
        Self::Compilation(value)
    }
}

impl From<CompareFailure> for TestFailure {
    fn from(value: CompareFailure) -> Self {
        Self::Comparison(value)
    }
}

impl From<UpdateFailure> for TestFailure {
    fn from(value: UpdateFailure) -> Self {
        Self::Update(value)
    }
}

impl From<CleanupFailure> for TestFailure {
    fn from(value: CleanupFailure) -> Self {
        Self::Cleanup(value)
    }
}

impl std::error::Error for TestFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            TestFailure::Preparation(e) => e,
            TestFailure::Compilation(e) => e,
            TestFailure::Comparison(e) => e,
            TestFailure::Update(e) => e,
            TestFailure::Cleanup(e) => e,
        })
    }
}

#[derive(Debug, Clone)]
pub enum PrepareFailure {}

impl Display for PrepareFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "preparation failed")
    }
}

impl std::error::Error for PrepareFailure {}

#[derive(Debug, Clone)]
pub struct CleanupFailure {}

impl Display for CleanupFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cleanup failed")
    }
}

impl std::error::Error for CleanupFailure {}

#[derive(Debug, Clone)]
pub struct CompileFailure {
    pub args: Vec<OsString>,
    pub output: Output,
}

impl Display for CompileFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "compilation failed")
    }
}

impl std::error::Error for CompileFailure {}

#[derive(Debug, Clone)]
pub enum CompareFailure {
    PageCount {
        output: usize,
        reference: usize,
    },
    Page {
        pages: Vec<(usize, ComparePageFailure)>,
        diff_dir: Option<PathBuf>,
    },
    MissingOutput,
    MissingReferences,
}

impl Display for CompareFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompareFailure::PageCount { output, reference } => write!(
                f,
                "page count differed: out ({}) != ref ({})",
                output, reference
            ),
            CompareFailure::Page { pages, diff_dir: _ } => write!(
                f,
                "{} page{} differed {:?}",
                pages.len(),
                if pages.len() == 1 { "" } else { "s" },
                pages.iter().map(|(n, _)| n).collect::<Vec<_>>()
            ),
            CompareFailure::MissingOutput => write!(f, "missing output"),
            CompareFailure::MissingReferences => write!(f, "missing references"),
        }
    }
}

impl std::error::Error for CompareFailure {}

#[derive(Debug, Clone)]
pub enum ComparePageFailure {
    Dimensions {
        output: (u32, u32),
        reference: (u32, u32),
    },
    Content,
}

impl Display for ComparePageFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComparePageFailure::Dimensions { output, reference } => {
                write!(
                    f,
                    "dimensions differed: out {:?} !=  ref {:?}",
                    output, reference
                )
            }
            ComparePageFailure::Content => write!(f, "content differed"),
        }
    }
}

impl std::error::Error for ComparePageFailure {}

#[derive(Debug, Clone)]
pub enum UpdateFailure {
    Optimize { error: PngError },
}

impl Display for UpdateFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "update failed")
    }
}

impl std::error::Error for UpdateFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            UpdateFailure::Optimize { error } => Some(error),
        }
    }
}
