use std::fmt::Debug;

use ecow::EcoString;
use regex::Regex;

use super::Test;
use crate::test::id::Identifier;
use crate::test::ReferenceKind;

/// A matcher which is applied to [`Identifier`]s when tests are collected.
#[derive(Debug, Clone)]
pub enum IdentifierMatcher {
    /// A regex filter, filters out tests which don't match the [`Regex`].
    Regex(Regex),

    /// A simple string filter.
    Simple {
        /// The term to match with.
        term: EcoString,

        /// Whether the term must be matched exactly. If this is `false` the
        /// term must only be contained within the test identifier.
        exact: bool,
    },
}

impl IdentifierMatcher {
    /// Returns whether this test's identifier matches.
    pub fn is_match(&self, id: &Identifier) -> bool {
        let id = id.as_str();
        match self {
            IdentifierMatcher::Regex(regex) => {
                if regex.is_match(id) {
                    return true;
                }
            }
            IdentifierMatcher::Simple { term, exact: true } => {
                if id == term {
                    return true;
                }
            }
            IdentifierMatcher::Simple { term, exact: false } => {
                if id.contains(term.as_str()) {
                    return true;
                }
            }
        }

        false
    }
}

/// A matcher which is applied to tests when they are collected.
pub struct Matcher {
    pub filter_ignored: bool,
    pub include_compile_only: bool,
    pub include_ephemeral: bool,
    pub include_persistent: bool,
    pub name: Option<IdentifierMatcher>,
    pub custom: Option<Box<dyn Fn(&Test) -> bool>>,
}

impl Debug for Matcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Matcher")
            .field("filter_ignored", &self.filter_ignored)
            .field("include_compile_only", &self.include_compile_only)
            .field("include_ephemeral", &self.include_ephemeral)
            .field("include_persistent", &self.include_persistent)
            .field("name", &self.name)
            .field("custom", &self.custom.as_ref().map(|_| ..))
            .finish()
    }
}

impl Matcher {
    /// Creates a new default matcher.
    pub fn new() -> Self {
        Self {
            filter_ignored: true,
            include_compile_only: true,
            include_ephemeral: true,
            include_persistent: true,
            name: None,
            custom: None,
        }
    }

    /// Set whether tests which are marked as ignored should be filtered out,
    /// defaults to `true`. Setting this to false is primarily useful for listing
    /// tests.
    pub fn filter_ignored(&mut self, yes: bool) -> &mut Self {
        self.filter_ignored = yes;
        self
    }

    /// Sets whether compile only tests should be included, defaults to `false`.
    pub fn include_compile_only(&mut self, include: bool) -> &mut Self {
        self.include_compile_only = include;
        self
    }

    /// Sets whether ephemeral tests should be included, defaults to `false`.
    pub fn include_ephemeral(&mut self, include: bool) -> &mut Self {
        self.include_ephemeral = include;
        self
    }

    /// Sets whether persistent tests should be included, defaults to `false`.
    pub fn include_persistent(&mut self, include: bool) -> &mut Self {
        self.include_persistent = include;
        self
    }

    /// Sets the name matcher, defaults to `None`.
    pub fn name(&mut self, matcher: Option<IdentifierMatcher>) -> &mut Self {
        self.name = matcher;
        self
    }

    /// Sets the custom matcher, defaults to `None`.
    pub fn custom(&mut self, matcher: Option<Box<dyn Fn(&Test) -> bool>>) -> &mut Self {
        self.custom = matcher;
        self
    }

    /// Returns `true` if all contained matchers match the test, else `false`.
    /// The matchers are short circuiting and the custom matcher is tested last,
    /// which means, if a name matcher fails the custom matcher is not tested.
    pub fn is_match(&self, test: &Test) -> bool {
        if self.filter_ignored && test.is_ignored() {
            return false;
        }

        if !match test.ref_kind() {
            Some(ReferenceKind::Ephemeral) => self.include_ephemeral,
            Some(ReferenceKind::Persistent) => self.include_persistent,
            None => self.include_compile_only,
        } {
            return false;
        }

        if let Some(filter) = &self.name {
            if !filter.is_match(test.id()) {
                return false;
            }
        }

        if let Some(filter) = &self.custom {
            if !(filter)(test) {
                return false;
            }
        }

        true
    }
}

impl Default for Matcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::ReferenceKind;

    macro_rules! assert_matcher {
        ($m:expr, $matches:expr $(,)?) => {
            assert_eq!(
                [
                    ("mod/test-1", Some(ReferenceKind::Ephemeral), false),
                    ("mod/test-2", Some(ReferenceKind::Persistent), false),
                    ("mod/other/test-1", Some(ReferenceKind::Persistent), false),
                    ("mod/other/test-2", Some(ReferenceKind::Ephemeral), false),
                    ("top-level", None, false),
                    ("ignored", None, true),
                ]
                .map(|(id, r, i)| Test {
                    id: Identifier::new(id).unwrap(),
                    ref_kind: r,
                    is_ignored: i,
                })
                .iter()
                .map($m)
                .collect::<Vec<_>>(),
                $matches,
            );
        };
    }

    #[test]
    fn test_name_regex() {
        let m = IdentifierMatcher::Regex(Regex::new(r#"mod/.+/test"#).unwrap());
        assert_matcher!(
            |t| m.is_match(t.id()),
            [false, false, true, true, false, false],
        );
    }

    #[test]
    fn test_name_contains() {
        let m = IdentifierMatcher::Simple {
            term: "-".into(),
            exact: false,
        };
        assert_matcher!(
            |t| m.is_match(t.id()),
            [true, true, true, true, true, false],
        );
    }

    #[test]
    fn test_name_exact() {
        let m = IdentifierMatcher::Simple {
            term: "mod/test-1".into(),
            exact: true,
        };
        assert_matcher!(
            |t| m.is_match(t.id()),
            [true, false, false, false, false, false],
        );
    }

    #[test]
    fn test_kind_compare_only() {
        let m = Matcher {
            filter_ignored: false,
            include_compile_only: false,
            include_ephemeral: true,
            include_persistent: true,
            name: None,
            custom: None,
        };
        assert_matcher!(|t| m.is_match(t), [true, true, true, true, false, false],);
    }

    #[test]
    fn test_kind_compile_only() {
        let m = Matcher {
            filter_ignored: false,
            include_compile_only: true,
            include_ephemeral: false,
            include_persistent: false,
            name: None,
            custom: None,
        };
        assert_matcher!(|t| m.is_match(t), [false, false, false, false, true, true],);
    }

    #[test]
    fn test_ignored() {
        let m = Matcher {
            filter_ignored: true,
            include_compile_only: true,
            include_ephemeral: true,
            include_persistent: true,
            name: None,
            custom: None,
        };
        assert_matcher!(|t| m.is_match(t), [true, true, true, true, true, false],);
    }
}
