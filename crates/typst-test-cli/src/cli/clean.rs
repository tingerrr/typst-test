use std::fmt::Write;

use super::{Context, Global};
use crate::cli::{bail_if_uninit, CliResult};

pub fn run(ctx: Context, global: &Global) -> anyhow::Result<CliResult> {
    bail_if_uninit!(ctx);

    let matcher = global.matcher.matcher();
    ctx.project.collect_tests(matcher)?;

    ctx.project.clean_artifacts()?;
    writeln!(ctx.reporter, "Removed test artifacts")?;

    Ok(CliResult::Ok)
}
