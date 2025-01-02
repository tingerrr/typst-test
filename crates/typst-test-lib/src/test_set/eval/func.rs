use std::fmt::{self, Debug};
use std::sync::Arc;

use super::value::TryFromValue;
use super::{Context, Error, Set, Value};

/// The backing implementation for a [`Func`].
type FuncImpl = Arc<dyn Fn(&Context, &[Value]) -> Result<Value, Error>>;

/// A function value, can be called or passed around.
#[derive(Clone)]
pub struct Func(FuncImpl);

impl Func {
    /// Create a new function with the given implementation.
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(&Context, &[Value]) -> Result<Value, Error> + 'static,
    {
        Self(Arc::new(f) as _)
    }

    /// Call the given function with the given context and arguments.
    pub fn call(&self, ctx: &Context, args: &[Value]) -> Result<Value, Error> {
        (self.0)(ctx, args)
    }
}

impl Debug for Func {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Func").field(&..).finish()
    }
}

impl Func {
    /// Constructor for [`Set::built_in_all`].
    pub fn built_in_all(ctx: &Context, args: &[Value]) -> Result<Value, Error> {
        expect_no_args("all", ctx, args)?;
        Ok(Value::Set(Set::built_in_all()))
    }

    /// Constructor for [`Set::built_in_none`].
    pub fn built_in_none(ctx: &Context, args: &[Value]) -> Result<Value, Error> {
        expect_no_args("none", ctx, args)?;
        Ok(Value::Set(Set::built_in_none()))
    }

    /// Constructor for [`Set::built_in_skip`].
    pub fn built_in_skip(ctx: &Context, args: &[Value]) -> Result<Value, Error> {
        expect_no_args("skip", ctx, args)?;
        Ok(Value::Set(Set::built_in_skip()))
    }

    /// Constructor for [`Set::built_in_compile_only`].
    pub fn built_in_compile_only(ctx: &Context, args: &[Value]) -> Result<Value, Error> {
        expect_no_args("compile-only", ctx, args)?;
        Ok(Value::Set(Set::built_in_compile_only()))
    }

    /// Constructor for [`Set::built_in_ephemeral`].
    pub fn built_in_ephemeral(ctx: &Context, args: &[Value]) -> Result<Value, Error> {
        expect_no_args("ephemeral", ctx, args)?;
        Ok(Value::Set(Set::built_in_ephemeral()))
    }

    /// Constructor for [`Set::built_in_persistent`].
    pub fn built_in_persistent(ctx: &Context, args: &[Value]) -> Result<Value, Error> {
        expect_no_args("persistent", ctx, args)?;
        Ok(Value::Set(Set::built_in_persistent()))
    }
}

/// Ensure there are no args.
pub fn expect_no_args(id: &str, _ctx: &Context, args: &[Value]) -> Result<(), Error> {
    if args.is_empty() {
        Ok(())
    } else {
        Err(Error::InvalidArgumentCount {
            func: id.into(),
            expected: 0,
            is_min: false,
            found: args.len(),
        })
    }
}

// TODO(tinger): see test_set module todo

/// Extract an exact number of values from the given arguments. Validates the
/// types of all arguments.
#[allow(dead_code)]
pub fn expect_args_exact<T: TryFromValue + Debug, const N: usize>(
    func: &str,
    _ctx: &Context,
    args: &[Value],
) -> Result<[T; N], Error> {
    if args.len() < N {
        return Err(Error::InvalidArgumentCount {
            func: func.into(),
            expected: N,
            is_min: false,
            found: args.len(),
        });
    }

    Ok(args
        .iter()
        .take(N)
        .map(T::try_from_value)
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .expect("we checked both min and max of the args"))
}

/// Extract a variadic number of values with a minimum amount given arguments.
/// Validates the types of all arguments.
#[allow(dead_code)]
pub fn expect_args_min<T: TryFromValue + Debug, const N: usize>(
    func: &str,
    _ctx: &Context,
    args: &[Value],
) -> Result<([T; N], Vec<T>), Error> {
    if args.len() < N {
        return Err(Error::InvalidArgumentCount {
            func: func.into(),
            expected: N,
            is_min: true,
            found: args.len(),
        });
    }

    let min = args
        .iter()
        .take(N)
        .map(T::try_from_value)
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .expect("we checked both min and max of the args");

    Ok((
        min,
        args[N..]
            .iter()
            .map(T::try_from_value)
            .collect::<Result<_, _>>()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_set::parse::Num;

    const NUM: Num = Num(0);
    const VAL: Value = Value::Num(NUM);

    #[test]
    fn test_expect_args_variadic_min_length() {
        let ctx = Context::new();

        assert_eq!(
            expect_args_min::<Num, 0>("f", &ctx, &[]).unwrap(),
            ([], vec![]),
        );
        assert_eq!(expect_args_min("f", &ctx, &[VAL]).unwrap(), ([], vec![NUM]),);
        assert_eq!(
            expect_args_min("f", &ctx, &[VAL, VAL]).unwrap(),
            ([], vec![NUM, NUM]),
        );

        assert!(expect_args_min::<Num, 1>("f", &ctx, &[]).is_err());
        assert_eq!(expect_args_min("f", &ctx, &[VAL]).unwrap(), ([NUM], vec![]),);
        assert_eq!(
            expect_args_min("f", &ctx, &[VAL, VAL]).unwrap(),
            ([NUM], vec![NUM]),
        );

        assert!(expect_args_min::<Num, 2>("f", &ctx, &[]).is_err());
        assert!(expect_args_min::<Num, 2>("f", &ctx, &[VAL]).is_err(),);
        assert_eq!(
            expect_args_min("f", &ctx, &[VAL, VAL]).unwrap(),
            ([NUM, NUM], vec![]),
        );
    }
}
