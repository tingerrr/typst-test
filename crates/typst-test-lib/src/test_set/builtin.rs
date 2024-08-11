//! Builtin test set constructors.

use std::fmt::Debug;
use std::sync::Arc;

use ecow::EcoString;
use glob::Pattern;
use regex::Regex;

use super::ast::{InfixOp, PatternKind, PostfixOp, PrefixOp};
use super::eval::{Context, Error as EvalError, TestSet, Value};
use super::{DynFunction, DynTestSet, Function};
use crate::store::test::Test;
use crate::test::{Annotation, ReferenceKind};

/// A test set which contains all tests.
#[derive(Debug, Clone)]
pub struct AllTestSet;

impl TestSet for AllTestSet {
    fn contains(&self, _test: &Test) -> bool {
        true
    }
}

/// A test set which contains no tests.
#[derive(Debug, Clone)]
pub struct NoneTestSet;

impl TestSet for NoneTestSet {
    fn contains(&self, _test: &Test) -> bool {
        false
    }
}

/// A test set which contains ignored tests.
#[derive(Debug, Clone)]
pub struct IgnoredTestSet;

impl TestSet for IgnoredTestSet {
    fn contains(&self, test: &Test) -> bool {
        test.is_ignored()
    }
}

/// A test set which contains tests of a certain [`ReferenceKind`].
#[derive(Debug, Clone)]
pub struct CustomTestSet {
    pub pattern: IdentifierPattern,
}

impl TestSet for CustomTestSet {
    fn contains(&self, test: &Test) -> bool {
        test.annotations()
            .iter()
            .any(|annot| matches!(annot, Annotation::Custom(ident) if self.pattern.matches(ident.as_str())))
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

impl TestSet for KindTestSet {
    fn contains(&self, test: &Test) -> bool {
        test.ref_kind() == self.kind
    }
}

/// A test set which contains tests matching the given identifier pattern.
#[derive(Debug, Clone)]
pub struct IdentifierTestSet {
    /// The pattern to use for identifier matching.
    pub pattern: IdentifierPattern,

    /// The target to match on.
    pub target: IdentifierTarget,
}

/// The target to apply the identifier pattern to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentifierTarget {
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
    Regex(Arc<Regex>),

    /// Matches all tests which match the glob [`Pattern`].
    Glob(Arc<Pattern>),

    /// Matches all tests which have exactly this name.
    Exact(EcoString),

    /// Matches all tests which contain the given term in their name.
    Contains(EcoString),
}

impl IdentifierPattern {
    /// Returns `true` if the identifier matches this pattern.
    pub fn matches(&self, id: &str) -> bool {
        match self {
            IdentifierPattern::Regex(regex) => regex.is_match(id),
            IdentifierPattern::Exact(term) => id == term,
            IdentifierPattern::Contains(term) => id.contains(term.as_str()),
            IdentifierPattern::Glob(glob) => glob.matches(id),
        }
    }
}

impl TestSet for IdentifierTestSet {
    fn contains(&self, test: &Test) -> bool {
        let id = test.id();
        let part = match self.target {
            IdentifierTarget::Full => id.as_str(),
            IdentifierTarget::Name => id.name(),
            IdentifierTarget::Module => id.module(),
        };

        self.pattern.matches(part)
    }
}

/// A function returning [`IdentifierTestSet`]s.
#[derive(Debug, Clone)]
pub struct IdentifierTestSetFunction {
    pub target: IdentifierTarget,
}

impl Function for IdentifierTestSetFunction {
    fn call(&self, _ctx: &Context, args: &[Value]) -> Result<Value, EvalError> {
        if args.len() != 1 {
            return Err(EvalError::InvalidArgumentCount {
                expected: 1,
                found: args.len(),
            });
        }

        let pat = args[0].to_pattern()?.clone();

        let pattern = match pat.kind {
            PatternKind::Exact => IdentifierPattern::Exact(pat.value.clone()),
            PatternKind::Contains => IdentifierPattern::Contains(pat.value.clone()),
            PatternKind::Regex => IdentifierPattern::Regex(Arc::new(Regex::new(&pat.value)?)),
            PatternKind::Glob => IdentifierPattern::Glob(Arc::new(Pattern::new(&pat.value)?)),
        };

        Ok(Value::TestSet(Arc::new(IdentifierTestSet {
            pattern,
            target: self.target,
        })))
    }
}

/// A function returning [`CustomTestSet`]s.
#[derive(Debug, Clone)]
pub struct CustomTestSetFunction;

impl Function for CustomTestSetFunction {
    fn call(&self, _ctx: &Context, args: &[Value]) -> Result<Value, EvalError> {
        if args.len() != 1 {
            return Err(EvalError::InvalidArgumentCount {
                expected: 1,
                found: args.len(),
            });
        }

        let pat = args[0].to_pattern()?.clone();

        let pattern = match pat.kind {
            PatternKind::Exact => IdentifierPattern::Exact(pat.value.clone()),
            PatternKind::Contains => IdentifierPattern::Contains(pat.value.clone()),
            PatternKind::Regex => IdentifierPattern::Regex(Arc::new(Regex::new(&pat.value)?)),
            PatternKind::Glob => IdentifierPattern::Glob(Arc::new(Pattern::new(&pat.value)?)),
        };

        Ok(Value::TestSet(Arc::new(CustomTestSet { pattern })))
    }
}

/// A prefix operator test set expression.
#[derive(Debug, Clone)]
pub struct PrefixTestSet {
    /// The operator of this expression.
    pub op: PrefixOp,

