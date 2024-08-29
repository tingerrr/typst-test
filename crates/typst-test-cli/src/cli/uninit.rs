use std::io;

use serde::Serialize;
use termcolor::{Color, WriteColor};
use thiserror::Error;
use typst_test_lib::test_set;
use typst_test_stdx::fmt::Term;

use super::Context;
use crate::error::{Failure, OperationFailure};
use crate::report::reports::ProjectJson;
use crate::report::{Report, Verbosity};
use crate::ui;
use crate::ui::Ui;

#[derive(clap::Args, Debug, Clone)]
#[group(id = "uninit-args")]
pub struct Args {
    /// Whether or not to skip confirmation
    #[arg(long, short)]
    pub force: bool,
}

#[derive(Debug, Error)]
#[error("deleltion aborted by user")]
pub struct DeletionAborted;

impl Failure for DeletionAborted {
    fn report(&self, ui: &Ui) -> io::Result<()> {
        ui.error("Deletion aborted")
    }
}

#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct InitReport<'p>(ProjectJson<'p>);

impl Report for InitReport<'_> {
    fn report<W: WriteColor>(&self, mut writer: W, _verbosity: Verbosity) -> anyhow::Result<()> {
        write!(writer, "Uninitalized ")?;
        if let Some(package) = &self.0.package {
            write!(writer, "package ")?;
            ui::write_colored(&mut writer, Color::Cyan, |w| write!(w, "{}", package.name))?
        } else {
            write!(writer, "project")?;
        }
        let count = self.0.tests.len();
        writeln!(
            writer,
            ", removed {} {}",
            count,
            Term::simple("test").with(count),
        )?;

        Ok(())
    }
}

pub fn run(ctx: &mut Context, args: &Args) -> anyhow::Result<()> {
    let mut project = ctx.ensure_init()?;
    project.collect_tests(test_set::builtin::all())?;

    let len = project.matched().len() + project.filtered().len();

    let confirmed = args.force
        || ctx.reporter.ui().prompt_yes_no(
            format!(
                "confirm deletion of {len} {}",
                Term::simple("test").with(len)
            ),
            false,
        )?;

    if !confirmed {
        anyhow::bail!(OperationFailure::from(DeletionAborted));
    }

    project.uninit()?;

    ctx.reporter
        .report(&InitReport(ProjectJson::new(&project)))?;

    Ok(())
}
