use std::io::Write;

use serde::Serialize;
use termcolor::{Color, WriteColor};
use typst_test_lib::test_set;
use typst_test_stdx::fmt::Term;

use super::Context;
use crate::report::reports::ProjectJson;
use crate::report::{Report, Verbosity};
use crate::ui;

#[derive(clap::Args, Debug, Clone)]
#[group(id = "uninit-args")]
pub struct Args {
    /// Whether or not to skip confirmation
    #[arg(long, short)]
    pub force: bool,
}

#[derive(Debug, Serialize)]
pub struct InitReport<'p> {
    #[serde(flatten)]
    inner: ProjectJson<'p>,
}

impl Report for InitReport<'_> {
    fn report<W: WriteColor>(&self, mut writer: W, _verbosity: Verbosity) -> anyhow::Result<()> {
        write!(writer, "Uninitalized project ")?;
        let (color, name) = match &self.inner.package {
            Some(package) => (Color::Cyan, package.name),
            None => (Color::Yellow, "<unnamed>"),
        };
        ui::write_colored(&mut writer, color, |w| write!(w, "{}", name))?;
        let count = self.inner.tests.len();
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
        ctx.operation_failure(|r| r.ui().error_with(|w| writeln!(w, "Deletion aborted")))?;
        anyhow::bail!("Deletion aborted");
    }

    project.uninit()?;

    ctx.reporter.report(&InitReport {
        inner: ProjectJson::new(&project),
    })?;

    Ok(())
}
