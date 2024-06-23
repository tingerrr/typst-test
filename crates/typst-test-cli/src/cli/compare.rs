use typst_test_lib::compare;

use super::{run, CliResult, Context, Global};

pub fn run(ctx: Context, global: &Global, args: &run::Args) -> anyhow::Result<CliResult> {
    run::run(ctx, global, args, |ctx| {
        ctx.with_compare_strategy(Some(compare::Strategy::default()))
    })
}
