//! Contains a convenience wrapper around [`regex::Regex`].

use std::fmt::Debug;
use std::hash::Hash;
use std::ops::Deref;

use crate::test::Id as TestId;

/// A regex pattern whith extra traits for convenience.
///
/// This implements traits such a [`Eq`] witout regard for the internal
/// structure, it purely compares by looking at the source pattern.
#[derive(Clone)]
pub struct Regex(pub regex::Regex);

impl Regex {
    /// Creates a new [`Regex`] from the given pattern.
    pub fn new(pat: regex::Regex) -> Self {
        Self(pat)
    }
}

impl Regex {
    /// The inner regex pattern.
    pub fn as_regex(&self) -> &regex::Regex {
        &self.0
    }

    /// The inner string.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Regex {
    /// Returns true if the test id matches this pattern.
    pub fn is_match(&self, id: &TestId) -> bool {
        self.0.is_match(id.as_str())
    }
}

impl Debug for Regex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "\"{}\"", self.0.as_str())
    }
}

impl PartialEq for Regex {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_str() == other.0.as_str()
    }
}

impl Eq for Regex {}

impl Hash for Regex {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.as_str().hash(state);
    }
}

impl Deref for Regex {
    type Target = regex::Regex;

    fn deref(&self) -> &Self::Target {
        self.as_regex()
    }
}

impl AsRef<regex::Regex> for Regex {
    fn as_ref(&self) -> &regex::Regex {
        self.as_regex()
    }
}

impl AsRef<str> for Regex {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl From<regex::Regex> for Regex {
    fn from(value: regex::Regex) -> Self {
        Self(value)
    }
}
