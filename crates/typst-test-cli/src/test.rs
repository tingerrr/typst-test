use std::fmt::Display;
use std::path::PathBuf;

use typst_test_lib::{compare, compile};

pub mod runner;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Stage {
    Preparation,
    Hooks,
    Loading,
    Compilation,
    Saving,
    Rendering,
    Comparison,
    Update,
    Cleanup,
}

impl Display for Stage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Stage::Preparation => "preparation",
                Stage::Hooks => "hooks",
                Stage::Loading => "loading",
                Stage::Compilation => "compilation",
                Stage::Rendering => "rendering",
                Stage::Saving => "saving",
                Stage::Comparison => "comparison",
                Stage::Update => "update",
                Stage::Cleanup => "cleanup",
            }
        )
    }
}

// TODO: add a soft stage error for misinputs?
#[derive(Debug, Clone, thiserror::Error)]
#[error("test failed")]
pub enum TestFailure {
    Compilation(#[from] CompileFailure),
    Comparison(#[from] CompareFailure),
}

impl TestFailure {
    pub fn stage(&self) -> Stage {
        match self {
            TestFailure::Compilation(_) => Stage::Compilation,
            TestFailure::Comparison(_) => Stage::Comparison,
        }
    }
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("compilation failed")]
pub struct CompileFailure {
    pub is_ref: bool,
    pub error: compile::Error,
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("comparison failed")]
pub enum CompareFailure {
    Visual {
        error: compare::Error,
        diff_dir: Option<PathBuf>,
    },
}
