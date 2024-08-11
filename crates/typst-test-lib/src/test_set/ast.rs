//! Compound test set expressions for evaluation using recursive AST-walking.

use core::str;
use std::borrow::Borrow;
use std::fmt::{Debug, Display};
use std::num::ParseIntError;
use std::ops::Deref;
use std::sync::Arc;

use ecow::{eco_format, EcoString, EcoVec};
use once_cell::sync::Lazy;
pub use op::{Infix as InfixOp, Postfix as PostfixOp, Prefix as PrefixOp};
pub use pattern::{Kind as PatternKind, Pattern};
use pest::error::Error as PestError;
use pest::iterators::{Pair, Pairs};
use pest::pratt_parser::PrattParser;
use pest_derive::Parser;
use thiserror::Error;

use super::builtin;
use super::eval::{Context, Error as EvalError, Eval, Value};
use super::id::Identifier;

/// A [`pest`] parser for test set expressions.
#[derive(Parser)]
#[grammar = "test_set/grammar.pest"]
pub struct TestSetParser;

impl Rule {
    /// Returns the [`PrefixOp`] which this rule corresponds to or `None` if it
    /// isn't an operator.
    pub fn as_prefix_op(&self) -> Option<PrefixOp> {
        match self {
            Rule::kw_not | Rule::op_complement | Rule::op_exclamation => Some(PrefixOp::Complement),
            _ => None,
        }
    }

    /// Returns the [`InfixOp`] which this rule corresponds to or `None` if it
    /// isn't an operator.
    pub fn as_infix_op(&self) -> Option<InfixOp> {
        match self {
            Rule::kw_or | Rule::op_union | Rule::op_pipe => Some(InfixOp::Union),
            Rule::kw_and | Rule::op_intersection | Rule::op_ampersand => {
                Some(InfixOp::Intersection)
            }
            Rule::op_backslash | Rule::op_tilde | Rule::kw_diff => Some(InfixOp::Difference),
            Rule::kw_xor | Rule::op_caret | Rule::op_delta => Some(InfixOp::SymmetricDifference),
            _ => None,
        }
    }

    /// Returns the [`PostfixOp`] which this rule corresponds to or `None` if it
    /// isn't an operator.
    pub fn as_postfix_op(&self) -> Option<PostfixOp> {
        match self {
            Rule::op_minus => Some(PostfixOp::Ancestors),
            Rule::op_plus => Some(PostfixOp::Descendants),
            _ => None,
        }
    }
}

macro_rules! debug_assert_no_pairs {
    ($pairs:expr) => {
        // NOTE: this allows us to also inspect the pairs easily
        debug_assert_eq!($pairs.collect::<Vec<_>>(), vec![]);
    };
}

/// Parses a given input string into a test set expression.
pub fn parse(input: &str) -> Result<Expr, Error> {
    use pest::Parser;

    let mut pairs = TestSetParser::parse(Rule::main, input).map_err(Box::new)?;

    let main = pairs.next().expect("main is not optional");
    #[cfg(debug_assertions)]
    {
        let eoi = pairs.next().expect("eoi is always given");
        assert_eq!(eoi.as_rule(), Rule::EOI);
    }
    debug_assert_no_pairs!(pairs);

    parse_expr(main.into_inner())
}

macro_rules! assert_rule {
    ($pair:expr => $rule:ident) => {
        if $pair.as_rule() != Rule::$rule {
            panic!(
                concat!("expected ", stringify!($rule), ", got {:?}: {:?}"),
                $pair.as_rule(),
                $pair,
            );
        }
    };
}

/// Parses an [`Atom`].
///
/// # Errors
/// Returns the inner atom fails parsing.
///
/// # Panics
/// Panics if the given pair is not of the expected rule, or if the inner pairs
/// don't match the expected count/rules.
pub fn parse_atom(pair: Pair<Rule>) -> Result<Atom, Error> {
    assert_rule!(pair => atom);

    let mut pairs = pair.into_inner();
    let atom = pairs.next().expect("atom is not empty");
    debug_assert_no_pairs!(pairs);

    Ok(match atom.as_rule() {
        Rule::variable => Atom::Variable(parse_variable(atom)?),
        Rule::function => Atom::Function(parse_function(atom)?),
        Rule::literal => Atom::Literal(parse_literal(atom)?),
        _ => unreachable!("atom can only be val, func or lit"),
    })
}

/// Parses a [`Variable`].
///
/// # Errors
/// Returns an error if the identifier fails parsing.
///
/// # Panics
/// Panics if the given pair is not of the expected rule, or if the inner pairs
/// don't match the expected count/rules.
pub fn parse_variable(pair: Pair<Rule>) -> Result<Variable, Error> {
    assert_rule!(pair => variable);

    let mut pairs = pair.into_inner();
    let id = pairs.next().expect("variable has id");
    debug_assert_no_pairs!(pairs);

    Ok(Variable {
        id: parse_identifier(id)?,
    })
}

