use std::collections::BTreeMap;
use std::fmt::{Debug, Display};
use std::str::FromStr;
use std::sync::Arc;

use ecow::EcoString;
use eval::{
    AllMatcher, BinaryMatcher, IdentiferMatcherTarget, IdentifierMatcher, IdentifierMatcherPattern,
    IgnoredMatcher, KindMatcher, NoneMatcher, UnaryMatcher,
};
use id::Identifier;
use once_cell::sync::Lazy;
use parsing::{
    Argument, Arguments, Atom, BinaryExpr, BinaryOp, Expr, Function, NameMatcher, Rule, UnaryExpr,
    UnaryOp, Value,
};
use pest::error::Error;
use regex::Regex;
use thiserror::Error;

use crate::store::test::Test;

pub mod eval;
pub mod id;
pub mod parsing;

/// A dynamic test set.
pub type DynTestSet = Arc<dyn TestSet + Send + Sync>;

/// A function which can construct a matcher for the given arguments.
pub type TestSetFactory =
    Box<dyn Fn(Arguments) -> Result<DynTestSet, BuildTestSetError> + Send + Sync>;

/// An error that occurs when a test set could not be constructed.
#[derive(Debug, Error)]
pub enum BuildTestSetError {
    /// The requested test set could not be found.
    UnknownTestSet { id: EcoString, func: bool },

    /// A regex matcher argument could not be parsed.
    RegexError(#[from] regex::Error),
}

impl Display for BuildTestSetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildTestSetError::UnknownTestSet { id, func } => {
                write!(f, "Unknown test set: {id}")?;
                if *func {
                    write!(f, "(...)")?;
                }
            }
            BuildTestSetError::RegexError(err) => write!(f, "{err}")?,
        }

        Ok(())
    }
}

/// A test set which matches tests, returning true for all tests contained in
/// this set.
pub trait TestSet: Debug + Send + Sync {
    /// Returns whether this test's identifier matches.
    fn is_match(&self, test: &Test) -> bool;
}

impl TestSet for Arc<dyn TestSet + Send + Sync> {
    fn is_match(&self, test: &Test) -> bool {
        TestSet::is_match(&**self, test)
    }
}

impl TestSet for Box<dyn TestSet + Send + Sync> {
    fn is_match(&self, test: &Test) -> bool {
        TestSet::is_match(&**self, test)
    }
}

impl<M: TestSet + Send + Sync> TestSet for &M {
    fn is_match(&self, test: &Test) -> bool {
        TestSet::is_match(*self, test)
    }
}

/// A full test set expression.
#[derive(Debug, Clone)]
pub struct TestSetExpr {
    root: Expr,
}

impl FromStr for TestSetExpr {
    type Err = Error<Rule>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parsing::parse_test_set_expr(s).map(|root| Self { root })
    }
}

impl TestSetExpr {
    /// Build the test set exression into a matcher.
    pub fn build(self, test_sets: &TestSets) -> Result<DynTestSet, BuildTestSetError> {
        build_matcher(self.root, test_sets)
    }
}

/// A map of test set values and functions used when building a test set expression into a [`TestSet`].
pub struct TestSets {
    values: BTreeMap<Identifier, DynTestSet>,
    funcs: BTreeMap<Identifier, TestSetFactory>,
}

impl TestSets {
    /// Try to get a test set value.
    pub fn get_value(&self, id: &str) -> Result<DynTestSet, BuildTestSetError> {
        self.values
            .get(id)
            .cloned()
            .ok_or_else(|| BuildTestSetError::UnknownTestSet {
                id: id.into(),
                func: false,
            })
    }

    /// Try to construct a test set function.
    pub fn get_func(&self, id: &str, args: Arguments) -> Result<DynTestSet, BuildTestSetError> {
        (self
            .funcs
            .get(id)
            .ok_or_else(|| BuildTestSetError::UnknownTestSet {
                id: id.into(),
                func: true,
            })?)(args)
    }
}

