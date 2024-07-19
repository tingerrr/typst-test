use std::fmt::Debug;
use std::sync::Arc;

use ecow::EcoString;
use regex::Regex;

use super::DynTestSet;
use crate::store::test::Test;
use crate::test::ReferenceKind;

/// A matcher which matches all tests.
#[derive(Debug, Clone)]
pub struct AllMatcher;

impl super::TestSet for AllMatcher {
    fn is_match(&self, _test: &Test) -> bool {
        true
    }
}

/// A matcher which matches no tests.
#[derive(Debug, Clone)]
pub struct NoneMatcher;

impl super::TestSet for NoneMatcher {
    fn is_match(&self, _test: &Test) -> bool {
        false
    }
}

/// A matcher which matches all ignored tests.
#[derive(Debug, Clone)]
pub struct IgnoredMatcher;

impl super::TestSet for IgnoredMatcher {
    fn is_match(&self, test: &Test) -> bool {
        test.is_ignored()
    }
}

/// A matcher which matches all ignored tests.
#[derive(Debug, Clone)]
pub struct KindMatcher {
    pub kind: Option<ReferenceKind>,
}
impl KindMatcher {
    /// A kind matcher whcih matches on compile only tests.
    pub fn compile_only() -> Self {
        Self { kind: None }
    }

    /// A kind matcher whcih matches on ephemeral tests.
    pub fn ephemeral() -> Self {
        Self {
            kind: Some(ReferenceKind::Ephemeral),
        }
    }

    /// A kind matcher whcih matches on persistent tests.
    pub fn persistent() -> Self {
        Self {
            kind: Some(ReferenceKind::Persistent),
        }
    }
}

impl super::TestSet for KindMatcher {
    fn is_match(&self, test: &Test) -> bool {
        test.ref_kind() == self.kind.as_ref()
    }
}

/// A matcher which matches tests by their identifiers.
#[derive(Debug, Clone)]
pub struct IdentifierMatcher {
    /// The pattern to use for identifier matching.
    pub pattern: IdentifierMatcherPattern,

    /// The target to match on.
    pub target: IdentiferMatcherTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentiferMatcherTarget {
    /// Match on the whole identifier.
    Full,

    /// Match on the name part of the identifier.
    Name,

    /// Match on the module part of the identifier.
    Module,
}

/// A matcher which matches tests by their identifiers.
#[derive(Debug, Clone)]
pub enum IdentifierMatcherPattern {
    /// Matches all tests which match the [`Regex`].
    Regex(Regex),

    /// Matches all tests which have exactly this name.
    Exact(EcoString),

    /// Matches all tests which contain the given term in their name.
    Contains(EcoString),
}

impl super::TestSet for IdentifierMatcher {
    fn is_match(&self, test: &Test) -> bool {
        let id = test.id();
        let part = match self.target {
            IdentiferMatcherTarget::Full => id.as_str(),
            IdentiferMatcherTarget::Name => id.name(),
            IdentiferMatcherTarget::Module => id.module(),
        };

        match &self.pattern {
            IdentifierMatcherPattern::Regex(regex) => regex.is_match(part),
            IdentifierMatcherPattern::Exact(term) => part == term,
            IdentifierMatcherPattern::Contains(term) => part.contains(term.as_str()),
        }
    }
}

/// A unary operator matcher.
#[derive(Debug, Clone)]
pub enum UnaryMatcher {
    /// Matches all tests which don't match the inner matcher.
    Complement(DynTestSet),
}

impl super::TestSet for UnaryMatcher {
    fn is_match(&self, test: &Test) -> bool {
        match self {
            UnaryMatcher::Complement(matcher) => !matcher.is_match(test),
        }
    }
}

/// A binary operator matcher.
#[derive(Debug, Clone)]
pub enum BinaryMatcher {
    /// Matches the union of the inner matchers, those tests that match either
    /// matcher.
    Union(DynTestSet, DynTestSet),