/// Parses a [`Function`].
///
/// # Errors
/// Returns an error if the identifier or any of the arguments fail parsing.
///
/// # Panics
/// Panics if the given pair is not of the expected rule, or if the inner pairs
/// don't match the expected count/rules.
pub fn parse_function(pair: Pair<Rule>) -> Result<Function, Error> {
    assert_rule!(pair => function);

    let mut pairs = pair.into_inner();
    let id = pairs.next().expect("func has id");
    let args = pairs.next().expect("func has args");
    debug_assert_no_pairs!(pairs);

    Ok(Function {
        id: parse_identifier(id)?,
        args: parse_function_arguments(args)?,
    })
}

/// Parses an identifier for a variable or function.
///
/// # Errors
/// Return no error, the return type is for compatibility.
///
/// # Panics
/// Panics if the given pair is not of the expected rule, or if the inner pairs
/// don't match the expected count/rules.
pub fn parse_identifier(pair: Pair<Rule>) -> Result<Identifier, Error> {
    assert_rule!(pair => identifier);

    let id = pair.as_str().to_owned();
    debug_assert_no_pairs!(pair.into_inner());

    Ok(Identifier::new(id).expect("parser ensures validity"))
}

/// Parses a [`Variable`].
///
/// # Errors
/// Returns an error if parsing the inner literal fails.
///
/// # Panics
/// Panics if the given pair is not of the expected rule, or if the inner pairs
/// don't match the expected count/rules.
pub fn parse_literal(pair: Pair<Rule>) -> Result<Literal, Error> {
    assert_rule!(pair => literal);

    let mut pairs = pair.into_inner();
    let literal = pairs.next().expect("literal has inner");
    debug_assert_no_pairs!(pairs);

    Ok(match literal.as_rule() {
        Rule::number => Literal::Number(parse_number(literal)?),
        Rule::string => Literal::String(parse_string(literal)?),
        Rule::pattern => Literal::Pattern(parse_pattern(literal)?),
        _ => unreachable!(),
    })
}

/// Parses a [`Variable`].
///
/// # Errors
/// Returns an error if the integer literal over- or underflows.
///
/// # Panics
/// Panics if the given pair is not of the expected rule, or if the inner pairs
/// don't match the expected count/rules.
pub fn parse_number(pair: Pair<Rule>) -> Result<i64, Error> {
    assert_rule!(pair => number);

    let value = match pair.as_str().parse() {
        Ok(v) => v,
        Err(err) => return Err(Error::IntegerError(err)),
    };
    debug_assert_no_pairs!(pair.into_inner());

    Ok(value)
}

/// Parses a string literal.
///
/// # Errors
/// Returns an error if the string contained an invalid escape sequence.
///
/// # Panics
/// Panics if the given pair is not of the expected rule, or if the inner pairs
/// don't match the expected count/rules.
pub fn parse_string(pair: Pair<Rule>) -> Result<EcoString, Error> {
    assert_rule!(pair => string);

    let mut pairs = pair.into_inner();
    let inner = pairs.next().expect("string has inner");
    debug_assert_no_pairs!(pairs);

    let inner = inner.as_str();
    if !inner.contains('\\') {
        return Ok(inner.into());
    }

    let mut rest = inner;
    let mut buffer = EcoString::with_capacity(inner.len());

    while let Some((ok, r)) = rest.split_once('\\') {
        let (esc, r) = match r.as_bytes() {
            [b'\\', ..] => ('\\', &r[1..]),
            [b't', ..] => ('\t', &r[1..]),
            [b'r', ..] => ('\r', &r[1..]),
            [b'n', ..] => ('\n', &r[1..]),
            [b'u', b'{', a, b, c, d, b'}', ..] => {
                let buf = [*a, *b, *c, *d];
                // SAFETY: we got these bytes directly from a valid UTF-8 boundary after the byte sequence `\u{`
                let val = unsafe { str::from_utf8_unchecked(&buf) };
                let val = u32::from_str_radix(val, 16).expect("u32 can fit four digit hex number");
                (
                    char::from_u32(val).ok_or_else(|| Error::InvalidEscape(eco_format!("")))?,
                    &r[7..],
                )
            }
            _ => unreachable!("EOI or unknown escape at {r:?}"),
        };
        buffer.push_str(ok);
        buffer.push(esc);
        rest = r;
    }

    Ok(buffer)
}

/// Parses function arguments, i.e. a comma separated list of [`Expr`]s
/// surrounded by parentheses.
///
/// # Errors
/// Returns the first error if parsing any of the inner expressions fails.
///
/// # Panics
/// Panics if the given pair is not of the expected rule, or if the inner pairs
/// don't match the expected count/rules.
pub fn parse_function_arguments(pair: Pair<Rule>) -> Result<EcoVec<Expr>, Error> {
    assert_rule!(pair => function_arguments);

    pair.into_inner()
        .map(Pair::into_inner)
        .map(parse_expr)
        .collect()
}

