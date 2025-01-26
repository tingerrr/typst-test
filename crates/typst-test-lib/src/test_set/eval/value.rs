//! Expression evaluation result values.

use ecow::EcoString;

use super::{Error, Func, Set};
use crate::test_set::Pat;

/// The value of a test set expression.
#[derive(Debug, Clone)]
pub enum Value {
    /// A test set.
    Set(Set),

    /// A function.
    Func(Func),

    /// An unsigned integer.
    Num(usize),

    /// A string.
    Str(String),

    /// A pattern.
    Pat(Pat),
}

impl Value {
    /// The type of this expression.
    pub fn as_type(&self) -> Type {
        match self {
            Value::Set(_) => Type::Set,
            Value::Func(_) => Type::Func,
            Value::Num(_) => Type::Num,
            Value::Str(_) => Type::Str,
            Value::Pat(_) => Type::Pat,
        }
    }

    /// Convert this value into a `T` or return an error.
    pub fn expect_type<T: TryFromValue>(&self) -> Result<T, Error> {
        T::try_from_value(self)
    }
}

impl From<Set> for Value {
    fn from(value: Set) -> Self {
        Self::Set(value)
    }
}

impl From<Func> for Value {
    fn from(value: Func) -> Self {
        Self::Func(value)
    }
}

impl From<usize> for Value {
    fn from(value: usize) -> Self {
        Self::Num(value)
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Self::Str(value)
    }
}

impl From<EcoString> for Value {
    fn from(value: EcoString) -> Self {
        Self::Str(value.into())
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Self::Str(value.into())
    }
}

impl From<Pat> for Value {
    fn from(value: Pat) -> Self {
        Self::Pat(value)
    }
}

/// A trait for types which can be unwrapped from a [`Value`].
pub trait TryFromValue: Sized {
    fn try_from_value(value: &Value) -> Result<Self, Error>;
}

/// The type of an expression. This is primarily used for diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Type {
    /// A test set.
    Set,

    /// A function.
    Func,

    /// An unsigned integer.
    Num,

    /// A string.
    Str,

    /// A pattern.
    Pat,
}

impl Type {
    /// The name of this type.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Set => "test set",
            Self::Func => "function",
            Self::Num => "number",
            Self::Str => "string",
            Self::Pat => "pattern",
        }
    }
}
