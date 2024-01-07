use std::ffi::OsString;
use std::fmt::Display;
use std::process::Output;

use context::Context;

use self::context::ContextResult;

pub mod context;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Test {
    name: String,
    folder: bool,
    // TODO: comparison
    // TODO: actions done before/after compiling/comparing
}

impl Test {
    pub fn new(name: String, folder: bool) -> Self {
        Self { name, folder }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn folder(&self) -> bool {
        self.folder
    }

    #[tracing::instrument(skip_all, fields(test = ?self.name))]
    pub fn run(&self, context: &Context, compare: bool) -> ContextResult {
        let context = context.test(self);
        context.run(compare)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Stage {
    Preparation,
    Compilation,
    Comparison,
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
            CompareFailure::Page { pages } => write!(
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
