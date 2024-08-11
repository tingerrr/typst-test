//! Evaluate the ASTs into test set expressions.

use std::collections::BTreeMap;
use std::fmt::Debug;
use std::sync::Arc;

use ecow::{eco_vec, EcoString, EcoVec};
use thiserror::Error;
use typst_test_stdx::fmt::{Separators, Term};

use super::ast::{Literal, Pattern};
use super::builtin;
use super::id::Identifier;
use crate::store::test::Test;

/// A dynamic test set.
pub type DynTestSet = Arc<dyn TestSet>;

/// A dynamic function.
pub type DynFunction = Arc<dyn Function>;

/// A function which must be called on evaluation to produce a value.
pub trait Function: Debug + Send + Sync + 'static {
    /// Builds a test set from the given node.
    fn call(&self, ctx: &Context, args: &[Value]) -> Result<Value, Error>;
}

impl Function for Arc<dyn Function> {
    fn call(&self, ctx: &Context, args: &[Value]) -> Result<Value, Error> {
        Function::call(&**self, ctx, args)
    }
}

impl Function for Box<dyn Function> {
    fn call(&self, ctx: &Context, args: &[Value]) -> Result<Value, Error> {
        Function::call(&**self, ctx, args)
    }
}

/// A test set, a representation of multiple tests which can be used to match on
/// tests.
pub trait TestSet: Debug + Send + Sync + 'static {
    /// Returns whether this test is contained in this test set.
    fn contains(&self, test: &Test) -> bool;
}

impl TestSet for Arc<dyn TestSet> {
    fn contains(&self, test: &Test) -> bool {
        TestSet::contains(&**self, test)
    }
}

impl TestSet for Box<dyn TestSet> {
    fn contains(&self, test: &Test) -> bool {
        TestSet::contains(&**self, test)
    }
}

/// AST nodes which can be evaluated into a test set.
pub trait Eval {
    /// Evaluates this AST into a test set.
    fn eval(&self, ctx: &Context) -> Result<Value, Error>;
}

/// The type of an expression. This is primarily use for diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Type {
    /// A compound expression of some form which isn't fully resolved.
    Expression,

    /// A string.
    String,

    /// A number.
    Number,

    /// A pattern.
    Pattern,

    /// A function.
    Function,

    /// A test set.
    TestSet,
}

impl Type {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Expression => "expression",
            Self::String => "string",
            Self::Number => "number",
            Self::Pattern => "pattern",
            Self::Function => "function",
            Self::TestSet => "test set",
        }
    }
}

/// An evaluated expression.
#[derive(Debug, Clone)]
pub enum Value {
    /// A test set expression.
    TestSet(DynTestSet),

    /// A function expression.
    Function(DynFunction),

    /// A number expresison.
    Number(i64),

    /// A string expression.
    String(EcoString),

    /// A pattern expresison.
    Pattern(Pattern),
}

impl Value {
    /// Converts a [`Literal`] directly into a value.
    pub fn from_literal(literal: Literal) -> Self {
        match literal {
            Literal::Number(num) => Self::Number(num),
            Literal::String(str) => Self::String(str),
            Literal::Pattern(pat) => Self::Pattern(pat),
        }
    }

    /// Returns the type of this expression.
    pub fn as_type(&self) -> Type {
        match self {
            Value::TestSet(_) => Type::TestSet,
            Value::Function(_) => Type::Function,
            Value::Number(_) => Type::Number,
            Value::String(_) => Type::String,
            Value::Pattern(_) => Type::Pattern,
        }
    }

    /// Returns the inner [`TestSet`] or `None` if it's not a test set.
    pub fn as_test_set(&self) -> Option<&DynTestSet> {
        match self {
            Self::TestSet(set) => Some(set),
            _ => None,
        }
    }

    /// Returns the inner [`Function`] or `None` if it's not a function.
    pub fn as_function(&self) -> Option<&DynFunction> {
        match self {
            Value::Function(func) => Some(func),
            _ => None,
        }
    }

    /// Returns the inner string literal or `None` if it's not a string literal.
    pub fn as_string(&self) -> Option<&EcoString> {
        match self {
            Self::String(str) => Some(str),
            _ => None,
        }
    }

    /// Returns the inner number literal or `None` if it's not a number literal.
    pub fn as_number(&self) -> Option<i64> {
        match self {
            Self::Number(num) => Some(*num),
            _ => None,
        }
    }

    /// Returns the inner [`Pattern`] literal or `None` if it's not a pattern
    /// literal.
    pub fn as_pattern(&self) -> Option<&Pattern> {
        match self {
            Self::Pattern(pat) => Some(pat),
            _ => None,
        }
    }

