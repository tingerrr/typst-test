use serde::Serialize;
use termcolor::{Color, WriteColor};
use typst_test_lib::test_set;
use typst_test_stdx::fmt::Term;

use super::Context;
use crate::report::{Report, Verbosity};
use crate::ui;

#[derive(Debug, Serialize)]
pub struct CleanReport {
    removed: usize,
}

impl Report for CleanReport {
    fn report<W: WriteColor>(&self, mut writer: W, _verbosity: Verbosity) -> anyhow::Result<()> {
        write!(writer, "Removed test artifacts for")?;
        ui::write_colored(&mut writer, Color::Green, |w| write!(w, "{}", self.removed))?;
        writeln!(writer, " {}", Term::simple("test").with(self.removed))?;

        Ok(())
    }
}

pub fn run(ctx: &mut Context) -> anyhow::Result<()> {
    let mut project = ctx.ensure_project()?;
    project.collect_tests(test_set::builtin::all())?;

    let len = project.matched().len();

    project.clean_artifacts()?;
    ctx.reporter.report(&CleanReport { removed: len })?;

    Ok(())
}
