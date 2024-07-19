use std::io::Write;

use super::Context;
use crate::project::ScaffoldOptions;

#[derive(clap::Parser, Debug, Clone)]
#[group(id = "init-args")]
pub struct Args {
    /// Do not create a default example test
    #[arg(long)]
    no_example: bool,
}

pub fn run(ctx: &mut Context, args: &Args) -> anyhow::Result<()> {
    let mut project = ctx.ensure_project()?;

    if project.is_init()? {
        ctx.operation_failure(|r| {
            writeln!(r, "Project '{}' was already initialized", project.name(),)
        })?;
        anyhow::bail!("");
    }

    let mut options = ScaffoldOptions::empty();
    options.set(ScaffoldOptions::EXAMPLE, !args.no_example);

    project.init(options)?;
    writeln!(
        ctx.reporter.lock().unwrap(),
        "Initialized project '{}'",
        project.name()
    )?;

    Ok(())
}