/// Parses a [`Pattern`] literal.
///
/// # Errors
/// Returns an error if parsing the string literal fails.
///
/// # Panics
/// Panics if the given pair is not of the expected rule, or if the inner pairs
/// don't match the expected count/rules.
pub fn parse_pattern(pair: Pair<Rule>) -> Result<Pattern, Error> {
    assert_rule!(pair => pattern);

    let mut pairs = pair.into_inner();
    let prefix = pairs.next().expect("pattern has prefix");
    let inner = pairs.next().expect("pattern has raw");
    debug_assert_no_pairs!(pairs);

    let kind = match prefix.as_rule() {
        Rule::pattern_prefix_contains => PatternKind::Contains,
        Rule::pattern_prefix_exact => PatternKind::Exact,
        Rule::pattern_prefix_regex => PatternKind::Regex,
        Rule::pattern_prefix_glob => PatternKind::Glob,
        _ => unreachable!(),
    };

    let inner = match inner.as_rule() {
        Rule::string => parse_string(inner)?,
        Rule::pattern_raw => inner.as_str().into(),
        _ => unreachable!(),
    };

    Ok(Pattern::new(kind, inner))
}

/// Parses a top level [`Expr`], taking care of operator precedence. This is a
/// suitable entry point to parse a [`Rule::main`] token pair.
///
/// # Panics
/// Panics if the given pair is not of the expected rule, or if the inner pairs
/// don't match the expected count/rules.
pub fn parse_expr(pairs: Pairs<Rule>) -> Result<Expr, Error> {
    static PRATT_PARSER: Lazy<PrattParser<Rule>> = Lazy::new(|| {
        use pest::pratt_parser::Assoc::*;
        use pest::pratt_parser::Op;
        use Rule::*;

        PrattParser::new()
            .op(Op::infix(op_union, Left) | Op::infix(op_pipe, Left) | Op::infix(kw_or, Left))
            .op(Op::infix(op_intersection, Left)
                | Op::infix(op_ampersand, Left)
                | Op::infix(kw_and, Left))
            .op(Op::infix(op_backslash, Left)
                | Op::infix(op_tilde, Left)
                | Op::infix(kw_diff, Left))
            .op(Op::infix(op_delta, Left) | Op::infix(op_caret, Left) | Op::infix(kw_xor, Left))
            .op(Op::prefix(op_complement) | Op::prefix(op_exclamation) | Op::prefix(kw_not))
            .op(Op::postfix(op_plus) | Op::postfix(op_minus))
    });

    PRATT_PARSER
        .map_primary(|primary| match primary.as_rule() {
            Rule::expr => parse_expr(primary.into_inner()),
            Rule::atom => Ok(Expr::from(parse_atom(primary)?)),
            rule => unreachable!("expected atom, found {rule:?}"),
        })
        .map_infix(|lhs, op, rhs| {
            let rule = op.as_rule();
            let operator = match rule.as_infix_op() {
                Some(op) => op,
                _ => unreachable!("expected infix operation, found {rule:?}"),
            };
            Ok(Expr::from(InfixExpr::new(operator, lhs?, rhs?)))
        })
        .map_prefix(|op, expr| {
            let rule = op.as_rule();
            let operator = match rule.as_prefix_op() {
                Some(op) => op,
                _ => unreachable!("expected prefix operation, found {rule:?}"),
            };
            Ok(Expr::from(PrefixExpr::new(operator, expr?)))
        })
        .map_postfix(|expr, op| {
            let rule = op.as_rule();
            let operator = match rule.as_postfix_op() {
                Some(op) => op,
                _ => unreachable!("expected postfix operation, found {rule:?}"),
            };
            Ok(Expr::from(PostfixExpr::new(operator, expr?)))
        })
        .parse(pairs)
}

/// Parsing or validation of a test set expression failed.
#[derive(Debug, Error)]
pub enum Error {
    /// An integer literal was out of range.
    #[error("an integer was out of range")]
    IntegerError(#[from] ParseIntError),

    /// A string literal contained an invalid escape sequence.
    #[error("invalid escape sequence: \"{0}\"")]
    InvalidEscape(EcoString),

    /// There was a syntax error.
    #[error("syntax error")]
    PestError(#[from] Box<PestError<Rule>>),
}

/// Operator precedence is as follows in order of lowest to highest:
/// 1. union
/// 1. difference
/// 1. intersection
/// 1. symmetric difference
/// 1. complement (prefix)
/// 1. ancestors/descendant (postfix)
///
/// All infix operators are left associative.
pub mod op {
    /// An infix operator.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum Infix {
        /// A symmetric difference operator.
        SymmetricDifference,

        /// A set difference operator.
        Difference,

        /// A intersection operator.
        Intersection,

        /// A union operator.
        Union,
    }

    /// A prefix operator.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum Prefix {
        /// A complement.
        Complement,
    }

    /// A postfix operator.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum Postfix {
        /// The ancestors of a test set.
        Ancestors,

        /// The descendants of a test set.
        Descendants,
    }
}

/// Pattern literals.
pub mod pattern {
    use std::fmt::Display;

    use ecow::EcoString;

    /// The kind of a pattern literal.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum Kind {
        /// An exact pattern.
        Exact,

        /// A contains pattern.
        Contains,

        /// A regex pattern.
        Regex,

