use std::fmt::Write;

use super::{CliResult, Context, Global};
use crate::project::ScaffoldOptions;

#[derive(clap::Parser, Debug, Clone)]
pub struct Args {
    /// Do not create a default example test
    #[arg(long)]
    no_example: bool,
}

pub fn run(ctx: Context, _global: &Global, args: &Args) -> anyhow::Result<CliResult> {
    if ctx.project.is_init()? {
        return Ok(CliResult::operation_failure(format!(
            "Project '{}' was already initialized",
            ctx.project.name(),
        )));
    }

    let mut options = ScaffoldOptions::empty();
    options.set(ScaffoldOptions::EXAMPLE, !args.no_example);

    ctx.project.init(options)?;
    writeln!(ctx.reporter, "Initialized project '{}'", ctx.project.name())?;

    Ok(CliResult::Ok)
}