    /// Matches the set difference of the inner matchers, those tests that match
    /// the left but not the right matcher.
    Difference(DynTestSet, DynTestSet),

    /// Matches the symmetric difference of the inner matchers, those tests that
    /// match only one matcher, but not both.
    SymmetricDifference(DynTestSet, DynTestSet),

    /// Matches the intersection of the inner matchers, those tests
    /// that match both.
    Intersect(DynTestSet, DynTestSet),
}

impl super::TestSet for BinaryMatcher {
    fn is_match(&self, test: &Test) -> bool {
        match self {
            BinaryMatcher::Union(m1, m2) => m1.is_match(test) || m2.is_match(test),
            BinaryMatcher::Difference(m1, m2) => m1.is_match(test) && !m2.is_match(test),
            BinaryMatcher::SymmetricDifference(m1, m2) => m1.is_match(test) ^ m2.is_match(test),
            BinaryMatcher::Intersect(m1, m2) => m1.is_match(test) && m2.is_match(test),
        }
    }
}

/// Returns the default test set.
pub fn default() -> DynTestSet {
    Arc::new(UnaryMatcher::Complement(Arc::new(IgnoredMatcher)))
}

/// A matcher for running an arbitray function on tests.
#[derive(Clone)]
pub struct FnMatcher {
    /// The closure to run on tests.
    pub custom: Arc<dyn Fn(&Test) -> bool + Send + Sync>,
}

impl Debug for FnMatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CustomMatcher")
            .field("custom", &..)
            .finish()
    }
}

impl FnMatcher {
    /// Crates a new matcher from the given closure.
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(&Test) -> bool + Send + Sync + 'static,
    {
        Self {
            custom: Arc::new(f),
        }
    }
}

