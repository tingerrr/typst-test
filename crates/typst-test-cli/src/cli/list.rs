use std::io;
use std::io::Write;

use serde::Serialize;
use termcolor::{Color, WriteColor};

use super::{Context, OperationArgs};
use crate::report::reports::TestJson;
use crate::report::{Report, Verbosity};
use crate::ui;
use crate::ui::Heading;

#[derive(clap::Args, Debug, Clone)]
#[group(id = "list-args")]
pub struct Args {
    #[command(flatten)]
    pub op_args: OperationArgs,
}

#[derive(Debug, Serialize)]
pub struct ListReport<'p> {
    tests: Vec<TestJson<'p>>,
}

impl Report for ListReport<'_> {
    fn report<W: WriteColor>(&self, writer: W, _verbosity: Verbosity) -> io::Result<()> {
        let mut w = Heading::new(writer, "Tests");

        // NOTE: max pading of 50 should be enough for most cases
        let pad = Ord::min(
            self.tests
                .iter()
                .map(|test| test.id.len())
                .max()
                .unwrap_or(usize::MAX),
            50,
        );

        for test in &self.tests {
            write!(w, "{: <pad$} ", test.id)?;
            let color = match test.kind {
                "ephemeral" => Color::Yellow,
                "persistent" => Color::Green,
                "compile-only" => Color::Yellow,
                k => unreachable!("unknown kind: {k}"),
            };
            ui::write_bold_colored(&mut w, color, |w| write!(w, "{}", test.kind))?;
            writeln!(w)?;
        }

        Ok(())
    }
}

pub fn run(ctx: &mut Context, args: &Args) -> anyhow::Result<()> {
    let project = ctx.collect_tests(&args.op_args, None)?;
    ctx.reporter.report(&ListReport {
        tests: project.matched().values().map(TestJson::new).collect(),
    })?;

    Ok(())
}
