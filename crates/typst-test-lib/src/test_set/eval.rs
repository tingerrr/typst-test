//! Evaluate test set expressions.

use std::fmt::Debug;
use std::sync::Arc;

use ecow::EcoString;
use regex::Regex;

use super::DynTestSet;
use crate::store::test::Test;
use crate::test::ReferenceKind;

/// A test set which contains all tests.
#[derive(Debug, Clone)]
pub struct AllTestSet;

impl super::TestSet for AllTestSet {
    fn contains(&self, _test: &Test) -> bool {
        true
    }
}

/// A test set which contains no tests.
#[derive(Debug, Clone)]
pub struct NoneTestSet;

impl super::TestSet for NoneTestSet {
    fn contains(&self, _test: &Test) -> bool {
        false
    }
}

/// A tet set which contains ignored tests.
#[derive(Debug, Clone)]
pub struct IgnoredTestSet;

impl super::TestSet for IgnoredTestSet {
    fn contains(&self, test: &Test) -> bool {
        test.is_ignored()
    }
}

/// A test set which contains tests of a certain [`ReferenceKind`].
#[derive(Debug, Clone)]
pub struct CustomTestSet {
    pub id: EcoString,
}

impl super::TestSet for CustomTestSet {
    fn contains(&self, test: &Test) -> bool {
        test.in_custom_test_set(&self.id)
    }
}

/// A test set which contains tests of a certain [`ReferenceKind`].
#[derive(Debug, Clone)]
pub struct KindTestSet {
    pub kind: Option<ReferenceKind>,
}

impl KindTestSet {
    /// A kind test set which contains compile only tests.
    pub fn compile_only() -> Self {
        Self { kind: None }
    }

    /// A kind test set which contains ephemeral tests.
    pub fn ephemeral() -> Self {
        Self {
            kind: Some(ReferenceKind::Ephemeral),
        }
    }

    /// A kind test set which contains persistent tests.
    pub fn persistent() -> Self {
        Self {
            kind: Some(ReferenceKind::Persistent),
        }
    }
}

impl super::TestSet for KindTestSet {
    fn contains(&self, test: &Test) -> bool {
        test.ref_kind() == self.kind.as_ref()
    }
}

/// A test set which contains tests matching the given identifier pattern.
#[derive(Debug, Clone)]
pub struct IdentifierTestSet {
    /// The pattern to use for identifier matching.
    pub pattern: IdentifierPattern,

    /// The target to match on.
    pub target: IdentiferTarget,
}

/// The target to apply the identifier pattern to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentiferTarget {
    /// Match on the whole identifier.
    Full,

    /// Match on the name part of the identifier.
    Name,

    /// Match on the module part of the identifier.
    Module,
}

/// A test set which contains tests by their identifiers.
#[derive(Debug, Clone)]
pub enum IdentifierPattern {
    /// Matches all tests which match the [`Regex`].
    Regex(Regex),

    /// Matches all tests which have exactly this name.
    Exact(EcoString),

    /// Matches all tests which contain the given term in their name.
    Contains(EcoString),
}

impl super::TestSet for IdentifierTestSet {
    fn contains(&self, test: &Test) -> bool {
        let id = test.id();
        let part = match self.target {
            IdentiferTarget::Full => id.as_str(),
            IdentiferTarget::Name => id.name(),
            IdentiferTarget::Module => id.module(),
        };

        match &self.pattern {
            IdentifierPattern::Regex(regex) => regex.is_match(part),
            IdentifierPattern::Exact(term) => part == term,
            IdentifierPattern::Contains(term) => part.contains(term.as_str()),
        }
    }
}

/// A unary operator test set.
#[derive(Debug, Clone)]
pub enum UnaryTestSet {
    /// Contains all tests which are not contained in the inner test set.
    Complement(DynTestSet),
}

impl super::TestSet for UnaryTestSet {
    fn contains(&self, test: &Test) -> bool {
        match self {
            UnaryTestSet::Complement(matcher) => !matcher.contains(test),
        }
    }
}