        /// A glob pattern.
        Glob,
    }

    /// A literal such as a string, number or pattern.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Pattern {
        /// The kind of pattenr this is.
        pub kind: Kind,

        /// The pattern literal.
        pub value: EcoString,
    }

    impl Pattern {
        /// Creates a new [`Pattern`].
        pub fn new<S>(kind: Kind, value: S) -> Self
        where
            S: Into<EcoString>,
        {
            Self {
                kind,
                value: value.into(),
            }
        }

        /// Creates a new exact [`Pattern`].
        pub fn exact<S>(value: S) -> Self
        where
            S: Into<EcoString>,
        {
            Self::new(Kind::Exact, value)
        }

        /// Creates a new contains [`Pattern`].
        pub fn contains<S>(value: S) -> Self
        where
            S: Into<EcoString>,
        {
            Self::new(Kind::Contains, value)
        }

        /// Creates a new regex [`Pattern`].
        pub fn regex<S>(value: S) -> Self
        where
            S: Into<EcoString>,
        {
            Self::new(Kind::Regex, value)
        }

        /// Creates a new glob [`Pattern`].
        pub fn glob<S>(value: S) -> Self
        where
            S: Into<EcoString>,
        {
            Self::new(Kind::Glob, value)
        }
    }

    impl Display for Pattern {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                f,
                "{}",
                match self.kind {
                    Kind::Exact => "=",
                    Kind::Contains => "~",
                    Kind::Regex => ":",
                    Kind::Glob => "#",
                }
            )?;

            write!(f, "'{}'", self.value)
        }
    }
}

/// A literal such as a string, number or pattern.
#[derive(Clone, PartialEq, Eq)]
pub enum Literal {
    /// A number literal.
    Number(i64),

    /// A string literal.
    String(EcoString),

    /// A pattern literal.
    Pattern(Pattern),
}

impl Literal {
    /// Creates a new number literal.
    pub fn number<I>(value: I) -> Self
    where
        I: Into<i64>,
    {
        Self::Number(value.into())
    }

    /// Creates a new string literal.
    pub fn string<S>(value: S) -> Self
    where
        S: Into<EcoString>,
    {
        Self::String(value.into())
    }

    /// Creates a new pattern literal.
    pub fn pattern<P>(value: P) -> Self
    where
        P: Into<Pattern>,
    {
        Self::Pattern(value.into())
    }

    /// Returns the inner number literal or `None` if it isn't a number.
    pub fn as_number(&self) -> Option<i64> {
        match self {
            Literal::Number(num) => Some(*num),
            _ => None,
        }
    }

    /// Returns the inner string literal or `None` if it isn't a string.
    pub fn as_string(&self) -> Option<&EcoString> {
        match self {
            Literal::String(str) => Some(str),
            _ => None,
        }
    }

    /// Returns the inner pattern literal or `None` if it isn't a pattern.
    pub fn as_pattern(&self) -> Option<&Pattern> {
        match self {
            Literal::Pattern(pat) => Some(pat),
            _ => None,
        }
    }
}

impl Debug for Literal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Literal::Number(num) => Debug::fmt(num, f),
            Literal::String(str) => Debug::fmt(str, f),
            Literal::Pattern(pat) => Debug::fmt(pat, f),
        }
    }
}

impl Display for Literal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Literal::Number(num) => Display::fmt(num, f),
            Literal::String(str) => Display::fmt(str, f),
            Literal::Pattern(pat) => Display::fmt(pat, f),
        }
    }
}

impl Eval for Literal {
    fn eval(&self, _ctx: &Context) -> Result<Value, EvalError> {
        Ok(Value::from_literal(self.clone()))
    }
}

/// A value with an identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variable {
    /// The identifier of this test set.
    pub id: Identifier,
}

impl Variable {
    /// Creates a new [`Variable`].
    pub fn new<S: Into<Identifier>>(id: S) -> Self {
        Self { id: id.into() }
    }
}

impl Display for Variable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.id)
    }
}

impl Eval for Variable {
    fn eval(&self, ctx: &Context) -> Result<Value, EvalError> {
        ctx.resolve_binding(self.id.as_str())
    }
}

/// A test set value or function with a name. This may be an alias, or a built
/// in test set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    /// The identifier of this test set.
    pub id: Identifier,

    /// The arguments passed to this function.
    pub args: EcoVec<Expr>,
}

impl Function {
    /// Creates a new [`Function`].
    pub fn new<S, I>(id: S, args: I) -> Self
    where
        S: Into<Identifier>,
        I: IntoIterator,
        I::Item: Into<Expr>,
    {
        Self {
            id: id.into(),
            args: args.into_iter().map(Into::into).collect(),
        }
    }
}

impl Eval for Function {
    fn eval(&self, ctx: &Context) -> Result<Value, EvalError> {
        let binding = ctx.resolve_binding(self.id.as_str())?.to_function()?;
        binding.call(
            ctx,
            &self
                .args
                .iter()
                .map(|expr| expr.eval(ctx))
                .collect::<Result<Vec<_>, _>>()?,
        )
    }
}

/// An expression atom, that is, a expression which does not contain any sub
/// expressions. This is the only allowed token function arguments.
#[derive(Clone, PartialEq, Eq)]
pub enum Atom {
    /// A literal value.
    Literal(Literal),

