use std::io::Write;

use serde::Serialize;
use termcolor::{Color, WriteColor};

use super::Context;
use crate::report::reports::ProjectJson;
use crate::report::{Report, Verbosity};
use crate::ui;

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

#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct InitReport<'p>(ProjectJson<'p>);

impl Report for InitReport<'_> {
    fn report<W: WriteColor>(&self, mut writer: W, _verbosity: Verbosity) -> anyhow::Result<()> {
        write!(writer, "Initialized project ")?;
        let (color, name) = match &self.0.package {
            Some(package) => (Color::Cyan, package.name),
            None => (Color::Yellow, "<unnamed>"),
        };
        ui::write_colored(&mut writer, color, |w| write!(w, "{}", name))?;
        writeln!(writer)?;

        Ok(())
    }
}

pub fn run(ctx: &mut Context, args: &Args) -> anyhow::Result<()> {
    let mut project = ctx.ensure_project()?;

    if project.is_init()? {
        ctx.operation_failure(|r| {
            r.ui().error_with(|w| {
                writeln!(w, "Project ")?;
                ui::write_colored(w, Color::Cyan, |w| write!(w, "{}", project.name()))?;
                writeln!(w, " was already initialized")
            })
        })?;
        anyhow::bail!("Project was already initalized");
    }

    project.init(args.no_example, args.vcs)?;

    ctx.reporter
        .report(&InitReport(ProjectJson::new(&project)))?;

    Ok(())
}
