use std::hash::Hash;

use super::eval::{Context, Error, Eval, Value};
use super::{Glob, Regex};
use crate::test::Id as TestId;

/// A pattern matching identifiers of tests.
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Pat {
    /// A glob pattern, matches if the glob matches the haystack.
    Glob(Glob),

    /// A regex pattern, matches if the regex matches the haystack.
    Regex(Regex),

    /// An exact pattern, matches if the pattern equals the haystack.
    Exact(String),
}

impl std::fmt::Debug for Pat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (prefix, pat) = match self {
            Pat::Glob(glob) => ("glob", glob.as_str()),
            Pat::Regex(regex) => ("regex", regex.as_str()),
            Pat::Exact(pat) => ("exact", pat.as_str()),
        };

        write!(f, "{prefix}:{pat:?}")
    }
}

impl Pat {
    /// Returns true if the test id matches this pattern.
    pub fn is_match(&self, id: &TestId) -> bool {
        match self {
            Self::Glob(pat) => pat.is_match(id),
            Self::Regex(regex) => regex.is_match(id),
            Self::Exact(pat) => id.as_str() == pat.as_str(),
        }
    }
}

impl Eval for Pat {
    fn eval(&self, _ctx: &Context) -> Result<Value, Error> {
        Ok(Value::Pat(self.clone()))
    }
}
