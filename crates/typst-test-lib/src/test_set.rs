//! Matching and filtering tests using a set expression DSL.

use std::collections::BTreeMap;
use std::fmt::{Debug, Display};
use std::str::FromStr;
use std::sync::Arc;

use ecow::EcoString;
use id::Identifier;
use once_cell::sync::Lazy;
use parsing::{Expr, Rule};
use pest::error::Error;
use regex::Regex;
use thiserror::Error;

use crate::store::test::Test;

mod eval;
pub mod id;
mod parsing;

// TODO: these are a leaky abstraction and need to be removed.
pub use parsing::{Argument, Arguments, NameMatcher};

/// A dynamic test set.
pub type DynTestSet = Arc<dyn TestSet + Send + Sync>;

// TODO: the arguments are a leaky abstraction
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

    /// The arguments passed to the test set were invalid.
    InvalidArguments { id: EcoString },
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
            BuildTestSetError::InvalidArguments { id } => {
                write!(f, "Invalid arguments for test set {id}(...)")?;
            }
        }

        Ok(())
    }
}

/// A test set, a representation of multiple tests which can be used to build
/// test set expressions.
pub trait TestSet: Debug + Send + Sync {
    /// Returns whether this test is contained in this test set.
    fn contains(&self, test: &Test) -> bool;
}

impl TestSet for Arc<dyn TestSet + Send + Sync> {
    fn contains(&self, test: &Test) -> bool {
        TestSet::contains(&**self, test)
    }
}

impl TestSet for Box<dyn TestSet + Send + Sync> {
    fn contains(&self, test: &Test) -> bool {
        TestSet::contains(&**self, test)
    }
}

impl<M: TestSet + Send + Sync> TestSet for &M {
    fn contains(&self, test: &Test) -> bool {
        TestSet::contains(*self, test)
    }
}

/// A parsed test set expression which can be built into a [`DynTestSet`].
#[derive(Debug, Clone)]
pub struct TestSetExpr {
    root: Expr,
}

impl FromStr for TestSetExpr {
    type Err = Box<Error<Rule>>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parsing::parse_test_set_expr(s).map(|root| Self { root })
    }
}

impl TestSetExpr {
    /// Build the test set expression into a [`DynTestSet`].
    pub fn build(self, test_sets: &TestSets) -> Result<DynTestSet, BuildTestSetError> {
        expr::build_test_set(self.root, test_sets)
    }
}

/// A map of test set values and functions used when building a test set expression into a [`TestSet`].
pub struct TestSets {
    values: BTreeMap<Identifier, DynTestSet>,
    funcs: BTreeMap<Identifier, TestSetFactory>,
}

impl TestSets {
    /// Creates a set containing the built-in test sets.
    pub fn builtin() -> Self {
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
            .map(|(id, m)| (Identifier::new(id).unwrap(), m))
            .collect(),
            funcs: [
                (
                    "custom",
                    Box::new(|args| {
                        let Arguments {
                            arg: Argument { matcher },
                        } = args;
                        Ok(match matcher {
                            NameMatcher::Plain(id) => builtin::custom(id.into()),
                            _ => {
                                return Err(BuildTestSetError::InvalidArguments {
                                    id: "custom".into(),
                                })
                            }
                        })
                    }) as TestSetFactory,
                ),
                (
                    "id",
                    Box::new(|args| {
                        let Arguments {
                            arg: Argument { matcher },
                        } = args;
                        Ok(match matcher {
                            NameMatcher::Exact(name) => builtin::id_string(name, true),
                            NameMatcher::Contains(name) => builtin::id_string(name, false),
                            NameMatcher::Regex(name) => builtin::id_regex(Regex::new(&name)?),
                            NameMatcher::Plain(_) => {
                                return Err(BuildTestSetError::InvalidArguments { id: "id".into() })
                            }
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
                            NameMatcher::Exact(name) => builtin::mod_string(name, true),
                            NameMatcher::Contains(name) => builtin::mod_string(name, false),
                            NameMatcher::Regex(name) => builtin::mod_regex(Regex::new(&name)?),
                            NameMatcher::Plain(_) => {
                                return Err(BuildTestSetError::InvalidArguments {
                                    id: "mod".into(),
                                })
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
                            NameMatcher::Exact(name) => builtin::name_string(name, true),
                            NameMatcher::Contains(name) => builtin::name_string(name, false),
                            NameMatcher::Regex(name) => builtin::name_regex(Regex::new(&name)?),
                            NameMatcher::Plain(_) => {
                                return Err(BuildTestSetError::InvalidArguments {
                                    id: "name".into(),
                                })
                            }
                        })
                    }),
                ),
            ]
            .into_iter()
            .map(|(id, m)| (Identifier::new(id).unwrap(), m))
            .collect(),
        }
    }

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
        Self::builtin()
    }
}

/// A map of builtin test sets, i.e. all that are contained in [`builtin`].
pub static BUILTIN_TESTSETS: Lazy<TestSets> = Lazy::new(TestSets::default);

/// Builtin test set constructors.
pub mod builtin {
    use eval::{
        AllTestSet, CustomTestSet, FnTestSet, IdentifierPattern, IdentifierTarget,
        IdentifierTestSet, IgnoredTestSet, KindTestSet, NoneTestSet,
    };

    use super::*;

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

