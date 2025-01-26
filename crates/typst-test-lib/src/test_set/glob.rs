//! Contains a convenience wrapper around [`glob::Pattern`].

use std::fmt::Debug;
use std::hash::Hash;
use std::ops::Deref;

use crate::test::Id as TestId;

/// A glob pattern.
#[derive(Clone)]
pub struct Glob(pub glob::Pattern);

impl Glob {
    /// Creates a new [`Glob`] from the given pattern.
    pub fn new(pat: glob::Pattern) -> Self {
        Self(pat)
    }
}

impl Glob {
    /// The inner glob pattern.
    pub fn as_glob(&self) -> &glob::Pattern {
        &self.0
    }

    /// The inner string.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Glob {
    /// Returns true if the test id matches this pattern.
    pub fn is_match(&self, id: &TestId) -> bool {
        self.0.matches(id.as_str())
    }
}

impl Debug for Glob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "\"{}\"", self.0.as_str())
    }
}

impl PartialEq for Glob {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_str() == other.0.as_str()
    }
}

impl Eq for Glob {}

impl Hash for Glob {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.as_str().hash(state);
    }
}

impl Deref for Glob {
    type Target = glob::Pattern;

    fn deref(&self) -> &Self::Target {
        self.as_glob()
    }
}

impl AsRef<glob::Pattern> for Glob {
    fn as_ref(&self) -> &glob::Pattern {
        self.as_glob()
    }
}

impl AsRef<str> for Glob {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl From<glob::Pattern> for Glob {
    fn from(value: glob::Pattern) -> Self {
        Self(value)
    }
}