impl super::TestSet for FnMatcher {
    fn is_match(&self, test: &Test) -> bool {
        (self.custom)(test)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::id::Identifier;
    use crate::test::ReferenceKind;
    use crate::test_set::TestSet;

    macro_rules! assert_matcher {
        ($m:expr, $matches:expr $(,)?) => {
            assert_eq!(
                [
                    ("mod/test-1", Some(ReferenceKind::Ephemeral), false),
                    ("mod/test-2", Some(ReferenceKind::Persistent), false),
                    ("mod/other/test-1", None, false),
                    ("mod/other/test-2", Some(ReferenceKind::Ephemeral), false),
                    ("top-level", None, false),
                    ("ignored", Some(ReferenceKind::Persistent), true),
                ]
                .map(|(id, r, i)| Test::new_test(Identifier::new(id).unwrap(), r, i,))
                .iter()
                .map(|t| $m.is_match(t))
                .collect::<Vec<_>>(),
                $matches,
            );
        };
    }

    #[test]
    fn test_default() {
        let m = default();
        assert_matcher!(m, [true, true, true, true, true, false]);
    }

    #[test]
    fn test_name_regex() {
        let m = IdentifierMatcher {
            pattern: IdentifierMatcherPattern::Regex(Regex::new(r#"mod/.+/test"#).unwrap()),
            target: IdentiferMatcherTarget::Full,
        };
        assert_matcher!(m, [false, false, true, true, false, false]);

        let m = IdentifierMatcher {
            pattern: IdentifierMatcherPattern::Regex(Regex::new(r#"mod/.+"#).unwrap()),
            target: IdentiferMatcherTarget::Module,
        };
        assert_matcher!(m, [false, false, true, true, false, false]);

        let m = IdentifierMatcher {
            pattern: IdentifierMatcherPattern::Regex(Regex::new(r#"test-\d"#).unwrap()),
            target: IdentiferMatcherTarget::Name,
        };
        assert_matcher!(m, [true, true, true, true, false, false]);
    }

    #[test]
    fn test_name_contains() {
        let m = IdentifierMatcher {
            pattern: IdentifierMatcherPattern::Contains("-".into()),
            target: IdentiferMatcherTarget::Full,
        };
        assert_matcher!(m, [true, true, true, true, true, false]);

        let m = IdentifierMatcher {
            pattern: IdentifierMatcherPattern::Contains("d".into()),
            target: IdentiferMatcherTarget::Module,
        };
        assert_matcher!(m, [true, true, true, true, false, false]);

        let m = IdentifierMatcher {
            pattern: IdentifierMatcherPattern::Contains("d".into()),
            target: IdentiferMatcherTarget::Name,
        };
        assert_matcher!(m, [false, false, false, false, false, true]);
    }

    #[test]
    fn test_name_exact() {
        let m = IdentifierMatcher {
            pattern: IdentifierMatcherPattern::Exact("mod/test-1".into()),
            target: IdentiferMatcherTarget::Full,
        };
        assert_matcher!(m, [true, false, false, false, false, false]);

        let m = IdentifierMatcher {
            pattern: IdentifierMatcherPattern::Exact("mod".into()),
            target: IdentiferMatcherTarget::Module,
        };
        assert_matcher!(m, [true, true, false, false, false, false]);

        let m = IdentifierMatcher {
            pattern: IdentifierMatcherPattern::Exact("test-1".into()),
            target: IdentiferMatcherTarget::Name,
        };
        assert_matcher!(m, [true, false, true, false, false, false]);
    }

    #[test]
    fn test_kind() {
        let m = KindMatcher::compile_only();
        assert_matcher!(m, [false, false, true, false, true, false]);

        let m = KindMatcher::ephemeral();
        assert_matcher!(m, [true, false, false, true, false, false]);

        let m = KindMatcher::persistent();
        assert_matcher!(m, [false, true, false, false, false, true]);
    }

    #[test]
    fn test_ignored() {
        let m = IgnoredMatcher;
        assert_matcher!(m, [false, false, false, false, false, true]);
    }

    #[test]
    fn test_all() {
        let m = AllMatcher;
        assert_matcher!(m, [true, true, true, true, true, true]);
    }

    #[test]
    fn test_none() {
        let m = NoneMatcher;
        assert_matcher!(m, [false, false, false, false, false, false]);
    }

    #[test]
    fn test_complement() {
        let m = UnaryMatcher::Complement(Arc::new(IgnoredMatcher));
        assert_matcher!(m, [true, true, true, true, true, false]);
    }

    #[test]
    fn test_binary() {
        let m = BinaryMatcher::Union(
            Arc::new(IdentifierMatcher {
                pattern: IdentifierMatcherPattern::Regex(Regex::new(r#"test-\d"#).unwrap()),
                target: IdentiferMatcherTarget::Full,
            }),
            Arc::new(KindMatcher::compile_only()),
        );
        assert_matcher!(m, [true, true, true, true, true, false]);

        let m = BinaryMatcher::Intersect(
            Arc::new(IdentifierMatcher {
                pattern: IdentifierMatcherPattern::Regex(Regex::new(r#"test-\d"#).unwrap()),
                target: IdentiferMatcherTarget::Full,
            }),
            Arc::new(KindMatcher::compile_only()),
        );
        assert_matcher!(m, [false, false, true, false, false, false]);

        let m = BinaryMatcher::Difference(
            Arc::new(IdentifierMatcher {
                pattern: IdentifierMatcherPattern::Regex(Regex::new(r#"test-\d"#).unwrap()),
                target: IdentiferMatcherTarget::Full,
            }),
            Arc::new(KindMatcher::compile_only()),
        );
        assert_matcher!(m, [true, true, false, true, false, false]);

        let m = BinaryMatcher::SymmetricDifference(
            Arc::new(IdentifierMatcher {
                pattern: IdentifierMatcherPattern::Regex(Regex::new(r#"test-\d"#).unwrap()),
                target: IdentiferMatcherTarget::Full,
            }),
            Arc::new(KindMatcher::compile_only()),
        );
        assert_matcher!(m, [true, true, false, true, true, false]);
    }
}
