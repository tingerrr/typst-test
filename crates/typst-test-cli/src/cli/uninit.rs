use std::io;

use serde::Serialize;
use termcolor::{Color, WriteColor};
use typst_test_lib::test_set;
use typst_test_stdx::fmt::Term;

use super::Context;
use crate::report::reports::ProjectJson;
use crate::report::{Report, Verbosity};
use crate::ui;

#[derive(Debug, Serialize)]
pub struct InitReport<'p> {
    #[serde(flatten)]
    inner: ProjectJson<'p>,
}

impl Report for InitReport<'_> {
    fn report<W: WriteColor>(&self, mut writer: W, _verbosity: Verbosity) -> io::Result<()> {
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

pub fn run(ctx: &mut Context) -> anyhow::Result<()> {
    let mut project = ctx.ensure_project()?;
    project.collect_tests(test_set::builtin::all())?;

    // TODO: confirmation?

    project.uninit()?;

    ctx.reporter.lock().unwrap().report(&InitReport {
        inner: ProjectJson::new(&project),
    })?;

    Ok(())
}
