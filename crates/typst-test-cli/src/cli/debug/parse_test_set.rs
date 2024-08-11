use std::io::Write;

use typst_test_lib::test_set::{self, eval, Eval};

use crate::cli::Context;

#[derive(clap::Args, Debug, Clone)]
#[group(id = "parse-test-set-args")]
pub struct Args {
    /// The test set expression to parse
    #[arg()]
    pub expression: String,
}

pub fn run(ctx: &mut Context, args: &Args) -> anyhow::Result<()> {
    let mut stderr = ctx.reporter.ui().stderr();

    writeln!(stderr, "expr: {:#?}", args.expression)?;

    let expr = test_set::parse(&args.expression);
    writeln!(stderr, "parsed: {:#?}", expr)?;

    let Ok(expr) = expr else {
        return Ok(());
    };

    let flattened = test_set::ast::flatten(expr);
    writeln!(stderr, "flattened: {flattened:#?}")?;

    let eval = flattened.eval(&eval::Context::builtin());
    writeln!(stderr, "eval: {eval:#?}")?;

    Ok(())
}
