//! Parsing, optimizating and evaluating the test set expression DSL for test
//! matching.

pub mod ast;
pub mod builtin;
pub mod eval;
pub mod id;

pub use ast::{parse, Expr, InfixExpr, InfixOp, PostfixExpr, PostfixOp, PrefixExpr, PrefixOp};
pub use eval::{DynFunction, DynTestSet, Eval, Function, TestSet};
pub use id::Identifier;
