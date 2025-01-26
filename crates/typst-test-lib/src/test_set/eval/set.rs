use std::fmt::Debug;
use std::sync::Arc;

use ecow::eco_vec;

use super::{Context, Error, TryFromValue, Type, Value};
use crate::test::Test;
use crate::test_set::Pat;

/// The backing implementation for a [`Set`].
type SetImpl = Arc<dyn Fn(&Context, &Test) -> Result<bool, Error> + 'static>;

/// A set value, can be used to check if a test is contained in it.
///
/// The defaut value is the `none` set, that which contains no tests.
#[derive(Clone)]
pub struct Set(SetImpl);

impl Set {
    /// Create a new set with the given implementation.
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(&Context, &Test) -> Result<bool, Error> + 'static,
    {
        Self(Arc::new(f) as _)
    }

    /// Whether the given test is contained within this set.
    pub fn contains(&self, ctx: &Context, test: &Test) -> Result<bool, Error> {
        (self.0)(ctx, test)
    }
}

impl Debug for Set {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Set").field(&..).finish()
    }
}

impl Default for Set {
    fn default() -> Self {
        Self::built_in_none()
    }
}

impl Set {
    /// Construct a set which contains _all_ tests.
    pub fn built_in_all() -> Self {
        Self::new(|_, _| Ok(true))
    }

    /// Construct a set which contains _no_ tests.
    pub fn built_in_none() -> Self {
        Self::new(|_, _| Ok(false))
    }

    /// Construct a set which contains all tests marked to be skip.
    pub fn built_in_skip() -> Self {
        Self::new(|_, test| Ok(test.is_skip()))
    }

    /// Construct a set which contains all compile-only tests.
    pub fn built_in_compile_only() -> Self {
        Self::new(|_, test| Ok(test.kind().is_compile_only()))
    }

    /// Construct a set which contains all ephemeral tests.
    pub fn built_in_ephemeral() -> Self {
        Self::new(|_, test| Ok(test.kind().is_ephemeral()))
    }

    /// Construct a set which contains all persistent tests.
    pub fn built_in_persistent() -> Self {
        Self::new(|_, test| Ok(test.kind().is_persistent()))
    }

    /// Construct a set which contains all tests matching the given pattern.
    ///
    /// This is the test set created by pattern literals like `r:'foot-(\w-)+'`.
    pub fn built_in_pattern(pat: Pat) -> Self {
        Self::new(move |_, test| Ok(pat.is_match(test.id())))
    }

    /// Construct a set which contains all tests _not_ contained in the given
    /// set.
    ///
    /// This is the test set created by `!set`.
    pub fn built_in_comp(set: Set) -> Self {
        Self::new(move |ctx, test| Ok(!set.contains(ctx, test)?))
    }

    /// Construct a set which contains all tests which are contained in any of
    /// the given sets.
    ///
    /// This is the test set created by `a | b`.
    pub fn built_in_union<I>(a: Set, b: Set, rest: I) -> Self
    where
        I: IntoIterator<Item = Set>,
    {
        let sets: Vec<_> = [a, b].into_iter().chain(rest).collect();

        Self::new(move |ctx, test| {
            for set in &sets {
                if set.contains(ctx, test)? {
                    return Ok(true);
                }
            }

            Ok(false)
        })
    }

    /// Construct a set which contains all tests which are contained in all of
    /// the given sets.
    ///
    /// This is the test set created by `a & b`.
    pub fn built_in_inter<I>(a: Set, b: Set, rest: I) -> Self
    where
        I: IntoIterator<Item = Set>,
    {
        let sets: Vec<_> = [a, b].into_iter().chain(rest).collect();

        Self::new(move |ctx, test| {
            for set in &sets {
                if !set.contains(ctx, test)? {
                    return Ok(false);
                }
            }

            Ok(true)
        })
    }

    /// Construct a set which contains all tests which are contained in the
    /// first but not the second set.
    ///
    /// This is the test set created by `a ~ b` and is equivalent to `a & !b`.
    pub fn built_in_diff(a: Set, b: Set) -> Self {
        Self::new(move |ctx, test| Ok(a.contains(ctx, test)? && !b.contains(ctx, test)?))
    }

    /// Construct a set which contains all tests which are contained in the
    /// either the first or the second, but not both sets.
    ///
    /// This is the test set created by `a ^ b`.
    pub fn built_in_sym_diff(a: Set, b: Set) -> Self {
        Self::new(move |ctx, test| Ok(a.contains(ctx, test)? ^ b.contains(ctx, test)?))
    }
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