    /// A value using an identifier.
    Variable(Variable),

    /// A function call.
    Function(Function),
}

impl Debug for Atom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Atom::Literal(lit) => Debug::fmt(lit, f),
            Atom::Variable(var) => Debug::fmt(var, f),
            Atom::Function(func) => Debug::fmt(func, f),
        }
    }
}

impl Eval for Atom {
    fn eval(&self, ctx: &Context) -> Result<Value, EvalError> {
        match self {
            Atom::Literal(lit) => lit.eval(ctx),
            Atom::Variable(var) => var.eval(ctx),
            Atom::Function(func) => func.eval(ctx),
        }
    }
}

/// A prefix operator test set expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefixExpr {
    /// The operator of this expression.
    pub op: PrefixOp,

    /// The inner expression.
    pub expr: Expr,
}

impl PrefixExpr {
    /// Creates a new [`PrefixExpr`].
    pub fn new<E>(op: PrefixOp, expr: E) -> Self
    where
        E: Into<Expr>,
    {
        Self {
            op,
            expr: expr.into(),
        }
    }

    /// Creates a new [`PrefixExpr`] with a [`PrefixOp::Complement`].
    pub fn complement<E>(expr: E) -> Self
    where
        E: Into<Expr>,
    {
        Self::new(PrefixOp::Complement, expr)
    }
}

impl Eval for PrefixExpr {
    fn eval(&self, ctx: &Context) -> Result<Value, EvalError> {
        Ok(Value::TestSet(Arc::new(builtin::PrefixTestSet::new(
            self.op,
            self.expr.eval(ctx)?.to_test_set()?,
        ))))
    }
}

/// A binary operator test set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfixExpr {
    /// The operator of this expression.
    pub op: InfixOp,

    /// The left hand side of this expression.
    pub lhs: Expr,

    /// The right hand side of this expression.
    pub rhs: Expr,
}

impl InfixExpr {
    /// Creates a new [`InfixExpr`].
    pub fn new<L, R>(op: InfixOp, lhs: L, rhs: R) -> Self
    where
        L: Into<Expr>,
        R: Into<Expr>,
    {
        Self {
            op,
            lhs: lhs.into(),
            rhs: rhs.into(),
        }
    }

    /// Creates a new [`InfixExpr`] with a [`InfixOp::SymmetricDifference`].
    pub fn symmetric_difference<L, R>(lhs: L, rhs: R) -> Self
    where
        L: Into<Expr>,
        R: Into<Expr>,
    {
        Self::new(InfixOp::SymmetricDifference, lhs, rhs)
    }

    /// Creates a new [`InfixExpr`] with a [`InfixOp::Difference`].
    pub fn difference<L, R>(lhs: L, rhs: R) -> Self
    where
        L: Into<Expr>,
        R: Into<Expr>,
    {
        Self::new(InfixOp::Difference, lhs, rhs)
    }

    /// Creates a new [`InfixExpr`] with a [`InfixOp::Intersection`].
    pub fn intersection<L, R>(lhs: L, rhs: R) -> Self
    where
        L: Into<Expr>,
        R: Into<Expr>,
    {
        Self::new(InfixOp::Intersection, lhs, rhs)
    }

    /// Creates a new [`InfixExpr`] with a [`InfixOp::Union`].
    pub fn union<L, R>(lhs: L, rhs: R) -> Self
    where
        L: Into<Expr>,
        R: Into<Expr>,
    {
        Self::new(InfixOp::Union, lhs, rhs)
    }
}

impl Eval for InfixExpr {
    fn eval(&self, ctx: &Context) -> Result<Value, EvalError> {
        Ok(Value::TestSet(Arc::new(builtin::InfixTestSet::new(
            self.op,
            self.lhs.eval(ctx)?.to_test_set()?,
            self.rhs.eval(ctx)?.to_test_set()?,
        ))))
    }
}

/// A postfix operator test set expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostfixExpr {
    /// The operator of this expression.
    pub op: PostfixOp,

    /// The inner expression.
    pub expr: Expr,
}

impl PostfixExpr {
    /// Creates a new [`PostfixExpr`].
    pub fn new<E>(op: PostfixOp, expr: E) -> Self
    where
        E: Into<Expr>,
    {
        Self {
            op,
            expr: expr.into(),
        }
    }

    /// Creates a new [`PostfixExpr`] with a [`PostfixOp::Ancestors`].
    pub fn ancestors<E>(expr: E) -> Self
    where
        E: Into<Expr>,
    {
        Self::new(PostfixOp::Ancestors, expr)
    }

    /// Creates a new [`PostfixExpr`] with a [`PostfixOp::Descendants`].
    pub fn descendants<E>(expr: E) -> Self
    where
        E: Into<Expr>,
    {
        Self::new(PostfixOp::Descendants, expr)
    }
}

impl Eval for PostfixExpr {
    fn eval(&self, ctx: &Context) -> Result<Value, EvalError> {
        Ok(Value::TestSet(Arc::new(builtin::PostfixTestSet::new(
            self.op,
            self.expr.eval(ctx)?.to_test_set()?,
        ))))
    }
}

