use once_cell::sync::Lazy;
use pest::error::Error;
use pest::iterators::{Pair, Pairs};
use pest::pratt_parser::PrattParser;
use pest_derive::Parser;

static PRATT_PARSER: Lazy<PrattParser<Rule>> = Lazy::new(|| {
    use pest::pratt_parser::Assoc::*;
    use pest::pratt_parser::Op;
    use Rule::*;

    PrattParser::new()
        .op(Op::infix(union, Left) | Op::infix(difference, Left))
        .op(Op::infix(intersect, Left))
        .op(Op::infix(symmetric_difference, Left))
        .op(Op::prefix(complement))
});

/// A [`pest`] parser for test set expressions.
#[derive(Parser)]
#[grammar = "test_set/grammar.pest"]
pub struct TestSetParser;

/// Parses a given input string into a test set expression.
pub fn parse_test_set_expr(input: &str) -> Result<Expr, Error<Rule>> {
    use pest::Parser;

    Ok(parse_expr(
        TestSetParser::parse(Rule::main, input)?
            .next()
            .expect("main is not optional")
            .into_inner(),
    ))
}

/// A binary operation token.
///
/// Operator precedence is as follows in order of lowest to highest:
/// 1. union & difference
/// 2. symmetric difference
/// 3. intersect
/// 4. complement
///
/// All binary operators are left associative.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    /// A symmetric difference operator (`xor` and `^`, etc.).
    SymmetricDifference,

    /// A set difference operator (`\` and `-`, etc.).
    Difference,

    /// An intersection operator (`and` and `&`, etc.).
    Intersection,

    /// A union operator (`or` and `|`, etc.).
    Union,
}

/// A unary operation token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    /// The prefix complement operator (`!`, etc.), has highest precedence of
    /// all operators.
    Complement,
}

/// An expression token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    /// A unary expression.
    Unary(UnaryExpr),

    /// A binary expression.
    Binary(BinaryExpr),

    /// A bare atom.
    Atom(Atom),
}

/// A unary prefix expresssion token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnaryExpr {
    /// The unary prefix operator.
    pub op: UnaryOp,

    /// The prefixed expression.
    pub expr: Box<Expr>,
}

/// A binary expresssion token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryExpr {
    /// The binary infix operator.
    pub op: BinaryOp,

    /// The left hand side of the binary expression.
    pub lhs: Box<Expr>,

    /// The right hand side of the binary expression.
    pub rhs: Box<Expr>,
}

/// An atom token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Atom {
    /// A value.
    Value(Value),

    /// A function.
    Function(Function),
}

/// An identifier token for test set values and test set functions.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Identifier {
    /// The identifier string, must be a valid identifier.
    pub value: String,
}

/// A value token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Value {
    /// The value identifier.
    pub id: Identifier,
}

/// A function token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    /// The function identifier.
    pub id: Identifier,

    /// The function arguments.
    pub args: Arguments,
}

/// A function arguments token. This currently holds a single argument but my
/// hold more in the future.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arguments {
    /// The single function argument.
    pub arg: Argument,
}

/// A single function argument token. This curently only holds a name matcher
/// but may hold other values in the future.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Argument {
    /// The name matcher.
    pub matcher: NameMatcher,
}

/// A name matcher token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameMatcher {
    /// An exact identifier matcher.
    Exact(String),

    /// A contains identifier matcher.
    Contains(String),

    /// An regex identifier matcher.
    Regex(String),
}

/// Parses an [`Atom`].
///
/// # Panics
/// Panics if the given pair is not of the expected rule, or if the inner pairs
/// don't match the expected count/rules.
pub fn parse_atom(pair: Pair<Rule>) -> Atom {
    if pair.as_rule() != Rule::atom {
        panic!("expected atom, got {:?}: {pair:?}", pair.as_rule());
    }

    let mut pairs = pair.into_inner();
    let atom = pairs.next().expect("atom is not empty");
    assert_eq!(pairs.next(), None, "expected no other pairs in atom");

    match atom.as_rule() {
        Rule::val => Atom::Value(parse_value(atom)),
        Rule::func => Atom::Function(parse_function(atom)),
        _ => unreachable!("atom can only be val or func"),
    }
}

/// Parses a [`Value`].
///
/// # Panics
/// Panics if the given pair is not of the expected rule, or if the inner pairs
/// don't match the expected count/rules.
pub fn parse_value(pair: Pair<Rule>) -> Value {
    if pair.as_rule() != Rule::val {
        panic!("expected val, got {:?}: {pair:?}", pair.as_rule());
    }

    let mut pairs = pair.into_inner();
    let id = pairs.next().expect("val has id");
    assert_eq!(pairs.next(), None, "expected no other pairs after id");
    Value { id: parse_id(id) }
}

