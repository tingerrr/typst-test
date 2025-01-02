//! Test set evaluation and suite matching.

use std::collections::BTreeMap;
use std::fmt::{Debug, Display};

use ecow::{EcoString, EcoVec};
use thiserror::Error;

use self::value::{Type, Value};
use super::id::Id;
use super::parse::{Atom, Expr, Function, InfixOp, Pat, PrefixOp};
use crate::stdx::fmt::{Separators, Term};
use crate::test::Id as TestId;

pub mod func;
pub mod set;
pub mod value;

pub use self::func::Func;
pub use self::set::Set;

/// A trait for expressions to be evaluated and matched.
pub trait Eval {
    /// Evaluates this expression to a value.
    fn eval(&self, ctx: &Context) -> Result<Value, Error>;
}

impl Pat {
    /// Whether this pattern matches the given test identifier.
    pub fn matches(&self, id: &TestId) -> bool {
        match self {
            Self::Glob(pat) => pat.matches(id.as_str()),
            Self::Regex(regex) => regex.is_match(id.as_str()),
            Self::Contains(pat) => id.as_str().contains(pat.as_str()),
            Self::Exact(pat) => id.as_str() == pat.as_str(),
            Self::Path(path) => id.as_str().starts_with(path.as_str()),
        }
    }
}

impl Eval for Pat {
    fn eval(&self, _ctx: &Context) -> Result<Value, Error> {
        Ok(Value::Pat(self.clone()))
    }
}

impl Eval for Atom {
    fn eval(&self, ctx: &Context) -> Result<Value, Error> {
        Ok(match self {
            Self::Ident(id) => ctx.resolve(id)?,
            Self::Num(n) => Value::Num(*n),
            Self::Str(s) => Value::Str(s.clone()),
            Self::Pat(pat) => pat.eval(ctx)?,
        })
    }
}

impl Eval for Function {
    fn eval(&self, ctx: &Context) -> Result<Value, Error> {
        let func: Func = ctx.resolve(&self.id.0)?.expect_type()?;
        let args = self
            .args
            .iter()
            .map(|e| e.eval(ctx))
            .collect::<Result<Vec<_>, _>>()?;
        func.call(ctx, &args)
    }
}

// TODO: flatten intersection and union chains
impl Eval for Expr {
    fn eval(&self, ctx: &Context) -> Result<Value, Error> {
        match self {
            Self::Atom(atom) => atom.eval(ctx),
            Self::Func(func) => func.eval(ctx),
            Self::Prefix { op, expr } => {
                // unary prefix operator is only valid for test sets
                let set: Set = expr.eval(ctx)?.expect_type()?;

                Ok(Value::Set(match op {
                    PrefixOp::Not => Set::built_in_comp(set),
                }))
            }
            Self::Infix { op, lhs, rhs } => {
                // binary infix operator is only valid for test sets
                let lhs: Set = lhs.eval(ctx)?.expect_type()?;
                let rhs: Set = rhs.eval(ctx)?.expect_type()?;

                Ok(Value::Set(match op {
                    InfixOp::Union => Set::built_in_union(lhs, rhs, []),
                    InfixOp::Inter => Set::built_in_inter(lhs, rhs, []),
                    InfixOp::Diff => Set::built_in_diff(lhs, rhs),
                    InfixOp::SymDiff => Set::built_in_sym_diff(lhs, rhs),
                }))
            }
        }
    }
}

/// An evaluation context used to retrieve bindings in test set expressions.
#[derive(Debug, Clone)]
pub struct Context {
    /// The bindings available for evaluation.
    bindings: BTreeMap<Id, Value>,
}

impl Context {
    /// Create a new evaluation context with no bindings.
    pub fn new() -> Self {
        Self {
            bindings: BTreeMap::new(),
        }
    }

    /// Binds the built-in functions and values.
    pub fn bind_built_ins(&mut self) {
        for (id, f) in [
            (
                "all",
                Func::built_in_all as for<'a, 'b> fn(&'a Context, &'b [Value]) -> _,
            ),
            ("none", Func::built_in_none),
            ("skip", Func::built_in_skip),
            ("compile-only", Func::built_in_compile_only),
            ("ephemeral", Func::built_in_ephemeral),
            ("persistent", Func::built_in_persistent),
        ] {
            self.bindings
                .insert(Id::new(id).unwrap(), Value::Func(Func::new(f)));
        }
    }

    /// Resolves a binding with the given identifier.
    pub fn resolve(&self, id: &str) -> Result<Value, Error> {
        self.bindings
            .get(id)
            .cloned()
            .ok_or_else(|| Error::UnknownBinding { id: id.into() })
    }

    /// Find similar bindings to the given identifier.
    pub fn find_similar(&self, id: &str) -> Vec<Id> {
        self.bindings
            .keys()
            .filter(|cand| strsim::jaro(id, cand.as_str()) > 0.7)
            .cloned()
            .collect()
    }
}

impl Default for Context {
    fn default() -> Self {
        let mut this = Self::new();
        this.bind_built_ins();
        this
    }
}

/// An error that occurs when a test set expression is evaluated.
#[derive(Debug, Error)]
pub enum Error {
    /// The requested binding could not be found.
    UnknownBinding {
        /// The given identifier.
        id: EcoString,
    },

    /// A function received an incorrect argument count.
    InvalidArgumentCount {
        /// The identifier of the function.
        func: EcoString,

        /// The minimum or exact expected number of arguments, interpretation
        /// depends on [`is_min`].
        expected: usize,

        /// Whether the expected number is the minimum and allows more arguments.
        is_min: bool,

        /// The number of arguments passed.
        found: usize,
    },

    /// An invalid type was used in an expression.
    TypeMismatch {
        /// The expected types.
        expected: EcoVec<Type>,

        /// The given type.
        found: Type,
    },

    /// A regex pattern could not be parsed.
    Regex(#[from] regex::Error),

    /// A glob pattern could not be parsed.
    Glob(#[from] glob::PatternError),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::UnknownBinding { id } => write!(f, "unknown binding: {id}"),
            Error::InvalidArgumentCount {
                func,
                expected,
                is_min,
                found,
            } => {
                let (found, ex) = (*found, *expected);

                if ex == 0 {
                    write!(
                        f,
                        "function {func} expects no {}, got {}",
                        Term::simple("argument").with(ex),
                        found,
                    )?;
                } else if *is_min {
                    write!(
                        f,
                        "function {func} expects at least {ex} {}, got {}",
                        Term::simple("argument").with(ex),
                        found,
                    )?;
                } else {
                    write!(
                        f,
                        "function {func} expects exactly {ex} {}, got {}",
                        Term::simple("argument").with(ex),
                        found,
                    )?;
                }

                Ok(())
            }
            Error::TypeMismatch { expected, found } => write!(
                f,
                "expected {}, found <{}>",
                Separators::comma_or().with(expected.iter().map(|t| format!("<{}>", t.name()))),
                found.name(),
            ),
            Error::Regex(_) => write!(f, "could not parse regex"),
            Error::Glob(_) => write!(f, "could not parse glob"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_set::parse::Path;

    fn test_id(id: &str) -> TestId {
        TestId::new(id).unwrap()
    }

    #[test]
    fn test_path_pattern_matches() {
        let pat = Pat::Path(Path::new("a/b/c"));

        assert!(pat.matches(&test_id("a/b/c")));
        assert!(pat.matches(&test_id("a/b/c/d")));

        assert!(!pat.matches(&test_id("a")));
        assert!(!pat.matches(&test_id("a/b")));
        assert!(!pat.matches(&test_id("x/a/b/c")));
    }
}