    /// Returns the `default` test set.
    pub fn default() -> DynTestSet {
        eval::default()
    }

    /// Returns a string based identifier matcher test set which targets the
    /// full identifier.
    pub fn id_string<S: Into<EcoString>>(term: S, exact: bool) -> DynTestSet {
        id_string_impl(IdentifierTarget::Full, term.into(), exact)
    }

    /// Returns a string based identifier matcher test set which targets the
    /// test name.
    pub fn name_string<S: Into<EcoString>>(term: S, exact: bool) -> DynTestSet {
        id_string_impl(IdentifierTarget::Name, term.into(), exact)
    }

    /// Returns a string based identifier matcher test set which targets the
    /// module.
    pub fn mod_string<S: Into<EcoString>>(term: S, exact: bool) -> DynTestSet {
        id_string_impl(IdentifierTarget::Module, term.into(), exact)
    }

    /// Returns an id test set using a contains or exact matcher for the given
    /// target.
    fn id_string_impl<S: Into<EcoString>>(
        target: IdentifierTarget,
        term: S,
        exact: bool,
    ) -> DynTestSet {
        Arc::new(IdentifierTestSet {
            pattern: if exact {
                IdentifierPattern::Exact(term.into())
            } else {
                IdentifierPattern::Contains(term.into())
            },
            target,
        })
    }

    /// Returns a regex based identifier matcher test set which targets the full
    /// identifier.
    pub fn id_regex(term: Regex) -> DynTestSet {
        id_regex_impl(IdentifierTarget::Full, term)
    }

    /// Returns a regex based identifier matcher test set which targets the test
    /// name.
    pub fn name_regex(term: Regex) -> DynTestSet {
        id_regex_impl(IdentifierTarget::Name, term)
    }

    /// Returns a regex based identifier matcher test set which targets the
    /// module.
    pub fn mod_regex(term: Regex) -> DynTestSet {
        id_regex_impl(IdentifierTarget::Module, term)
    }

    /// Returns an id test set using a regex matcher for the given target.
    fn id_regex_impl(target: IdentifierTarget, pattern: Regex) -> DynTestSet {
        Arc::new(IdentifierTestSet {
            pattern: IdentifierPattern::Regex(pattern),
            target,
        })
    }

    /// Returns a custom matcher which matches all tests with a custom
    /// [`Annotation`][crate::test::Annotation].
    pub fn custom(id: EcoString) -> DynTestSet {
        Arc::new(CustomTestSet { id })
    }

    /// Creates a new test set which is defined by the given matcher function.
    pub fn from_fn<F: Fn(&Test) -> bool + Send + Sync + 'static>(f: F) -> DynTestSet {
        Arc::new(FnTestSet::new(f))
    }
}

/// Test set expression builders.
pub mod expr {
    use eval::{BinaryTestSet, UnaryTestSet};
    use parsing::{Atom, BinaryExpr, BinaryOp, Expr, Function, UnaryExpr, UnaryOp, Value};

    use super::*;

    /// Creates the omplement `!a`.
    pub fn complement(a: DynTestSet) -> DynTestSet {
        Arc::new(UnaryTestSet::Complement(a))
    }

    /// Creates the union of `a | b`.
    pub fn union(a: DynTestSet, b: DynTestSet) -> DynTestSet {
        Arc::new(BinaryTestSet::Union(a, b))
    }

    /// Creates the intersection of `a & b`.
    pub fn intersection(a: DynTestSet, b: DynTestSet) -> DynTestSet {
        Arc::new(BinaryTestSet::Intersect(a, b))
    }

    /// Creates the difference of `a - b`.
    pub fn difference(a: DynTestSet, b: DynTestSet) -> DynTestSet {
        Arc::new(BinaryTestSet::Difference(a, b))
    }

    /// Creates the symmetric difference of `a ^ b`.
    pub fn symmetric_difference(a: DynTestSet, b: DynTestSet) -> DynTestSet {
        Arc::new(BinaryTestSet::SymmetricDifference(a, b))
    }

    /// Build a matcher from the given [`Expr`] using the given test sets.
    pub(super) fn build_test_set(
        expr: Expr,
        test_sets: &TestSets,
    ) -> Result<DynTestSet, BuildTestSetError> {
        Ok(match expr {
            Expr::Unary(UnaryExpr { op, expr }) => match op {
                UnaryOp::Complement => complement(build_test_set(*expr, test_sets)?),
            },
            Expr::Binary(BinaryExpr { op, lhs, rhs }) => match op {
                BinaryOp::SymmetricDifference => symmetric_difference(
                    build_test_set(*lhs, test_sets)?,
                    build_test_set(*rhs, test_sets)?,
                ),
                BinaryOp::Difference => difference(
                    build_test_set(*lhs, test_sets)?,
                    build_test_set(*rhs, test_sets)?,
                ),
                BinaryOp::Intersection => intersection(
                    build_test_set(*lhs, test_sets)?,
                    build_test_set(*rhs, test_sets)?,
                ),
                BinaryOp::Union => union(
                    build_test_set(*lhs, test_sets)?,
                    build_test_set(*rhs, test_sets)?,
                ),
            },
            Expr::Atom(Atom::Value(Value { id })) => test_sets.get_value(&id.value)?,
            Expr::Atom(Atom::Function(Function { id, args })) => {
                test_sets.get_func(&id.value, args)?
            }
        })
    }
}