    /// The inner expression.
    pub test_set: DynTestSet,
}

impl PrefixTestSet {
    /// Creates a new [`PrefixTestSet`].
    ///
    /// Wraps the `expr` in an `Arc`.
    pub fn new<T>(op: PrefixOp, expr: T) -> Self
    where
        T: TestSet,
    {
        Self {
            op,
            test_set: Arc::new(expr),
        }
    }

    /// Creates a new [`PrefixTestSet`] with a [`PrefixOp::Complement`].
    ///
    /// Wraps the `expr` in an `Arc`.
    pub fn complement<T>(expr: T) -> Self
    where
        T: TestSet,
    {
        Self::new(PrefixOp::Complement, expr)
    }
}

impl TestSet for PrefixTestSet {
    fn contains(&self, test: &Test) -> bool {
        match self.op {
            PrefixOp::Complement => !self.test_set.contains(test),
        }
    }
}

/// A binary operator test set.
#[derive(Debug, Clone)]
pub struct InfixTestSet {
    /// The operator of this expression.
    pub op: InfixOp,

    /// The left hand side of this expression.
    pub lhs: DynTestSet,

    /// The right hand side of this expression.
    pub rhs: DynTestSet,
}

impl InfixTestSet {
    /// Creates a new [`InfixTestSet`].
    ///
    /// Wraps the `lhs` and `rhs` in an `Arc`.
    pub fn new<L, R>(op: InfixOp, lhs: L, rhs: R) -> Self
    where
        L: TestSet,
        R: TestSet,
    {
        Self {
            op,
            lhs: Arc::new(lhs),
            rhs: Arc::new(rhs),
        }
    }

    /// Creates a new [`InfixTestSet`] with a [`InfixOp::SymmetricDifference`].
    ///
    /// Wraps the `lhs` and `rhs` in an `Arc`.
    pub fn symmetric_difference<L, R>(lhs: L, rhs: R) -> Self
    where
        L: TestSet,
        R: TestSet,
    {
        Self::new(InfixOp::SymmetricDifference, lhs, rhs)
    }

    /// Creates a new [`InfixTestSet`] with a [`InfixOp::Difference`].
    ///
    /// Wraps the `lhs` and `rhs` in an `Arc`.
    pub fn difference<L, R>(lhs: L, rhs: R) -> Self
    where
        L: TestSet,
        R: TestSet,
    {
        Self::new(InfixOp::Difference, lhs, rhs)
    }

    /// Creates a new [`InfixTestSet`] with a [`InfixOp::Intersection`].
    ///
    /// Wraps the `lhs` and `rhs` in an `Arc`.
    pub fn intersection<L, R>(lhs: L, rhs: R) -> Self
    where
        L: TestSet,
        R: TestSet,
    {
        Self::new(InfixOp::Intersection, lhs, rhs)
    }

    /// Creates a new [`InfixTestSet`] with a [`InfixOp::Union`].
    ///
    /// Wraps the `lhs` and `rhs` in an `Arc`.
    pub fn union<L, R>(lhs: L, rhs: R) -> Self
    where
        L: TestSet,
        R: TestSet,
    {
        Self::new(InfixOp::Union, lhs, rhs)
    }
}

impl TestSet for InfixTestSet {
    fn contains(&self, test: &Test) -> bool {
        match self.op {
            InfixOp::SymmetricDifference => self.lhs.contains(test) ^ self.rhs.contains(test),
            // TODO: this may be an optimization opportunity, could be
            // distirbuted according to DeMorgan
            InfixOp::Difference => self.lhs.contains(test) && !self.rhs.contains(test),
            InfixOp::Intersection => self.lhs.contains(test) && self.rhs.contains(test),
            InfixOp::Union => self.lhs.contains(test) || self.rhs.contains(test),
        }
    }
}

/// A prefix operator test set expression.
#[derive(Debug, Clone)]
pub struct PostfixTestSet {
    /// The operator of this expression.
    pub op: PostfixOp,

