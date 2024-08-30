use std::fmt::Display;
use std::io;
use std::io::Write;

use serde::Serialize;
use termcolor::{Color, WriteColor};
use thiserror::Error;

use super::Context;
use crate::error::{Failure, OperationFailure};
use crate::report::reports::ProjectJson;
use crate::report::{Report, Verbosity};
use crate::ui;
use crate::ui::Ui;

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

#[derive(Debug, Error)]
pub struct ProjectAlreadyIntialized(Option<String>);

impl Display for ProjectAlreadyIntialized {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0.as_deref() {
            Some(name) => write!(f, "Package '{name}' was already initialized"),
            None => write!(f, "Project was already initialized"),
        }
    }
}

impl Failure for ProjectAlreadyIntialized {
    fn report(&self, ui: &Ui) -> io::Result<()> {
        ui.error_with(|w| {
            if let Some(name) = self.0.as_deref() {
                write!(w, "Package ")?;
                ui::write_colored(w, Color::Cyan, |w| write!(w, "{name}"))?
            } else {
                write!(w, "Project ")?;
            }
            writeln!(w, " was already initialized")
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct InitReport<'p>(ProjectJson<'p>);

impl Report for InitReport<'_> {
    fn report<W: WriteColor>(&self, mut writer: W, _verbosity: Verbosity) -> anyhow::Result<()> {
        write!(writer, "Initialized ")?;
        if let Some(package) = &self.0.package {
            write!(writer, "package ")?;
            ui::write_colored(&mut writer, Color::Cyan, |w| {
                writeln!(w, "{}", package.name)
            })?
        } else {
            writeln!(writer, "project")?;
        }

        Ok(())
    }
}

pub fn run(ctx: &mut Context, args: &Args) -> anyhow::Result<()> {
    let mut project = ctx.ensure_project()?;

    if project.is_init()? {
        anyhow::bail!(OperationFailure::from(ProjectAlreadyIntialized(
            project.manifest().map(|m| m.package.name.to_owned())
        )));
    }

    project.init(args.no_example, args.vcs)?;

    ctx.reporter
        .report(&InitReport(ProjectJson::new(&project)))?;

    Ok(())
}
