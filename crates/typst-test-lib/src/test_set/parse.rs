//! Test set expression parsing.

use std::char::CharTryFromError;
use std::fmt::Debug;
use std::hash::Hash;
use std::ops::Deref;
use std::sync::{Arc, LazyLock};

use pest::iterators::Pair;
use pest::pratt_parser::PrattParser;
use pest::Parser;
use pest_derive::Parser;
use thiserror::Error;

/// An error for parsing failures.
#[derive(Debug, Error)]
pub enum Error {
    /// The uiunput ended unexpectedly.
    #[error("expected one of {rules:?}, found nothing")]
    UnexpectedEOI {
        /// The expected rules.
        rules: Vec<Rule>,
    },

    /// Expected no further input, but found some.
    #[error("expected no further pairs, found {rule:?}")]
    ExpectedEOI {
        /// The rule that was found.
        rule: Rule,
    },

    /// Expected a certain set of rules. but found a different rule.
    #[error("expected one of {rules:?}, found {found:?}")]
    UnexpectedRules {
        /// The expected rules
        rules: Vec<Rule>,

        /// The rule that was found.
        found: Rule,
    },

    /// A string escape did not describe a valid unicode code point.
    #[error("a string escape did not describe a valid unicode code point")]
    UnicodeEscape(#[from] CharTryFromError),

    /// A pest error occurred.
    #[error("the expression could not be parsed")]
    Pest(#[from] Box<pest::error::Error<Rule>>),

    /// A regex pattern could not be parsed.
    #[error("a regex pattern could not be parsed")]
    Regex(#[from] regex::Error),

    /// A glob pattern could not be parsed.
    #[error("a glob pattern could not be parsed")]
    Glob(#[from] glob::PatternError),
}

/// An extension trait for pest iterators and its adapters.
pub trait PairsExt<'a> {
    /// If there is another pair ensure it is of the expected rules.
    fn try_expect_pair(&mut self, rules: &[Rule]) -> Result<Option<Pair<'a, Rule>>, Error>;

    /// Ensure there is a pair of one of the expected rules.
    fn expect_pair(&mut self, rules: &[Rule]) -> Result<Pair<'a, Rule>, Error>;

    /// Ensure there are no further pairs.
    fn expect_end(&mut self) -> Result<(), Error>;
}

impl<'a, I> PairsExt<'a> for I
where
    I: Iterator<Item = Pair<'a, Rule>>,
{
    fn try_expect_pair(&mut self, rules: &[Rule]) -> Result<Option<Pair<'a, Rule>>, Error> {
        self.next()
            .map(|pair| pair.expect_rules(rules).map(|_| pair))
            .transpose()
    }

    fn expect_pair(&mut self, rules: &[Rule]) -> Result<Pair<'a, Rule>, Error> {
        self.next()
            .ok_or_else(|| Error::UnexpectedEOI {
                rules: rules.to_owned(),
            })
            .and_then(|pair| pair.expect_rules(rules).map(|_| pair))
    }

    fn expect_end(&mut self) -> Result<(), Error> {
        if let Some(pair) = self.next() {
            return Err(Error::ExpectedEOI {
                rule: pair.as_rule(),
            });
        }

        Ok(())
    }
}

/// An extension trait for the [`Pair`] type.
pub trait PairExt<'a> {
    fn expect_rules(&self, rule: &[Rule]) -> Result<(), Error>;
}

impl<'a> PairExt<'a> for Pair<'a, Rule> {
    fn expect_rules(&self, rules: &[Rule]) -> Result<(), Error> {
        if !rules.contains(&self.as_rule()) {
            return Err(Error::UnexpectedRules {
                rules: rules.to_owned(),
                found: self.as_rule(),
            });
        }

        Ok(())
    }
}

/// A parser for test set expressions.
#[derive(Parser)]
#[grammar = "test_set/grammar.pest"]
struct TestSetParser;