    /// The inner expression.
    pub test_set: DynTestSet,
}

impl PostfixTestSet {
    /// Creates a new [`PostfixTestSet`].
    ///
    /// Wraps the `expr` in an `Arc`.
    pub fn new<T>(op: PostfixOp, expr: T) -> Self
    where
        T: TestSet,
    {
        Self {
            op,
            test_set: Arc::new(expr),
        }
    }

    /// Creates a new [`PostfixTestSet`] with a [`PostfixOp::Ancestors`].
    ///
    /// Wraps the `expr` in an `Arc`.
    pub fn ancestors<T>(expr: T) -> Self
    where
        T: TestSet,
    {
        Self::new(PostfixOp::Ancestors, expr)
    }

    /// Creates a new [`PostfixTestSet`] with a [`PostfixOp::Descendants`].
    ///
    /// Wraps the `expr` in an `Arc`.
    pub fn descendants<T>(expr: T) -> Self
    where
        T: TestSet,
    {
        Self::new(PostfixOp::Descendants, expr)
    }
}

impl TestSet for PostfixTestSet {
    fn contains(&self, _test: &Test) -> bool {
        match self.op {
            PostfixOp::Ancestors => unimplemented!("ancestors test set is not yet implemented"),
            PostfixOp::Descendants => unimplemented!("descendants test set is not yet implemented"),
        }
    }
}

/// A test set for running an arbitrary function on tests to check if they're
/// contained in the test set.
#[derive(Clone)]
pub struct FnTestSet {
    /// The closure to run on tests.
    pub custom: Arc<dyn Fn(&Test) -> bool + Send + Sync>,
}

impl Debug for FnTestSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FnTestSet").field("custom", &..).finish()
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

impl TestSet for FnTestSet {
    fn contains(&self, test: &Test) -> bool {
        (self.custom)(test)
    }
}

/// Returns the `none` test set.
pub fn none() -> DynTestSet {
    Arc::new(NoneTestSet)
}

/// Returns the `all` test set.
pub fn all() -> DynTestSet {
    Arc::new(AllTestSet)
}

/// Returns the `ignored` test set.
pub fn ignored() -> DynTestSet {
    Arc::new(IgnoredTestSet)
}

/// Returns the `compile-only` test set.
pub fn compile_only() -> DynTestSet {
    Arc::new(KindTestSet::compile_only())
}

/// Returns the `ephemeral` test set.
pub fn ephemeral() -> DynTestSet {
    Arc::new(KindTestSet::ephemeral())
}

/// Returns the `persistent` test set.
pub fn persistent() -> DynTestSet {
    Arc::new(KindTestSet::persistent())
}

/// Returns the `default` test set, an alias for `!ignored`.
pub fn default() -> DynTestSet {
    Arc::new(PrefixTestSet::complement(IgnoredTestSet))
}

/// Creates the `id()` test set constructor.
pub fn id() -> DynFunction {
    Arc::new(IdentifierTestSetFunction {
        target: IdentifierTarget::Full,
    })
}

/// Creates the `mod()` test set constructor.
pub fn mod_() -> DynFunction {
    Arc::new(IdentifierTestSetFunction {
        target: IdentifierTarget::Module,
    })
}

/// Creates the `name()` test set constructor.
pub fn name() -> DynFunction {
    Arc::new(IdentifierTestSetFunction {
        target: IdentifierTarget::Name,
    })
}

/// Creates the `custom()` test set constructor.
pub fn custom() -> DynFunction {
    Arc::new(CustomTestSetFunction)
}

/// Creates a new test set which is defined by the given matcher function.
pub fn from_fn<F: Fn(&Test) -> bool + Send + Sync + 'static>(f: F) -> DynTestSet {
    Arc::new(FnTestSet::new(f))
}

#[cfg(test)]
mod tests {
    use ecow::eco_vec;

    use super::*;
    use crate::test::id::Identifier as TestIdentifier;
    use crate::test::{Annotation, ReferenceKind};
    use crate::test_set::Identifier;