/// A test set expression which must be evaluated to a test set.
#[derive(Clone, PartialEq, Eq)]
pub enum InnerExpr {
    /// A prefix operator sub expression.
    Prefix(PrefixExpr),

    /// A infix operator sub expression.
    Infix(InfixExpr),

    /// A postfix operator sub expression.
    Postfix(PostfixExpr),

    /// An expression atom.
    Atom(Atom),
}

impl Debug for InnerExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InnerExpr::Prefix(expr) => Debug::fmt(expr, f),
            InnerExpr::Infix(expr) => Debug::fmt(expr, f),
            InnerExpr::Postfix(expr) => Debug::fmt(expr, f),
            InnerExpr::Atom(atom) => Debug::fmt(atom, f),
        }
    }
}

impl InnerExpr {
    pub fn as_prefix_expr(&self) -> Option<&PrefixExpr> {
        match self {
            InnerExpr::Prefix(p) => Some(p),
            _ => None,
        }
    }

    pub fn as_infix_expr(&self) -> Option<&InfixExpr> {
        match self {
            InnerExpr::Infix(p) => Some(p),
            _ => None,
        }
    }
}

impl Eval for InnerExpr {
    fn eval(&self, ctx: &Context) -> Result<Value, EvalError> {
        match self {
            InnerExpr::Prefix(expr) => expr.eval(ctx),
            InnerExpr::Infix(expr) => expr.eval(ctx),
            InnerExpr::Postfix(expr) => expr.eval(ctx),
            InnerExpr::Atom(atom) => atom.eval(ctx),
        }
    }
}

/// A test set expression which must be evaluated to a test set.
#[derive(Clone, PartialEq, Eq)]
pub struct Expr(pub Arc<InnerExpr>);

impl Expr {
    /// Returns a reference to the inner expresison.
    pub fn as_inner(&self) -> &InnerExpr {
        &self.0
    }

    /// Returns a mutable reference to the inner expression, cloning it if
    /// necessary.
    pub fn make_inner_mut(&mut self) -> &mut InnerExpr {
        Arc::make_mut(&mut self.0)
    }
}

impl Debug for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&*self.0, f)
    }
}

impl Eval for Expr {
    fn eval(&self, ctx: &Context) -> Result<Value, EvalError> {
        self.0.eval(ctx)
    }
}

impl From<PrefixExpr> for Expr {
    fn from(value: PrefixExpr) -> Self {
        Self(Arc::new(InnerExpr::Prefix(value)))
    }
}

impl From<InfixExpr> for Expr {
    fn from(value: InfixExpr) -> Self {
        Self(Arc::new(InnerExpr::Infix(value)))
    }
}

impl From<PostfixExpr> for Expr {
    fn from(value: PostfixExpr) -> Self {
        Self(Arc::new(InnerExpr::Postfix(value)))
    }
}

impl From<Function> for Expr {
    fn from(value: Function) -> Self {
        Self(Arc::new(InnerExpr::Atom(Atom::Function(value))))
    }
}

impl From<Atom> for Expr {
    fn from(value: Atom) -> Self {
        Self(Arc::new(InnerExpr::Atom(value)))
    }
}

impl From<Literal> for Expr {
    fn from(value: Literal) -> Self {
        Self(Arc::new(InnerExpr::Atom(Atom::Literal(value))))
    }
}

impl From<i64> for Expr {
    fn from(value: i64) -> Self {
        Self(Arc::new(InnerExpr::Atom(Atom::Literal(Literal::Number(
            value,
        )))))
    }
}

impl From<EcoString> for Expr {
    fn from(value: EcoString) -> Self {
        Self(Arc::new(InnerExpr::Atom(Atom::Literal(Literal::String(
            value,
        )))))
    }
}

impl From<Pattern> for Expr {
    fn from(value: Pattern) -> Self {
        Self(Arc::new(InnerExpr::Atom(Atom::Literal(Literal::Pattern(
            value,
        )))))
    }
}

impl From<Variable> for Expr {
    fn from(value: Variable) -> Self {
        Self(Arc::new(InnerExpr::Atom(Atom::Variable(value))))
    }
}

impl AsRef<InnerExpr> for Expr {
    fn as_ref(&self) -> &InnerExpr {
        self.as_inner()
    }
}

impl Borrow<InnerExpr> for Expr {
    fn borrow(&self) -> &InnerExpr {
        self.as_inner()
    }
}

impl Deref for Expr {
    type Target = InnerExpr;

    fn deref(&self) -> &Self::Target {
        self.as_inner()
    }
}