/// The pratt parser defining the operator precedence.
static PRATT_PARSER: LazyLock<PrattParser<Rule>> = LazyLock::new(|| {
    use pest::pratt_parser::{Assoc, Op};

    PrattParser::new()
        .op(Op::infix(Rule::infix_op_pipe, Assoc::Left) | Op::infix(Rule::infix_op_or, Assoc::Left))
        .op(Op::infix(Rule::infix_op_amper, Assoc::Left)
            | Op::infix(Rule::infix_op_and, Assoc::Left))
        .op(Op::infix(Rule::infix_op_tilde, Assoc::Left)
            | Op::infix(Rule::infix_op_diff, Assoc::Left))
        .op(Op::infix(Rule::infix_op_caret, Assoc::Left)
            | Op::infix(Rule::infix_op_xor, Assoc::Left))
        .op(Op::prefix(Rule::prefix_op_excl) | Op::prefix(Rule::prefix_op_not))
});

impl Rule {
    /// Turns this rule into the respective prefix operator.
    fn to_prefix(self) -> Option<PrefixOp> {
        Some(match self {
            Rule::prefix_op_excl | Rule::prefix_op_not => PrefixOp::Not,
            _ => return None,
        })
    }

    /// Turns this rule into the respective infix operator.
    fn to_infix(self) -> Option<InfixOp> {
        Some(match self {
            Rule::infix_op_pipe | Rule::infix_op_or => InfixOp::Union,
            Rule::infix_op_amper | Rule::infix_op_and => InfixOp::Inter,
            Rule::infix_op_tilde | Rule::infix_op_diff => InfixOp::Diff,
            Rule::infix_op_caret | Rule::infix_op_xor => InfixOp::SymDiff,
            _ => return None,
        })
    }

    /// The token this rule corresponds to, or a sensble substitute for
    /// diagnostics.
    pub fn name(self) -> &'static str {
        match self {
            Rule::EOI => "EOI",
            Rule::main | Rule::expr | Rule::expr_term | Rule::expr_atom => "expression",
            Rule::expr_group => "expression group",
            Rule::prefix_op => "prefix op",
            Rule::prefix_op_excl => "symbol complement op",
            Rule::prefix_op_not => "literal complement op",
            Rule::infix_op => "infix op",
            Rule::infix_op_caret => "symbol symmetric difference op",
            Rule::infix_op_amper => "symbol intersection op",
            Rule::infix_op_tilde => "symbol difference op",
            Rule::infix_op_pipe => "symbol union op",
            Rule::infix_op_xor => "literal symmetric difference op",
            Rule::infix_op_and => "literal intersection op",
            Rule::infix_op_diff => "literal difference op",
            Rule::infix_op_or => "literal union op",
            Rule::id => "identifier",
            Rule::func | Rule::func_args | Rule::func_args_inner => "function arguments",
            Rule::func_args_sep => "comma",
            Rule::func_args_delim_open => "opening parenthesis",
            Rule::func_args_delim_close => "closing parenthesis",
            Rule::pat => "pattern",
            Rule::pat_kind => "pattern kind",
            Rule::pat_kind_glob => "glob pattern kind",
            Rule::pat_kind_regex => "regex pattern kind",
            Rule::pat_kind_contains => "contains pattern kind",
            Rule::pat_kind_exact => "exact pattern kind",
            Rule::pat_kind_path => "path pattern kind",
            Rule::pat_inner | Rule::pat_pat => "pattern",
            Rule::pat_raw_lit => "raw pattern literal",
            Rule::pat_sep => "colon",
            Rule::str => "string",
            Rule::str_single | Rule::str_single_inner => "single quoted string",
            Rule::str_double | Rule::str_double_inner => "double quoted string",
            Rule::str_single_delim => "single quote",
            Rule::str_double_delim => "double quote",
            Rule::str_single_char | Rule::str_double_char => "any",
            Rule::str_double_esc_meta
            | Rule::str_double_esc_ascii
            | Rule::str_double_esc_unicode => "escape",
            Rule::num | Rule::num_inner => "number",
            Rule::num_part => "digit",
            Rule::num_sep => "underscore",
            Rule::WHITESPACE => "whitespace",
        }
    }

    /// The token for this rule to use in diagnostics.
    pub fn token(self) -> &'static str {
        match self {
            Rule::EOI => "<EOI>",
            Rule::main | Rule::expr | Rule::expr_term | Rule::expr_atom => "<expr>",
            Rule::expr_group => "(...)",
            Rule::prefix_op => "<prefix op>",
            Rule::prefix_op_excl => "!",
            Rule::prefix_op_not => "not",
            Rule::infix_op => "<infix op>",
            Rule::infix_op_caret => "^",
            Rule::infix_op_amper => "&",
            Rule::infix_op_tilde => "~",
            Rule::infix_op_pipe => "|",
            Rule::infix_op_xor => "xor",
            Rule::infix_op_and => "and",
            Rule::infix_op_diff => "diff",
            Rule::infix_op_or => "or",
            Rule::id => "<ident>",
            Rule::func | Rule::func_args | Rule::func_args_inner => "<args>",
            Rule::func_args_sep => "<comma>",
            Rule::func_args_delim_open => "(",
            Rule::func_args_delim_close => ")",
            Rule::pat => "<kind>:<pattern>",
            Rule::pat_kind => "<pattern kind>",
            Rule::pat_kind_glob => "glob",
            Rule::pat_kind_regex => "regex",
            Rule::pat_kind_contains => "contains",
            Rule::pat_kind_exact => "exact",
            Rule::pat_kind_path => "path",
            Rule::pat_inner | Rule::pat_pat => "<pattern>",
            Rule::pat_raw_lit => "<raw pattern>",
            Rule::pat_sep => ":",
            Rule::str => "<str>",
            Rule::str_single | Rule::str_single_inner => "'...'",
            Rule::str_double | Rule::str_double_inner => "\"...\"",
            Rule::str_single_delim => "'",
            Rule::str_double_delim => "\"",
            Rule::str_single_char | Rule::str_double_char => "<ANY>",
            Rule::str_double_esc_meta
            | Rule::str_double_esc_ascii
            | Rule::str_double_esc_unicode => "<escape>",
            Rule::num | Rule::num_inner => "<number>",
            Rule::num_part => "<digit>",
            Rule::num_sep => "_",
            Rule::WHITESPACE => "<WHITESPACE>",
        }
    }
}