/// A binary operator test set.
#[derive(Debug, Clone)]
pub enum BinaryTestSet {
    /// Contains the union of the inner test sets, those tests that are
    /// contained in either test set.
    Union(DynTestSet, DynTestSet),

    /// Contains the set difference of the inner test sets, those tests that are
    /// contained in the left but not the right test set.
    Difference(DynTestSet, DynTestSet),

    /// Contains the symmetric difference of the inner test sets, those tests
    /// that are contained in only one test set, but not both.
    SymmetricDifference(DynTestSet, DynTestSet),

    /// Contains the intersection of the inner test sets, those tests
    /// that are contained in both test sets.
    Intersect(DynTestSet, DynTestSet),
}

impl super::TestSet for BinaryTestSet {
    fn contains(&self, test: &Test) -> bool {
        match self {
            BinaryTestSet::Union(m1, m2) => m1.contains(test) || m2.contains(test),
            BinaryTestSet::Difference(m1, m2) => m1.contains(test) && !m2.contains(test),
            BinaryTestSet::SymmetricDifference(m1, m2) => m1.contains(test) ^ m2.contains(test),
            BinaryTestSet::Intersect(m1, m2) => m1.contains(test) && m2.contains(test),
        }
    }
}

/// Returns the default test set, `!ignored`.
pub fn default() -> DynTestSet {
    Arc::new(UnaryTestSet::Complement(Arc::new(IgnoredTestSet)))
}

/// A test set for running an arbitray function on tests to check if they're
/// contained in the test set.
#[derive(Clone)]
pub struct FnTestSet {
    /// The closure to run on tests.
    pub custom: Arc<dyn Fn(&Test) -> bool + Send + Sync>,
}

impl Debug for FnTestSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CustomMatcher")
            .field("custom", &..)
            .finish()
    }
}

impl FnTestSet {
    /// Crates a new test set from the given closure.
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(&Test) -> bool + Send + Sync + 'static,
    {
        Self {
            custom: Arc::new(f),
        }
    }
}

impl super::TestSet for FnTestSet {
    fn contains(&self, test: &Test) -> bool {
        (self.custom)(test)
    }
}

#[cfg(test)]
mod tests {
    use ecow::eco_vec;

    use super::*;
    use crate::test::id::Identifier as TestIdentifier;
    use crate::test::{Annotation, ReferenceKind};
    use crate::test_set::TestSet;