    macro_rules! assert_matcher {
        ($m:expr, $matches:expr $(,)?) => {
            assert_eq!(
                [
                    ("mod/test-1", Some(ReferenceKind::Ephemeral), eco_vec![]),
                    (
                        "mod/test-2",
                        Some(ReferenceKind::Persistent),
                        eco_vec![Annotation::Custom(Identifier::new("foo").unwrap())]
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
                        eco_vec![Annotation::Custom(Identifier::new("foo").unwrap())]
                    ),
                    (
                        "ignored",
                        Some(ReferenceKind::Persistent),
                        eco_vec![Annotation::Ignored]
                    ),
                ]
                .map(|(id, r, a)| Test::new_full(TestIdentifier::new(id).unwrap(), r, a))
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
        let m = CustomTestSet {
            pattern: IdentifierPattern::Contains("foo".into()),
        };
        assert_matcher!(m, [false, true, false, false, true, false]);
    }

    #[test]
    fn test_name_regex() {
        let m = IdentifierTestSet {
            pattern: IdentifierPattern::Regex(Arc::new(Regex::new(r#"mod/.+/test"#).unwrap())),
            target: IdentifierTarget::Full,
        };
        assert_matcher!(m, [false, false, true, true, false, false]);

        let m = IdentifierTestSet {
            pattern: IdentifierPattern::Regex(Arc::new(Regex::new(r#"mod/.+"#).unwrap())),
            target: IdentifierTarget::Module,
        };
        assert_matcher!(m, [false, false, true, true, false, false]);

        let m = IdentifierTestSet {
            pattern: IdentifierPattern::Regex(Arc::new(Regex::new(r#"test-\d"#).unwrap())),
            target: IdentifierTarget::Name,
        };
        assert_matcher!(m, [true, true, true, true, false, false]);
    }

    #[test]
    fn test_name_contains() {
        let m = IdentifierTestSet {
            pattern: IdentifierPattern::Contains("-".into()),
            target: IdentifierTarget::Full,
        };
        assert_matcher!(m, [true, true, true, true, true, false]);

        let m = IdentifierTestSet {
            pattern: IdentifierPattern::Contains("d".into()),
            target: IdentifierTarget::Module,
        };
        assert_matcher!(m, [true, true, true, true, false, false]);

        let m = IdentifierTestSet {
            pattern: IdentifierPattern::Contains("d".into()),
            target: IdentifierTarget::Name,
        };
        assert_matcher!(m, [false, false, false, false, false, true]);
    }

    #[test]
    fn test_name_exact() {
        let m = IdentifierTestSet {
            pattern: IdentifierPattern::Exact("mod/test-1".into()),
            target: IdentifierTarget::Full,
        };
        assert_matcher!(m, [true, false, false, false, false, false]);

        let m = IdentifierTestSet {
            pattern: IdentifierPattern::Exact("mod".into()),
            target: IdentifierTarget::Module,
        };
        assert_matcher!(m, [true, true, false, false, false, false]);

        let m = IdentifierTestSet {
            pattern: IdentifierPattern::Exact("test-1".into()),
            target: IdentifierTarget::Name,
        };
        assert_matcher!(m, [true, false, true, false, false, false]);
    }

    #[test]
    fn test_name_glob() {
        let m = IdentifierTestSet {
            pattern: IdentifierPattern::Glob(Arc::new(Pattern::new("mod/**/").unwrap())),
            target: IdentifierTarget::Full,
        };
        assert_matcher!(m, [true, true, true, true, false, false]);

        let m = IdentifierTestSet {
            pattern: IdentifierPattern::Glob(Arc::new(Pattern::new("**/other").unwrap())),
            target: IdentifierTarget::Module,
        };
        assert_matcher!(m, [false, false, true, true, false, false]);

        let m = IdentifierTestSet {
            pattern: IdentifierPattern::Glob(Arc::new(Pattern::new("*-*").unwrap())),
            target: IdentifierTarget::Name,
        };
        assert_matcher!(m, [true, true, true, true, true, false]);
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
        let m = PrefixTestSet::complement(IgnoredTestSet);
        assert_matcher!(m, [true, true, true, true, true, false]);
    }

    #[test]
    fn test_binary() {
        let m = InfixTestSet::union(
            IdentifierTestSet {
                pattern: IdentifierPattern::Regex(Arc::new(Regex::new(r#"test-\d"#).unwrap())),
                target: IdentifierTarget::Full,
            },
            KindTestSet::compile_only(),
        );
        assert_matcher!(m, [true, true, true, true, true, false]);

        let m = InfixTestSet::intersection(
            IdentifierTestSet {
                pattern: IdentifierPattern::Regex(Arc::new(Regex::new(r#"test-\d"#).unwrap())),
                target: IdentifierTarget::Full,
            },
            KindTestSet::compile_only(),
        );
        assert_matcher!(m, [false, false, true, false, false, false]);

        let m = InfixTestSet::difference(
            IdentifierTestSet {
                pattern: IdentifierPattern::Regex(Arc::new(Regex::new(r#"test-\d"#).unwrap())),
                target: IdentifierTarget::Full,
            },
            KindTestSet::compile_only(),
        );
        assert_matcher!(m, [true, true, false, true, false, false]);

        let m = InfixTestSet::symmetric_difference(
            IdentifierTestSet {
                pattern: IdentifierPattern::Regex(Arc::new(Regex::new(r#"test-\d"#).unwrap())),
                target: IdentifierTarget::Full,
            },
            KindTestSet::compile_only(),
        );
        assert_matcher!(m, [true, true, false, true, true, false]);
    }
}