/// Define a simple str wrapper new type.
macro_rules! define_str_newtype {
    (
        $(#[$meta:meta])*
        $vis:vis struct $id:ident;
    ) => {
        $(#[$meta])*
        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        $vis struct $id(pub Arc<String>);

        impl $id {
            #[doc = concat!("Creates a new [`", stringify!($id), "`] from the given string.")]
            pub fn new<S: Into<String>>(str: S) -> Self {
                Self(Arc::new(str.into()))
            }

            /// The inner string.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Debug for $id {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.pad(&self.0)
            }
        }

        impl Deref for $id {
            type Target = str;

            fn deref(&self) -> &Self::Target {
                self.as_str()
            }
        }

        impl AsRef<str> for $id {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }
    };
}

define_str_newtype! {
    /// An identifier.
    pub struct Ident;
}

define_str_newtype! {
    /// A string literal.
    pub struct Str;
}

define_str_newtype! {
    /// A path literal.
    pub struct Path;
}

/// A glob pattern literal.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Glob(pub Arc<glob::Pattern>);

impl Glob {
    /// Creates a new [`Glob`] from the given pattern.
    pub fn new(pat: glob::Pattern) -> Self {
        Self(Arc::new(pat))
    }

    /// The inner glob pattern.
    pub fn as_glob(&self) -> &glob::Pattern {
        &self.0
    }

    /// The inner string.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Debug for Glob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.as_str())
    }
}

impl Deref for Glob {
    type Target = glob::Pattern;

    fn deref(&self) -> &Self::Target {
        self.as_glob()
    }
}

impl AsRef<glob::Pattern> for Glob {
    fn as_ref(&self) -> &glob::Pattern {
        self.as_glob()
    }
}

impl AsRef<str> for Glob {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// A regex pattern literal.
#[derive(Clone)]
pub struct Regex(pub Arc<regex::Regex>);

impl Regex {
    /// Creates a new [`Regex`] from the given pattern.
    pub fn new(pat: regex::Regex) -> Self {
        Self(Arc::new(pat))
    }