/// Optimize a test set expression by flattening according to the following
/// rules:
/// - `!!x -> x`
/// - `a \ !b -> a & b` and `a & !b -> a \ b`
///
/// Expressions are optimized inside out in order of the list above.
pub fn flatten(t: Expr) -> Expr {
    match &*t.0 {
        InnerExpr::Prefix(PrefixExpr { op, expr: inner }) => {
            let inner = flatten(inner.clone());

            // first rule
            // `!!x` -> `x`
            match op {
                PrefixOp::Complement => {
                    if let Some(PrefixExpr {
                        op: PrefixOp::Complement,
                        expr: x,
                    }) = inner.as_prefix_expr()
                    {
                        return x.clone();
                    }
                }
            }

            Expr(Arc::new(InnerExpr::Prefix(PrefixExpr::complement(inner))))
        }
        InnerExpr::Infix(InfixExpr { op, lhs, rhs }) => {
            let (lhs, rhs) = (flatten(lhs.clone()), flatten(rhs.clone()));

            // second rule
            // `a \ !b -> a & b` and `a & !b -> a \ b`
            'sec: {
                if let Some(PrefixExpr {
                    op: PrefixOp::Complement,
                    expr: not_rhs,
                }) = rhs.as_prefix_expr()
                {
                    let not_rhs = flatten(not_rhs.clone());

                    let swapped = match op {
                        InfixOp::Difference => InfixOp::Intersection,
                        InfixOp::Intersection => InfixOp::Difference,
                        _ => break 'sec,
                    };

                    return Expr(Arc::new(InnerExpr::Infix(InfixExpr::new(
                        swapped, lhs, not_rhs,
                    ))));
                }
            }

            Expr(Arc::new(InnerExpr::Infix(InfixExpr::new(*op, lhs, rhs))))
        }
        InnerExpr::Postfix(PostfixExpr { op, expr: inner }) => Expr(Arc::new(InnerExpr::Postfix(
            PostfixExpr::new(*op, flatten(inner.clone())),
        ))),
        InnerExpr::Atom(Atom::Function(Function { id, args })) => {
            Expr(Arc::new(InnerExpr::Atom(Atom::Function(Function {
                id: id.clone(),
                args: args.iter().cloned().map(flatten).collect(),
            }))))
        }
        InnerExpr::Atom(_) => t,
    }
}

#[cfg(test)]
mod tests {
    use ecow::eco_vec;
    use pest::consumes_to;

    use super::*;

    #[test]
    fn test_parse_test_set_expr_empty() {
        assert!(parse(" \t  ").is_err());
    }

    #[test]
    fn test_parse_test_set_expr() {
        let expr = parse("  (test(=exact) ~ func(:\" regex\")) & !val ").unwrap();
        assert_eq!(
            expr,
            Expr::from(InfixExpr::intersection(
                InfixExpr::difference(
                    Atom::Function(Function::new(
                        Identifier::new("test").unwrap(),
                        eco_vec![Pattern::exact("exact")],
                    )),
                    Atom::Function(Function::new(
                        Identifier::new("func").unwrap(),
                        eco_vec![Pattern::regex(" regex")],
                    )),
                ),
                PrefixExpr::complement(Variable {
                    id: Identifier::new("val").unwrap(),
                }),
            ))
        );
    }

    #[test]
    fn test_parse_identifier() {
        // simple words underscores numbers and dashes
        pest::parses_to! {
            parser: TestSetParser,
            input: "foo-6_bar",
            rule: Rule::identifier,
            tokens: [
                identifier(0, 9)
            ]
        };
        // double dashes are allowed
        pest::parses_to! {
            parser: TestSetParser,
            input: "v--l",
            rule: Rule::identifier,
            tokens: [
                identifier(0, 4)
            ]
        };
        // leading char must be alphabetic
        pest::fails_with! {
            parser: TestSetParser,
            input: "-a",
            rule: Rule::identifier,
            positives: [Rule::identifier],
            negatives: [],
            pos: 0
        };
    }

    #[test]
    fn test_parse_function() {
        // no whitespace between id and args
        pest::fails_with! {
            parser: TestSetParser,
            input: "val ()",
            rule: Rule::function,
            positives: [Rule::function_arguments],
            negatives: [],
            pos: 3
        };
        // can be empty
        pest::parses_to! {
            parser: TestSetParser,
            input: "val()",
            rule: Rule::function,
            tokens: [
                function(0, 5, [
                    identifier(0, 3),
                    function_arguments(3, 5)
                ])
            ]
        };
        // can be empty
        pest::parses_to! {
            parser: TestSetParser,
            input: "val( )",
            rule: Rule::function,
            tokens: [
                function(0, 6, [
                    identifier(0, 3),
                    function_arguments(3, 6)
                ])
            ]
        };
    }

    #[test]
    fn test_parse_function_with_args() {
        pest::parses_to! {
            parser: TestSetParser,
            input: "val(a)",
            rule: Rule::function,
            tokens: [
                function(0, 6, [
                    identifier(0, 3),
                    function_arguments(3, 6, [
                        expr(4, 5, [
                            atom(4, 5, [
                                variable(4, 5, [
                                    identifier(4, 5)
                                ])
                            ])
                        ])
                    ])
                ])
            ]
        };
        // trailing comma is allowed
        pest::parses_to! {
            parser: TestSetParser,
            input: "val(a,)",
            rule: Rule::function,
            tokens: [
                function(0, 7, [
                    identifier(0, 3),
                    function_arguments(3, 7, [
                        expr(4, 5, [
                            atom(4, 5, [
                                variable(4, 5, [
                                    identifier(4, 5)
                                ])
                            ])
                        ])
                    ])
                ])
            ]
        };
        // trailing command is allowd once, expression missing
        pest::fails_with! {
            parser: TestSetParser,
            input: "val(a,,)",
            rule: Rule::function,
            positives: [Rule::expr],
            negatives: [],
            pos: 6
        };
    }

