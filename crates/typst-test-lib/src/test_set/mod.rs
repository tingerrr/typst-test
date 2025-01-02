//! A functional set-based DSL for filtering tests.

// TODO(tinger): Link to book, expose API to create simple test sets
// programmatically.

use std::mem;
use std::str::FromStr;

use thiserror::Error;

use self::eval::{Eval, Set};
use crate::test::Test;

mod eval;
mod id;
mod parse;

pub use self::eval::Context;
pub use self::id::{Id, ParseIdError};

/// A parsed test set expression, this type can only be parsed from a string and
/// not created manually at the moment.
///
/// This also includes extra parsing logic for the special `all:` modifier
/// prefix, which is not part of the test set grammar.
///
/// This type is cheap to clone.
#[derive(Debug, Clone)]
pub struct TestSetExpr {
    all: bool,
    expr: parse::Expr,
}

impl TestSetExpr {
    /// Parse and evaluate a string into a test set expression.
    pub fn parse<S: AsRef<str>>(input: S) -> Result<Self, Error> {
        let input = input.as_ref().trim();

        let (all, input) = input
            .strip_prefix("all:")
            .map(|rest| (true, rest))
            .unwrap_or((false, input));

        let expr = parse::parse(input)?;

        Ok(Self { all, expr })
    }
}

impl TestSetExpr {
    /// Whether this test set had the special `all:` modifier prefix.
    pub fn all(&self) -> bool {
        self.all
    }
}

impl FromStr for TestSetExpr {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

/// A test set which can be used to easily filter tests.
///
/// This type is cheap to clone.
#[derive(Debug, Default, Clone)]
pub struct TestSet {
    all: bool,
    ctx: Context,
    set: Set,
}

impl TestSet {
    /// Creates a new test set which contains all tests.
    ///
    /// This will contain an empty [`Context`] and return `true` for
    /// [`TestSet::all`].
    pub fn all() -> Self {
        Self {
            all: true,
            ctx: Context::new(),
            set: Set::built_in_all(),
        }
    }

    /// Create a new test set from the given context and expression.
    pub fn evaluate(ctx: Context, expr: TestSetExpr) -> Result<Self, Error> {
        let TestSetExpr { all, expr } = expr;
        let set = expr.eval(&ctx).and_then(|value| value.expect_type())?;

        Ok(Self { all, ctx, set })
    }

    /// Parse and evaluate a string into a directly test set.
    pub fn parse_and_evaluate<S: AsRef<str>>(ctx: Context, input: S) -> Result<Self, Error> {
        Self::evaluate(ctx, TestSetExpr::parse(input)?)
    }

    /// Adds an implicit `(...) ~ skip()` around the expression, this is used to
    /// easily filter out skipped test for default test sets.
    pub fn add_implicit_skip(&mut self) {
        self.set = Set::built_in_diff(mem::take(&mut self.set), Set::built_in_skip());
    }
}

impl TestSet {
    /// Whether this test set has the special `all:` modifier. Handling this is
    /// up to the caller and has no impact on the inner test set.
    ///
    /// This is used to allow the user to easily assert that they intend to
    /// operator on many tests at once.
    pub fn has_all_modifier(&self) -> bool {
        self.all
    }

    /// The context used to evaluate the inner set.
    pub fn ctx(&self) -> &Context {
        &self.ctx
    }

    /// Whether the given test is contained in this test set.
    pub fn contains(&self, test: &Test) -> Result<bool, Error> {
        Ok(self.set.contains(&self.ctx, test)?)
    }
}

/// The inner implementation for [`Error`].
#[derive(Debug, Error)]
enum ErrorImpl {
    /// A parse error occurred.
    #[error(transparent)]
    Parse(parse::Error),

    /// An eval error occurred.
    #[error(transparent)]
    Eval(eval::Error),
}

/// Returned by [`TestSet::evaluate`] and [`TestSet::parse_and_evaluate`].
#[derive(Debug, Error)]
#[error(transparent)]
pub struct Error(ErrorImpl);

impl From<parse::Error> for Error {
    fn from(value: parse::Error) -> Self {
        Self(ErrorImpl::Parse(value))
    }
}

impl From<eval::Error> for Error {
    fn from(value: eval::Error) -> Self {
        Self(ErrorImpl::Eval(value))
    }
}
