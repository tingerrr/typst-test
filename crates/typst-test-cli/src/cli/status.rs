use serde::Serialize;
use termcolor::{Color, WriteColor};
use typst_test_lib::test_set;

use super::Context;
use crate::report::reports::ProjectJson;
use crate::report::{Report, Verbosity};
use crate::ui;

#[derive(Debug, Serialize)]
pub struct StatusReport<'p> {
    #[serde(flatten)]
    inner: ProjectJson<'p>,
}

impl Report for StatusReport<'_> {
    fn report<W: WriteColor>(&self, mut writer: W, _verbosity: Verbosity) -> anyhow::Result<()> {
        struct Delims {
            open: &'static str,
            middle: &'static str,
            close: &'static str,
        }

        let delims = Delims {
            open: " ┌ ",
            middle: " ├ ",
            close: " └ ",
        };

        let align = ["Template", "Project", "Tests"]
            .map(str::len)
            .into_iter()
            .max()
            .unwrap();

        if let Some(package) = &self.inner.package {
            write!(writer, "{:>align$}{}", "Project", delims.open)?;
            ui::write_bold_colored(&mut writer, Color::Cyan, |w| write!(w, "{}", &package.name))?;
            write!(writer, ":")?;
            ui::write_bold_colored(&mut writer, Color::Cyan, |w| {
                write!(w, "{}", &package.version)
            })?;
            writeln!(writer)?;
        } else {
            write!(writer, "{:>align$}{}", "Project", delims.open)?;
            ui::write_bold_colored(&mut writer, Color::Yellow, |w| write!(w, "none"))?;
            writeln!(writer)?;
        }

        if let Some(vcs) = &self.inner.vcs {
            write!(writer, "{:>align$}{}", "Vcs", delims.middle)?;
            ui::write_bold_colored(&mut writer, Color::Green, |w| write!(w, "{vcs}"))?;
            writeln!(writer)?;
        } else {
            write!(writer, "{:>align$}{}", "Vcs", delims.middle)?;
            ui::write_bold_colored(&mut writer, Color::Yellow, |w| write!(w, "none"))?;
            writeln!(writer)?;
        }

        if self.inner.tests.is_empty() {
            write!(writer, "{:>align$}{}", "Tests", delims.middle)?;
            ui::write_bold_colored(&mut writer, Color::Cyan, |w| writeln!(w, "none"))?;
        } else {
            let mut persistent = 0;
            let mut ephemeral = 0;
            let mut compile_only = 0;

            for test in &self.inner.tests {
                match test.kind {
                    "persistent" => persistent += 1,
                    "ephemeral" => ephemeral += 1,
                    "compile-only" => compile_only += 1,
                    k => unreachable!("unknown kind: {k}"),
                }
            }

            write!(writer, "{:>align$}{}", "Tests", delims.middle)?;
            ui::write_bold_colored(&mut writer, Color::Green, |w| write!(w, "{persistent}"))?;
            writeln!(writer, " persistent")?;

            write!(writer, "{:>align$}{}", "", delims.middle)?;
            ui::write_bold_colored(&mut writer, Color::Green, |w| write!(w, "{ephemeral}"))?;
            writeln!(writer, " ephemeral")?;

            write!(writer, "{:>align$}{}", "", delims.middle)?;
            ui::write_bold_colored(&mut writer, Color::Yellow, |w| write!(w, "{compile_only}"))?;
            writeln!(writer, " compile-only")?;
        }

        write!(writer, "{:>align$}{}", "Template", delims.close)?;
        if let Some(path) = &self.inner.template_path {
            ui::write_bold_colored(&mut writer, Color::Cyan, |w| write!(w, "{path}"))?;
        } else {
            ui::write_bold_colored(&mut writer, Color::Green, |w| write!(w, "none"))?;
        }
        writeln!(writer)?;

        Ok(())
    }
}

pub fn run(ctx: &mut Context) -> anyhow::Result<()> {
    let mut project = ctx.ensure_init()?;
    project.collect_tests(test_set::builtin::all())?;
    project.load_template()?;

    ctx.reporter.report(&StatusReport {
        inner: ProjectJson::new(&project),
    })?;

    Ok(())
}