    /// Same as [`Self::as_test_set`], but clones the value and returns a type
    /// error instead of `None`.
    pub fn to_test_set(&self) -> Result<DynTestSet, Error> {
        self.as_test_set()
            .ok_or_else(|| Error::TypeMismatch {
                expected: eco_vec![Type::TestSet],
                found: self.as_type(),
            })
            .cloned()
    }

    /// Same as [`Self::as_function`], but returns a type error instead of
    /// `None`.
    pub fn to_function(&self) -> Result<DynFunction, Error> {
        self.as_function()
            .ok_or_else(|| Error::TypeMismatch {
                expected: eco_vec![Type::Function],
                found: self.as_type(),
            })
            .cloned()
    }

    /// Same as [`Self::as_number`], but clones the value and returns a type
    /// error instead of `None`.
    pub fn to_number(&self) -> Result<i64, Error> {
        self.as_number().ok_or_else(|| Error::TypeMismatch {
            expected: eco_vec![Type::Number],
            found: self.as_type(),
        })
    }

    /// Same as [`Self::as_string`], but clones the value and returns a type
    /// error instead of `None`.
    pub fn to_string(&self) -> Result<EcoString, Error> {
        self.as_string()
            .ok_or_else(|| Error::TypeMismatch {
                expected: eco_vec![Type::String],
                found: self.as_type(),
            })
            .cloned()
    }

    /// Same as [`Self::as_pattern`], but clones the value and returns a type
    /// error instead of `None`.
    pub fn to_pattern(&self) -> Result<Pattern, Error> {
        self.as_pattern()
            .ok_or_else(|| Error::TypeMismatch {
                expected: eco_vec![Type::Pattern],
                found: self.as_type(),
            })
            .cloned()
    }
}

impl From<DynTestSet> for Value {
    fn from(value: DynTestSet) -> Self {
        Self::TestSet(value)
    }
}

impl From<DynFunction> for Value {
    fn from(value: DynFunction) -> Self {
        Self::Function(value)
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Self::Number(value)
    }
}

impl From<EcoString> for Value {
    fn from(value: EcoString) -> Self {
        Self::String(value)
    }
}

impl From<Pattern> for Value {
    fn from(value: Pattern) -> Self {
        Self::Pattern(value)
    }
}

impl From<Literal> for Value {
    fn from(value: Literal) -> Self {
        Self::from_literal(value)
    }
}

pub struct Context {
    /// The variables available in this context, these are fully resolved.
    bindings: BTreeMap<Identifier, Value>,
}

impl Context {
    /// Returns a context with the builtin variables defined.
    pub fn builtin() -> Self {
        Self {
            bindings: [
                ("all", Value::TestSet(builtin::all())),
                ("none", Value::TestSet(builtin::none())),
                ("ignored", Value::TestSet(builtin::ignored())),
                ("compile-only", Value::TestSet(builtin::compile_only())),
                ("ephemeral", Value::TestSet(builtin::ephemeral())),
                ("persistent", Value::TestSet(builtin::persistent())),
                ("default", Value::TestSet(builtin::default())),
                ("id", Value::Function(builtin::id())),
                ("mod", Value::Function(builtin::id())),
                ("name", Value::Function(builtin::id())),
                ("custom", Value::Function(builtin::custom())),
            ]
            .into_iter()
            .map(|(id, m)| (Identifier::new(id).unwrap(), m))
            .collect(),
        }
    }

    /// Try to resolve a binding.
    pub fn resolve_binding(&self, id: &str) -> Result<Value, Error> {
        self.bindings
            .get(id)
            .cloned()
            .ok_or_else(|| Error::UnknownBinding {
                id: id.into(),
                similar: self
                    .bindings
                    .keys()
                    .filter(|cand| strsim::jaro(id, cand.as_str()) > 0.7)
                    .cloned()
                    .collect(),
            })
    }
}

/// An error that occurs when a test set could not be constructed.
#[derive(Debug, Error)]
pub enum Error {
    /// The requested binding could not be found.
    #[error("unknown test set {id}")]
    UnknownBinding {
        id: EcoString,
        similar: Vec<Identifier>,
    },

    /// A function could not be evaluated.
    #[error("expected {expected} {}, found {found}", Term::simple("argument").with(*expected))]
    InvalidArgumentCount { expected: usize, found: usize },

    /// An invalid type was used in an expression.
    #[error(
        "expected {}, found <{}>",
        Separators::comma_or().with(expected.iter().map(|t| format!("<{}>", t.as_str()))),
        found.as_str(),
    )]
    TypeMismatch { expected: EcoVec<Type>, found: Type },

    /// A regex pattern could not be parsed.
    #[error("could not parse regex")]
    RegexError(#[from] regex::Error),

    /// A glob pattern could not be parsed.
    #[error("could not parse glob")]
    GlobError(#[from] glob::PatternError),
}

#[allow(dead_code)]
fn assert_traits() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    assert_send::<DynTestSet>();
    assert_sync::<DynFunction>();
}
