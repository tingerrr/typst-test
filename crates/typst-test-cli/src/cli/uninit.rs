use std::fmt::Write;

use super::{CliResult, Context, Global};
use crate::util;

pub fn run(ctx: Context, global: &Global) -> anyhow::Result<CliResult> {
    let matcher = global.matcher.matcher();
    ctx.project.collect_tests(matcher)?;
    let count = ctx.project.matched().len();

    ctx.project.uninit()?;
    writeln!(
        ctx.reporter,
        "Removed {} {}",
        count,
        util::fmt::plural(count, "test"),
    )?;

    Ok(CliResult::Ok)
}