/// Parses a [`Function`].
///
/// # Panics
/// Panics if the given pair is not of the expected rule, or if the inner pairs
/// don't match the expected count/rules.
pub fn parse_function(pair: Pair<Rule>) -> Function {
    if pair.as_rule() != Rule::func {
        panic!("expected func, got {:?}: {pair:?}", pair.as_rule());
    }

    let mut pairs = pair.into_inner();
    let id = pairs.next().expect("func has id");
    let args = pairs.next().expect("func has args");
    assert_eq!(
        pairs.next(),
        None,
        "expected no other pairs after id and args"
    );

    let id = match id.as_rule() {
        Rule::id => parse_id(id),
        _ => unreachable!("id can only be id"),
    };

    let args = match args.as_rule() {
        Rule::args => parse_args(args),
        _ => unreachable!("args can only be args"),
    };

    Function { id, args }
}

/// Parses an [`Identifier`].
///
/// # Panics
/// Panics if the given pair is not of the expected rule, or if the inner pairs
/// don't match the expected count/rules.
pub fn parse_id(pair: Pair<Rule>) -> Identifier {
    if pair.as_rule() != Rule::id {
        panic!("expected id, got {:?}: {pair:?}", pair.as_rule());
    }

    let id = pair.as_str().to_owned();
    assert_eq!(pair.into_inner().next(), None, "expected no pairs in id");
    Identifier { value: id }
}

/// Parses [`Arguments`].
///
/// # Panics
/// Panics if the given pair is not of the expected rule, or if the inner pairs
/// don't match the expected count/rules.
pub fn parse_args(pair: Pair<Rule>) -> Arguments {
    if pair.as_rule() != Rule::args {
        panic!("expected args, got {:?}: {pair:?}", pair.as_rule());
    }

    let mut pairs = pair.into_inner();
    let arg = pairs.next().expect("args has arg");
    assert_eq!(pairs.next(), None, "expected no other pairs after arg");

    Arguments {
        arg: parse_arg(arg),
    }
}

/// Parses a single [`Argument`].
///
/// # Panics
/// Panics if the given pair is not of the expected rule, or if the inner pairs
/// don't match the expected count/rules.
pub fn parse_arg(pair: Pair<Rule>) -> Argument {
    if pair.as_rule() != Rule::arg {
        panic!("expected arg, got {:?}: {pair:?}", pair.as_rule());
    }

    let mut pairs = pair.into_inner();
    let matcher = pairs.next().expect("arg has matcher");
    assert_eq!(pairs.next(), None, "expected no other pairs after matcher");
    Argument {
        matcher: parse_matcher(matcher),
    }
}

/// Parses an [`NameMatcher`].
///
/// # Panics
/// Panics if the given pair is not of the expected rule, or if the inner pairs
/// don't match the expected count/rules.
pub fn parse_matcher(pair: Pair<Rule>) -> NameMatcher {
    if pair.as_rule() != Rule::matcher {
        panic!("expected matcher, got {:?}: {pair:?}", pair.as_rule());
    }

    let mut pairs = pair.into_inner();
    let inner = pairs.next().expect("matcher has inner matcher");
    assert_eq!(
        pairs.next(),
        None,
        "expected no other pairs after inner matcher"
    );

    fn extract_name(inner: Pair<Rule>) -> String {
        let mut pairs = inner.into_inner();
        let name = pairs.next().expect("inner matcher has name");
        assert_eq!(pairs.next(), None, "expected no other pairs after name");

        name.as_str().to_owned()
    }

    match inner.as_rule() {
        Rule::exact_matcher => NameMatcher::Exact(extract_name(inner)),
        Rule::contains_matcher => NameMatcher::Contains(extract_name(inner)),
        Rule::regex_matcher => NameMatcher::Regex(extract_name(inner).replace("\\/", "/")),
        _ => unreachable!("inner matcher can only be exact, contains, regex or plain"),
    }
}

/// Parses a top level [`Expr`], taking care of operator precedence. This is a
/// suitable entry point to parse a [`Rule::main`] token pair.
///
/// # Panics
/// Panics if the given pair is not of the expected rule, or if the inner pairs
/// don't match the expected count/rules.
pub fn parse_expr(pairs: Pairs<Rule>) -> Expr {
    PRATT_PARSER
        .map_primary(|primary| match primary.as_rule() {
            Rule::atom => Expr::Atom(parse_atom(primary)),
            Rule::term => parse_expr(primary.into_inner()),
            Rule::expr => parse_expr(primary.into_inner()),
            rule => unreachable!("expected atom, found {rule:?}"),
        })
        .map_infix(|lhs, op, rhs| {
            let operator = match op.as_rule() {
                Rule::symmetric_difference => BinaryOp::SymmetricDifference,
                Rule::intersect => BinaryOp::Intersection,
                Rule::union => BinaryOp::Union,
                Rule::difference => BinaryOp::Difference,
                rule => unreachable!("expected infix operation, found {rule:?}"),
            };
            Expr::Binary(BinaryExpr {
                op: operator,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            })
        })
        .map_prefix(|op, expr| {
            let operator = match op.as_rule() {
                Rule::complement => UnaryOp::Complement,
                rule => unreachable!("expected prefix operations, found {rule:?}"),
            };
            Expr::Unary(UnaryExpr {
                op: operator,
                expr: Box::new(expr),
            })
        })
        .parse(pairs)
}