    #[test]
    fn test_parse_number() {
        let expr = parse("1").unwrap();
        assert_eq!(expr, Expr::from(Literal::number(1)));
    }

    #[test]
    fn test_parse_string_escape() {
        let expr = parse(r#""\\\u{00f3}\t\r\n""#).unwrap();
        assert_eq!(expr, Expr::from(Literal::string("\\\u{00f3}\t\r\n")));
    }

    #[test]
    fn test_parse_pattern() {
        // whitespace is not allowed
        pest::fails_with! {
            parser: TestSetParser,
            input: "~ a",
            rule: Rule::pattern,
            positives: [Rule::string, Rule::pattern_raw],
            negatives: [],
            pos: 1
        };
        // no inner pattern
        pest::fails_with! {
            parser: TestSetParser,
            input: ":",
            rule: Rule::pattern,
            positives: [Rule::string, Rule::pattern_raw],
            negatives: [],
            pos: 1
        };
        // simple raw pattern
        pest::parses_to! {
            parser: TestSetParser,
            input: "=a",
            rule: Rule::pattern,
            tokens: [
                pattern(0, 2, [
                    pattern_prefix_exact(0, 1),
                    pattern_raw(1, 2),
                ])
            ]
        };
    }

    #[test]
    fn test_parse_pattern_string() {
        pest::parses_to! {
            parser: TestSetParser,
            input: r#":"a""#,
            rule: Rule::pattern,
            tokens: [
                pattern(0, 4, [
                    pattern_prefix_regex(0, 1),
                    string(1, 4, [
                        double_string_inner(2, 3)
                    ]),
                ])
            ]
        };
    }

    #[test]
    fn test_parse_pattern_raw() {
        // anything but whitespace
        pest::parses_to! {
            parser: TestSetParser,
            input: "#a--**-+",
            rule: Rule::pattern,
            tokens: [
                pattern(0, 8, [
                    pattern_prefix_glob(0, 1),
                    pattern_raw(1, 8),
                ])
            ]
        };
        // parenthesis must match
        pest::parses_to! {
            parser: TestSetParser,
            input: ":a(a)",
            rule: Rule::pattern,
            tokens: [
                pattern(0, 5, [
                    pattern_prefix_regex(0, 1),
                    pattern_raw(1, 5, [
                        pattern_raw(3, 4),
                    ]),
                ])
            ]
        };
        pest::fails_with! {
            parser: TestSetParser,
            input: ":(a",
            rule: Rule::pattern,
            positives: [Rule::string, Rule::pattern_raw],
            negatives: [],
            pos: 1
        };
    }

    #[test]
    fn test_parse_pattern_raw_in_function() {
        let expr = parse("func(:hello(-world)+!)").unwrap();
        assert_eq!(
            expr,
            Expr::from(Atom::Function(Function::new(
                Identifier::new("func").unwrap(),
                eco_vec![Pattern::regex("hello(-world)+!")],
            )),)
        );
    }

    fn dummy_leaf_expr() -> Expr {
        Expr::from(Literal::Number(0))
    }

    #[test]
    fn test_flatten_complement() {
        let pre = Expr::from(PrefixExpr::complement(Expr::from(PrefixExpr::complement(
            dummy_leaf_expr(),
        ))));

        let post = dummy_leaf_expr();

        assert_eq!(flatten(pre), post);
    }

    #[test]
    fn test_flatten_diff_comp_to_inter() {
        let pre = Expr::from(InfixExpr::difference(
            dummy_leaf_expr(),
            Expr::from(PrefixExpr::complement(dummy_leaf_expr())),
        ));

        let post = Expr::from(InfixExpr::intersection(
            dummy_leaf_expr(),
            dummy_leaf_expr(),
        ));

        assert_eq!(flatten(pre), post);
    }

    #[test]
    fn test_flatten_inter_comp_to_diff() {
        let pre = Expr::from(InfixExpr::intersection(
            dummy_leaf_expr(),
            Expr::from(PrefixExpr::complement(dummy_leaf_expr())),
        ));

        let post = Expr::from(InfixExpr::difference(dummy_leaf_expr(), dummy_leaf_expr()));

        assert_eq!(flatten(pre), post);
    }

    #[test]
    fn test_flatten_atom_no_op() {
        let pre = Expr::from(Variable::new(Identifier::new("var").unwrap()));
        let post = pre.clone();

        assert_eq!(flatten(pre), post);
    }

    #[test]
    fn test_flatten_func_no_op() {
        let pre = Expr::from(Function {
            id: Identifier::new("func").unwrap(),
            args: eco_vec![],
        });
        let post = pre.clone();

        assert_eq!(flatten(pre), post);
    }
}
