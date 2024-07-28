use std::io::Write;

use super::Context;

#[derive(clap::Parser, Debug, Clone)]
#[group(id = "init-args")]
pub struct Args {
    /// Do not create a default example test
    #[arg(long)]
    no_example: bool,

    /// Which VCS to use for ignoring files
    #[arg(long, default_value = "git")]
    vcs: Vcs,
}

#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vcs {
    /// The git VCS.
    Git,

    /// No VCS.
    None,
}

pub fn run(ctx: &mut Context, args: &Args) -> anyhow::Result<()> {
    let mut project = ctx.ensure_project()?;

    if project.is_init()? {
        ctx.operation_failure(|r| {
            writeln!(r, "Project '{}' was already initialized", project.name(),)
        })?;
        anyhow::bail!("Project was already initalized");
    }

    project.init(args.no_example, args.vcs)?;
    writeln!(
        ctx.reporter.lock().unwrap(),
        "Initialized project '{}'",
        project.name()
    )?;

    Ok(())
}