impl Default for TestSets {
    fn default() -> Self {
        use IdentiferMatcherTarget::{Full, Module, Name};

        Self {
            values: [
                ("all", builtin::all()),
                ("none", builtin::none()),
                ("ignored", builtin::ignored()),
                ("compile-only", builtin::compile_only()),
                ("ephemeral", builtin::ephemeral()),
                ("persistent", builtin::persistent()),
                ("default", builtin::default()),
            ]
            .into_iter()
            .map(|(id, m)| (Identifier { id: id.into() }, m))
            .collect(),
            funcs: [
                (
                    "id",
                    Box::new(|args| {
                        let Arguments {
                            arg: Argument { matcher },
                        } = args;
                        Ok(match matcher {
                            NameMatcher::Exact(name) => builtin::id_string(Full, name, true),
                            NameMatcher::Contains(name) => builtin::id_string(Full, name, false),
                            NameMatcher::Regex(name) => builtin::id_regex(Full, Regex::new(&name)?),
                        })
                    }) as TestSetFactory,
                ),
                (
                    "mod",
                    Box::new(|args| {
                        let Arguments {
                            arg: Argument { matcher },
                        } = args;

                        Ok(match matcher {
                            NameMatcher::Exact(name) => builtin::id_string(Module, name, true),
                            NameMatcher::Contains(name) => builtin::id_string(Module, name, false),
                            NameMatcher::Regex(name) => {
                                builtin::id_regex(Module, Regex::new(&name)?)
                            }
                        })
                    }),
                ),
                (
                    "name",
                    Box::new(|args| {
                        let Arguments {
                            arg: Argument { matcher },
                        } = args;

                        Ok(match matcher {
                            NameMatcher::Exact(name) => builtin::id_string(Name, name, true),
                            NameMatcher::Contains(name) => builtin::id_string(Name, name, false),
                            NameMatcher::Regex(name) => builtin::id_regex(Name, Regex::new(&name)?),
                        })
                    }),
                ),
            ]
            .into_iter()
            .map(|(id, m)| (Identifier { id: id.into() }, m))
            .collect(),
        }
    }
}

/// A map of builtin test sets, i.e. all that are contained in [`builtin`].
pub static BUILTIN_TESTSETS: Lazy<TestSets> = Lazy::new(TestSets::default);

/// Builtin test sets.
pub mod builtin {
    use super::*;

    /// Returns the `none` test set.
    pub fn none() -> DynTestSet {
        Arc::new(NoneMatcher)
    }

    /// Returns the `all` test set.
    pub fn all() -> DynTestSet {
        Arc::new(AllMatcher)
    }

    /// Returns the `ignored` test set.
    pub fn ignored() -> DynTestSet {
        Arc::new(IgnoredMatcher)
    }

    /// Returns the `compile-only` test set.
    pub fn compile_only() -> DynTestSet {
        Arc::new(KindMatcher::compile_only())
    }

    /// Returns the `ephemeral` test set.
    pub fn ephemeral() -> DynTestSet {
        Arc::new(KindMatcher::ephemeral())
    }

    /// Returns the `persistent` test set.
    pub fn persistent() -> DynTestSet {
        Arc::new(KindMatcher::persistent())
    }

    /// Returns the `default` test set.
    pub fn default() -> DynTestSet {
        eval::default()
    }

    /// Returns an id test set using a contains or exact matcher for the given
    /// target.
    pub fn id_string<S: Into<EcoString>>(
        target: IdentiferMatcherTarget,
        term: S,
        exact: bool,
    ) -> DynTestSet {
        Arc::new(IdentifierMatcher {
            pattern: if exact {
                IdentifierMatcherPattern::Exact(term.into())
            } else {
                IdentifierMatcherPattern::Contains(term.into())
            },
            target,
        })
    }

    /// Returns an id test set using a regex matcher for the given target.
    pub fn id_regex(target: IdentiferMatcherTarget, pattern: Regex) -> DynTestSet {
        Arc::new(IdentifierMatcher {
            pattern: IdentifierMatcherPattern::Regex(pattern),
            target,
        })
    }
}

/// Build a matcher from the given [`Expr`] using the given test sets.
pub fn build_matcher(expr: Expr, test_sets: &TestSets) -> Result<DynTestSet, BuildTestSetError> {
    Ok(match expr {
        Expr::Unary(UnaryExpr { op, expr }) => match op {
            UnaryOp::Complement => {
                Arc::new(UnaryMatcher::Complement(build_matcher(*expr, test_sets)?))
            }
        },
        Expr::Binary(BinaryExpr { op, lhs, rhs }) => match op {
            BinaryOp::SymmetricDifference => Arc::new(BinaryMatcher::SymmetricDifference(
                build_matcher(*lhs, test_sets)?,
                build_matcher(*rhs, test_sets)?,
            )),
            BinaryOp::Difference => Arc::new(BinaryMatcher::Difference(
                build_matcher(*lhs, test_sets)?,
                build_matcher(*rhs, test_sets)?,
            )),
            BinaryOp::Intersection => Arc::new(BinaryMatcher::Intersect(
                build_matcher(*lhs, test_sets)?,
                build_matcher(*rhs, test_sets)?,
            )),
            BinaryOp::Union => Arc::new(BinaryMatcher::Union(
                build_matcher(*lhs, test_sets)?,
                build_matcher(*rhs, test_sets)?,
            )),
        },
        Expr::Atom(Atom::Value(Value { id })) => test_sets.get_value(&id.value)?,
        Expr::Atom(Atom::Function(Function { id, args })) => test_sets.get_func(&id.value, args)?,
    })
}