    /// The inner regex pattern.
    pub fn as_regex(&self) -> &regex::Regex {
        &self.0
    }

    /// The inner string.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Debug for Regex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.as_str())
    }
}

impl PartialEq for Regex {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_str() == other.0.as_str()
    }
}

impl Eq for Regex {}

impl Hash for Regex {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.as_str().hash(state);
    }
}

impl Deref for Regex {
    type Target = regex::Regex;

    fn deref(&self) -> &Self::Target {
        self.as_regex()
    }
}

impl AsRef<regex::Regex> for Regex {
    fn as_ref(&self) -> &regex::Regex {
        self.as_regex()
    }
}

impl AsRef<str> for Regex {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// A number literal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Num(pub usize);

/// A pattern matching identifiers of tests.
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Pat {
    /// A regex pattern, matches if the glob matches the haystack.
    Glob(Glob),

    /// A regex pattern, matches if the regex matches the haystack.
    Regex(Regex),

    /// A contains pattern, matches if the pattern is contained in the haystack.
    Contains(Str),

    /// An exact pattern, matches if the pattern equals the haystack.
    Exact(Str),

    /// A path pattern, matches if the pattern refers to the haystack.
    ///
    /// Absolute paths will be treated as relative from the suite root, relative
    /// paths will be treated as relative to a PWD given by the evaluation
    /// context.
    Path(Path),
}

impl Debug for Pat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (prefix, pat) = match self {
            Pat::Glob(glob) => ("glob", glob.as_str()),
            Pat::Regex(regex) => ("regex", regex.as_str()),
            Pat::Contains(pat) => ("contains", pat.as_str()),
            Pat::Exact(pat) => ("exact", pat.as_str()),
            Pat::Path(path) => ("path", path.as_str()),
        };

        write!(f, "{prefix}:{pat:?}")
    }
}

/// An atom, i.e. a leaf node within a test set expression such as an identifier
/// or pattern.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Atom {
    /// An identifier.
    Ident(Ident),

    /// A number literal.
    Num(Num),

    /// A string literal.
    Str(Str),

    /// A pattern literal.
    Pat(Pat),
}

/// A test set function.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Function {
    /// The identifier of this function.
    pub id: Ident,

    /// The arguments of this function.
    pub args: Vec<Expr>,
}

/// A unary prefix operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PrefixOp {
    /// The negation operator.
    Not,
}

impl PrefixOp {
    /// The symbol representing this operator.
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Not => "!",
        }
    }
}

/// A binary infix operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InfixOp {
    /// The union/or operator.
    Union,

    /// The intersection/and operator.
    Inter,

    /// The difference operator.
    Diff,

    /// The symmetric difference/xor operator.
    SymDiff,
}

impl InfixOp {
    /// The symbol representing this operator.
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Union => "|",
            Self::Inter => "&",
            Self::Diff => "~",
            Self::SymDiff => "^",
        }
    }
}

/// An general expression.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Expr {
    /// An expression atom.
    Atom(Atom),

    /// A function call expression.
    Func(Function),

    /// A prefix expression.
    Prefix {
        /// The unary prefix operator.
        op: PrefixOp,

        /// The inner expression.
        expr: Arc<Expr>,
    },

    /// An infix expression.
    Infix {
        /// The binary infix operator.
        op: InfixOp,

        /// The left hand side of this binary expression.
        lhs: Arc<Expr>,

        /// The right hand side of this binary expression.
        rhs: Arc<Expr>,
    },
}

/// Parse the given input into a test set expression.
#[tracing::instrument(ret)]
pub fn parse(input: &str) -> Result<Expr, Error> {
    // unwrap main into its root level expr, removing the EOI pair
    let root_expr = TestSetParser::parse(Rule::main, input)
        .map_err(|err| Box::new(err.renamed_rules(|r| r.token().to_owned())))?
        .next()
        .unwrap()
        .into_inner()
        .next()
        .unwrap();

    parse_expr(root_expr, &PRATT_PARSER)
}

