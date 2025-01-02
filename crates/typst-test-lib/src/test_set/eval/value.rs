use ecow::eco_vec;

use super::{Error, Func, Set};
use crate::test_set::parse::{Num, Pat, Str};

/// The value a test set expression can evaluate to.
#[derive(Debug, Clone)]
pub enum Value {
    /// A test set.
    Set(Set),

    /// A function.
    Func(Func),

    /// An unsigned integer.
    Num(Num),

    /// A string.
    Str(Str),

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

    /// Convert this value into a [`T`] or return an error.
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

impl From<Num> for Value {
    fn from(value: Num) -> Self {
        Self::Num(value)
    }
}

impl From<Str> for Value {
    fn from(value: Str) -> Self {
        Self::Str(value)
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

macro_rules! impl_try_from_value {
    ($id:ident) => {
        impl TryFromValue for $id {
            fn try_from_value(value: &Value) -> Result<Self, Error> {
                Ok(match value {
                    Value::$id(set) => set.clone(),
                    _ => {
                        return Err(Error::TypeMismatch {
                            expected: eco_vec![Type::$id],
                            found: value.as_type(),
                        })
                    }
                })
            }
        }
    };
}

impl TryFromValue for Set {
    fn try_from_value(value: &Value) -> Result<Self, Error> {
        Ok(match value {
            Value::Set(set) => set.clone(),
            Value::Pat(pat) => Set::built_in_pattern(pat.clone()),
            _ => {
                return Err(Error::TypeMismatch {
                    expected: eco_vec![Type::Set, Type::Pat],
                    found: value.as_type(),
                })
            }
        })
    }
}

impl_try_from_value!(Func);
impl_try_from_value!(Num);
impl_try_from_value!(Str);
impl_try_from_value!(Pat);

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