#[cfg(test)]
mod tests {
    use pest::{consumes_to, Parser};

    use super::*;

    #[test]
    fn test_parse_test_set_expr_empty() {
        assert!(parse_test_set_expr(" \t  ").is_err());
    }

    #[test]
    fn test_parse_test_set_expr() {
        let expr = parse_test_set_expr("  (test(=exact) - func(/ regex/)) & !val ").unwrap();
        assert_eq!(
            expr,
            Expr::Binary(BinaryExpr {
                op: BinaryOp::Intersection,
                lhs: Box::new(Expr::Binary(BinaryExpr {
                    op: BinaryOp::Difference,
                    lhs: Box::new(Expr::Atom(Atom::Function(Function {
                        id: Identifier {
                            value: "test".into()
                        },
                        args: Arguments {
                            arg: Argument {
                                matcher: NameMatcher::Exact("exact".into())
                            }
                        },
                    }))),
                    rhs: Box::new(Expr::Atom(Atom::Function(Function {
                        id: Identifier {
                            value: "func".into()
                        },
                        args: Arguments {
                            arg: Argument {
                                matcher: NameMatcher::Regex(" regex".into())
                            }
                        },
                    }))),
                })),
                rhs: Box::new(Expr::Unary(UnaryExpr {
                    op: UnaryOp::Complement,
                    expr: Box::new(Expr::Atom(Atom::Value(Value {
                        id: Identifier {
                            value: "val".into()
                        }
                    }))),
                })),
            })
        );
    }

    #[test]
    fn test_parser_rule_value() {
        pest::fails_with! {
            parser: TestSetParser,
            input: " val",
            rule: Rule::val,
            positives: [Rule::id],
            negatives: [],
            pos: 0
        };
        pest::parses_to! {
            parser: TestSetParser,
            input: "val",
            rule: Rule::val,
            tokens: [
                val(0, 3, [
                    id(0, 3)
                ])
            ]
        };
        pest::parses_to! {
            parser: TestSetParser,
            input: "val ",
            rule: Rule::val,
            tokens: [
                val(0, 3, [
                    id(0, 3)
                ])
            ]
        };
    }

    #[test]
    fn test_parser_rule_function() {
        pest::fails_with! {
            parser: TestSetParser,
            input: "val( =plain)",
            rule: Rule::func,
            positives: [Rule::matcher],
            negatives: [],
            pos: 4
        };
        pest::fails_with! {
            parser: TestSetParser,
            input: "val (=plain)",
            rule: Rule::func,
            positives: [Rule::args],
            negatives: [],
            pos: 3
        };
        pest::parses_to! {
            parser: TestSetParser,
            input: "val(=plain)",
            rule: Rule::func,
            tokens: [
                func(0, 11, [
                    id(0, 3),
                    args(3, 11, [
                        arg(4, 10, [
                            matcher(4, 10, [
                                exact_matcher(4, 10, [
                                    name(5, 10)
                                ])
                            ])
                        ])
                    ])
                ])
            ]
        }
    }

    #[test]
    fn test_parse_matcher() {
        pest::fails_with! {
            parser: TestSetParser,
            input: "= a",
            rule: Rule::matcher,
            positives: [Rule::name],
            negatives: [],
            pos: 1
        };
        pest::fails_with! {
            parser: TestSetParser,
            input: "//",
            rule: Rule::matcher,
            positives: [Rule::regex],
            negatives: [],
            pos: 1
        };
        pest::parses_to! {
            parser: TestSetParser,
            input: "=a",
            rule: Rule::matcher,
            tokens: [
                matcher(0, 2, [
                    exact_matcher(0, 2, [
                        name(1, 2)
                    ])
                ])
            ]
        }
        pest::parses_to! {
            parser: TestSetParser,
            input: "~a",
            rule: Rule::matcher,
            tokens: [
                matcher(0, 2, [
                    contains_matcher(0, 2, [
                        name(1, 2)
                    ])
                ])
            ]
        }
        pest::parses_to! {
            parser: TestSetParser,
            input: "~a",
            rule: Rule::matcher,
            tokens: [
                matcher(0, 2, [
                    contains_matcher(0, 2, [
                        name(1, 2)
                    ])
                ])
            ]
        }
        pest::parses_to! {
            parser: TestSetParser,
            input: r"/\/a/",
            rule: Rule::matcher,
            tokens: [
                matcher(0, 5, [
                    regex_matcher(0, 5, [
                        regex(1, 4)
                    ])
                ])
            ]
        }
        pest::parses_to! {
            parser: TestSetParser,
            input: "/ a/",
            rule: Rule::matcher,
            tokens: [
                matcher(0, 4, [
                    regex_matcher(0, 4, [
                        regex(1, 3)
                    ])
                ])
            ]
        }

        assert_eq!(
            parse_matcher(
                TestSetParser::parse(Rule::matcher, r"/\/a/")
                    .unwrap()
                    .next()
                    .unwrap()
            ),
            NameMatcher::Regex("/a".into())
        );
    }
}