/// Parse the given pair into an expression.
fn parse_expr(pair: Pair<Rule>, pratt: &PrattParser<Rule>) -> Result<Expr, Error> {
    pratt
        .map_primary(|primary| match primary.as_rule() {
            Rule::id => parse_id(primary).map(Atom::Ident).map(Expr::Atom),
            Rule::pat_inner => parse_pat(primary).map(Atom::Pat).map(Expr::Atom),
            Rule::str_single | Rule::str_double => {
                parse_str(primary).map(Atom::Str).map(Expr::Atom)
            }
            Rule::num_inner => parse_num(primary).map(Atom::Num).map(Expr::Atom),
            Rule::func => parse_func(primary, pratt).map(Expr::Func),
            Rule::expr => parse_expr(primary, pratt),
            x => unreachable!("unhandled primary expression {x:?}"),
        })
        .map_prefix(|op, expr| match op.as_rule().to_prefix() {
            Some(op) => Ok(Expr::Prefix {
                op,
                expr: Arc::new(expr?),
            }),
            None => unreachable!("unhandled prefix operator {:?}", op.as_rule()),
        })
        .map_infix(|lhs, op, rhs| match op.as_rule().to_infix() {
            Some(op) => Ok(Expr::Infix {
                op,
                lhs: Arc::new(lhs?),
                rhs: Arc::new(rhs?),
            }),
            None => unreachable!("unhandled infix operator {:?}", op.as_rule()),
        })
        .parse(pair.into_inner())
}

/// Parse the given pair into an identifier.
fn parse_id(pair: Pair<Rule>) -> Result<Ident, Error> {
    pair.expect_rules(&[Rule::id])?;
    Ok(Ident::new(pair.as_str()))
}

/// Parse the given pair into a pattern literal.
fn parse_pat(pair: Pair<Rule>) -> Result<Pat, Error> {
    pair.expect_rules(&[Rule::pat_inner])?;
    let mut pairs = pair.into_inner();

    let kind = pairs.expect_pair(&[Rule::pat_kind])?.as_str();
    let _ = pairs.expect_pair(&[Rule::pat_sep])?;
    let inner = pairs.expect_pair(&[Rule::pat_raw_lit, Rule::str_double, Rule::str_single])?;
    pairs.expect_end()?;

    let pat = if inner.as_rule() == Rule::pat_raw_lit {
        Str::new(inner.as_str())
    } else {
        parse_str(inner)?
    };

    Ok(match kind {
        "g" | "glob" => Pat::Glob(Glob::new(glob::Pattern::new(&pat.0)?)),
        "r" | "regex" => Pat::Regex(Regex::new(regex::Regex::new(&pat.0)?)),
        "c" | "contains" => Pat::Contains(pat),
        "e" | "exact" => Pat::Exact(pat),
        "p" | "path" => Pat::Path(Path(pat.0)),
        _ => unreachable!("unhandled kind: {kind:?}"),
    })
}

/// Parse the given pair into a number literal.
fn parse_num(pair: Pair<Rule>) -> Result<Num, Error> {
    pair.expect_rules(&[Rule::num_inner])?;
    let mut s = pair.as_str().as_bytes();
    let mut num = 0;

    while let Some((&d, rest)) = s.split_first() {
        debug_assert!(
            matches!(d, b'0'..=b'9' | b'_'),
            "parser should ensure this is only digits and underscores",
        );

        s = rest;

        if d == b'_' {
            continue;
        }

        // decimal equivalent of shift left and or LSB
        num *= 10;
        num += (d - b'0') as usize;
    }

    Ok(Num(num))
}