    macro_rules! assert_matcher {
        ($m:expr, $matches:expr $(,)?) => {
            assert_eq!(
                [
                    ("mod/test-1", Some(ReferenceKind::Ephemeral), eco_vec![]),
                    (
                        "mod/test-2",
                        Some(ReferenceKind::Persistent),
                        eco_vec![Annotation::Custom("foo".into())]
                    ),
                    ("mod/other/test-1", None, eco_vec![]),
                    (
                        "mod/other/test-2",
                        Some(ReferenceKind::Ephemeral),
                        eco_vec![]
                    ),
                    (
                        "top-level",
                        None,
                        eco_vec![Annotation::Custom("foo".into())]
                    ),
                    (
                        "ignored",
                        Some(ReferenceKind::Persistent),
                        eco_vec![Annotation::Ignored]
                    ),
                ]
                .map(|(id, r, a)| Test::new_test(TestIdentifier::new(id).unwrap(), r, a))
                .iter()
                .map(|t| $m.contains(t))
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
    fn test_custom() {
        let m = CustomTestSet { id: "foo".into() };
        assert_matcher!(m, [false, true, false, false, true, false]);
    }

    #[test]
    fn test_name_regex() {
        let m = IdentifierTestSet {
            pattern: IdentifierPattern::Regex(Regex::new(r#"mod/.+/test"#).unwrap()),
            target: IdentiferTarget::Full,
        };
        assert_matcher!(m, [false, false, true, true, false, false]);

        let m = IdentifierTestSet {
            pattern: IdentifierPattern::Regex(Regex::new(r#"mod/.+"#).unwrap()),
            target: IdentiferTarget::Module,
        };
        assert_matcher!(m, [false, false, true, true, false, false]);

        let m = IdentifierTestSet {
            pattern: IdentifierPattern::Regex(Regex::new(r#"test-\d"#).unwrap()),
            target: IdentiferTarget::Name,
        };
        assert_matcher!(m, [true, true, true, true, false, false]);
    }

    #[test]
    fn test_name_contains() {
        let m = IdentifierTestSet {
            pattern: IdentifierPattern::Contains("-".into()),
            target: IdentiferTarget::Full,
        };
        assert_matcher!(m, [true, true, true, true, true, false]);

        let m = IdentifierTestSet {
            pattern: IdentifierPattern::Contains("d".into()),
            target: IdentiferTarget::Module,
        };
        assert_matcher!(m, [true, true, true, true, false, false]);

        let m = IdentifierTestSet {
            pattern: IdentifierPattern::Contains("d".into()),
            target: IdentiferTarget::Name,
        };
        assert_matcher!(m, [false, false, false, false, false, true]);
    }

    #[test]
    fn test_name_exact() {
        let m = IdentifierTestSet {
            pattern: IdentifierPattern::Exact("mod/test-1".into()),
            target: IdentiferTarget::Full,
        };
        assert_matcher!(m, [true, false, false, false, false, false]);

        let m = IdentifierTestSet {
            pattern: IdentifierPattern::Exact("mod".into()),
            target: IdentiferTarget::Module,
        };
        assert_matcher!(m, [true, true, false, false, false, false]);

        let m = IdentifierTestSet {
            pattern: IdentifierPattern::Exact("test-1".into()),
            target: IdentiferTarget::Name,
        };
        assert_matcher!(m, [true, false, true, false, false, false]);
    }

    #[test]
    fn test_kind() {
        let m = KindTestSet::compile_only();
        assert_matcher!(m, [false, false, true, false, true, false]);

        let m = KindTestSet::ephemeral();
        assert_matcher!(m, [true, false, false, true, false, false]);

        let m = KindTestSet::persistent();
        assert_matcher!(m, [false, true, false, false, false, true]);
    }

    #[test]
    fn test_ignored() {
        let m = IgnoredTestSet;
        assert_matcher!(m, [false, false, false, false, false, true]);
    }

    #[test]
    fn test_all() {
        let m = AllTestSet;
        assert_matcher!(m, [true, true, true, true, true, true]);
    }

    #[test]
    fn test_none() {
        let m = NoneTestSet;
        assert_matcher!(m, [false, false, false, false, false, false]);
    }

    #[test]
    fn test_complement() {
        let m = UnaryTestSet::Complement(Arc::new(IgnoredTestSet));
        assert_matcher!(m, [true, true, true, true, true, false]);
    }

    #[test]
    fn test_binary() {
        let m = BinaryTestSet::Union(
            Arc::new(IdentifierTestSet {
                pattern: IdentifierPattern::Regex(Regex::new(r#"test-\d"#).unwrap()),
                target: IdentiferTarget::Full,
            }),
            Arc::new(KindTestSet::compile_only()),
        );
        assert_matcher!(m, [true, true, true, true, true, false]);

        let m = BinaryTestSet::Intersect(
            Arc::new(IdentifierTestSet {
                pattern: IdentifierPattern::Regex(Regex::new(r#"test-\d"#).unwrap()),
                target: IdentiferTarget::Full,
            }),
            Arc::new(KindTestSet::compile_only()),
        );
        assert_matcher!(m, [false, false, true, false, false, false]);

        let m = BinaryTestSet::Difference(
            Arc::new(IdentifierTestSet {
                pattern: IdentifierPattern::Regex(Regex::new(r#"test-\d"#).unwrap()),
                target: IdentiferTarget::Full,
            }),
            Arc::new(KindTestSet::compile_only()),
        );
        assert_matcher!(m, [true, true, false, true, false, false]);

        let m = BinaryTestSet::SymmetricDifference(
            Arc::new(IdentifierTestSet {
                pattern: IdentifierPattern::Regex(Regex::new(r#"test-\d"#).unwrap()),
                target: IdentiferTarget::Full,
            }),
            Arc::new(KindTestSet::compile_only()),
        );
        assert_matcher!(m, [true, true, false, true, true, false]);
    }
}