/// Parse the given pair into a string literal.
fn parse_str(pair: Pair<Rule>) -> Result<Str, Error> {
    pair.expect_rules(&[Rule::str_single, Rule::str_double])?;

    let mut pairs = pair.into_inner();
    let start = pairs.expect_pair(&[Rule::str_single_delim, Rule::str_double_delim])?;
    let inner = pairs.expect_pair(&[Rule::str_single_inner, Rule::str_double_inner])?;
    let _ = pairs.expect_pair(&[start.as_rule()])?;
    pairs.expect_end()?;

    match inner.as_rule() {
        Rule::str_single_inner => Ok(Str::new(inner.as_str())),
        Rule::str_double_inner => {
            if !inner.as_str().contains('\\') {
                Ok(Str::new(inner.as_str()))
            } else {
                let mut buf = String::with_capacity(inner.as_str().len());

                let mut rest = inner.as_str();
                while let Some((lit, esc)) = rest.split_once('\\') {
                    buf.push_str(lit);

                    if esc.starts_with(['\\', '"', 'n', 'r', 't']) {
                        match esc.as_bytes()[0] {
                            b'\\' => buf.push('\\'),
                            b'"' => buf.push('"'),
                            b'n' => buf.push('\n'),
                            b'r' => buf.push('\r'),
                            b't' => buf.push('\t'),
                            _ => unreachable!(),
                        }
                        rest = &esc[1..];
                    } else if let Some(esc) = esc.strip_prefix("u{") {
                        let (digits, other) =
                            esc.split_once('}').expect("parser ensure closing '}'");

                        buf.push(
                            u32::from_str_radix(digits, 16)
                                .expect("parser ensures hex digits only")
                                .try_into()?,
                        );

                        rest = other;
                    } else {
                        unreachable!(
                            "unhandled string escape sequence: {:?}",
                            esc.split_once(' ').map(|(p, _)| p).unwrap_or(esc)
                        );
                    }
                }

                Ok(Str::new(buf))
            }
        }
        _ => unreachable!(),
    }
}

/// Parse th given pair into a function
fn parse_func(pair: Pair<Rule>, pratt: &PrattParser<Rule>) -> Result<Function, Error> {
    pair.expect_rules(&[Rule::func])?;
    let mut pairs = pair.into_inner();

    let id = pairs.expect_pair(&[Rule::id])?;
    let id = parse_id(id)?;

    let args = pairs.expect_pair(&[Rule::func_args])?;
    let mut pairs = args.into_inner();

    let _ = pairs.expect_pair(&[Rule::func_args_delim_open])?;
    let args_or_close = pairs.expect_pair(&[Rule::func_args_inner, Rule::func_args_delim_close])?;
    let args = if args_or_close.as_rule() == Rule::func_args_inner {
        let _ = pairs.expect_pair(&[Rule::func_args_delim_close])?;

        let mut pairs = args_or_close.into_inner();

        let mut args = vec![];
        loop {
            let Some(arg) = pairs.try_expect_pair(&[Rule::expr])? else {
                break;
            };

            args.push(parse_expr(arg, pratt)?);

            let Some(_) = pairs.try_expect_pair(&[Rule::func_args_sep])? else {
                break;
            };
        }

        args
    } else {
        vec![]
    };

    pairs.expect_end()?;

    Ok(Function { id, args })
}

#[cfg(test)]
mod tests {
    use super::*;

    // TODO: test failures

    #[test]
    fn test_parse_single_string() {
        assert_eq!(
            parse(r#"'a string \'"#).unwrap(),
            Expr::Atom(Atom::Str(Str::new(r#"a string \"#)))
        );
    }

    #[test]
    fn test_parse_double_string() {
        assert_eq!(
            parse(r#""a string \" \u{30}""#).unwrap(),
            Expr::Atom(Atom::Str(Str::new(r#"a string " 0"#)))
        );
    }

    #[test]
    fn test_parse_ident() {
        assert_eq!(
            parse("abc").unwrap(),
            Expr::Atom(Atom::Ident(Ident::new("abc")))
        );
        assert_eq!(
            parse("a-bc").unwrap(),
            Expr::Atom(Atom::Ident(Ident::new("a-bc")))
        );
        assert_eq!(
            parse("a__bc-").unwrap(),
            Expr::Atom(Atom::Ident(Ident::new("a__bc-")))
        );
    }

    #[test]
    fn test_parse_number() {
        assert_eq!(parse("1234").unwrap(), Expr::Atom(Atom::Num(Num(1234))));
        assert_eq!(parse("1_000").unwrap(), Expr::Atom(Atom::Num(Num(1000))));
    }

    #[test]
    fn test_parse_pattern_string() {
        assert_eq!(
            parse("r:'^abc*$'").unwrap(),
            Expr::Atom(Atom::Pat(Pat::Regex(Regex::new(
                regex::Regex::new("^abc*$").unwrap()
            ))))
        );
        assert_eq!(
            parse(r#"glob:"a/**/b""#).unwrap(),
            Expr::Atom(Atom::Pat(Pat::Glob(Glob::new(
                glob::Pattern::new("a/**/b").unwrap()
            ))))
        );
    }

    #[test]
    fn test_parse_pattern_raw() {
        assert_eq!(
            parse("g:a/**/b").unwrap(),
            Expr::Atom(Atom::Pat(Pat::Glob(Glob::new(
                glob::Pattern::new("a/**/b").unwrap()
            ))))
        );
        assert_eq!(
            parse("p:a/b").unwrap(),
            Expr::Atom(Atom::Pat(Pat::Path(Path::new("a/b"))))
        );
    }

    #[test]
    fn test_parse_func_no_args() {
        assert_eq!(
            parse("func()").unwrap(),
            Expr::Func(Function {
                id: Ident::new("func"),
                args: vec![],
            })
        );
        assert_eq!(
            parse("func(  )").unwrap(),
            Expr::Func(Function {
                id: Ident::new("func"),
                args: vec![],
            })
        );
    }

    #[test]
    fn test_parse_func_simple_args() {
        assert_eq!(
            parse("func( a, 1  , p:'a/b')").unwrap(),
            Expr::Func(Function {
                id: Ident::new("func"),
                args: vec![
                    Expr::Atom(Atom::Ident(Ident::new("a"))),
                    Expr::Atom(Atom::Num(Num(1))),
                    Expr::Atom(Atom::Pat(Pat::Path(Path::new("a/b"))))
                ],
            })
        );
    }

    #[test]
    fn test_parse_prefix_expression() {
        assert_eq!(
            parse("! not 0").unwrap(),
            Expr::Prefix {
                op: PrefixOp::Not,
                expr: Arc::new(Expr::Prefix {
                    op: PrefixOp::Not,
                    expr: Arc::new(Expr::Atom(Atom::Num(Num(0)))),
                }),
            }
        );
    }

    #[test]
    fn test_parse_infix_expression() {
        assert_eq!(
            parse("0 and 1 or 2").unwrap(),
            Expr::Infix {
                op: InfixOp::Union,
                lhs: Arc::new(Expr::Infix {
                    op: InfixOp::Inter,
                    lhs: Arc::new(Expr::Atom(Atom::Num(Num(0)))),
                    rhs: Arc::new(Expr::Atom(Atom::Num(Num(1)))),
                }),
                rhs: Arc::new(Expr::Atom(Atom::Num(Num(2)))),
            }
        );

        assert_eq!(
            parse("0 and (1 or 2)").unwrap(),
            Expr::Infix {
                op: InfixOp::Inter,
                lhs: Arc::new(Expr::Atom(Atom::Num(Num(0)))),
                rhs: Arc::new(Expr::Infix {
                    op: InfixOp::Union,
                    lhs: Arc::new(Expr::Atom(Atom::Num(Num(1)))),
                    rhs: Arc::new(Expr::Atom(Atom::Num(Num(2)))),
                }),
            }
        );
    }

    #[test]
    fn test_parse_expression() {
        assert_eq!(
            parse("regex:'abc' and not (abc | func(0))").unwrap(),
            Expr::Infix {
                op: InfixOp::Inter,
                lhs: Arc::new(Expr::Atom(Atom::Pat(Pat::Regex(Regex::new(
                    regex::Regex::new("abc").unwrap()
                ))))),
                rhs: Arc::new(Expr::Prefix {
                    op: PrefixOp::Not,
                    expr: Arc::new(Expr::Infix {
                        op: InfixOp::Union,
                        lhs: Arc::new(Expr::Atom(Atom::Ident(Ident::new("abc")))),
                        rhs: Arc::new(Expr::Func(Function {
                            id: Ident::new("func"),
                            args: vec![Expr::Atom(Atom::Num(Num(0)))]
                        })),
                    }),
                }),
            }
        );
    }
}
