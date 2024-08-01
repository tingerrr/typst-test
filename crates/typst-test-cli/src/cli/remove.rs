use std::io;

use serde::Serialize;
use termcolor::{Color, WriteColor};

use super::{Context, OperationArgs};
use crate::report::{Report, Verbosity};
use crate::ui;

#[derive(clap::Args, Debug, Clone)]
#[group(id = "remove-args")]
pub struct Args {
    #[command(flatten)]
    pub op_args: OperationArgs,
}

#[derive(Debug, Serialize)]
pub struct RemoveReport {
    removed: usize,
}

impl Report for RemoveReport {
    fn report<W: WriteColor>(&self, mut writer: W, _verbosity: Verbosity) -> io::Result<()> {
        write!(writer, "Removed ")?;
        ui::write_bold_colored(&mut writer, Color::Green, |w| write!(w, "{}", self.removed))?;
        writeln!(writer, "tests")?;

        Ok(())
    }
}

pub fn run(ctx: &mut Context, args: &Args) -> anyhow::Result<()> {
    let mut project = ctx.collect_tests(&args.op_args, "remove")?;

    let len = project.matched().len();
    project.delete_tests()?;
    ctx.reporter
        .lock()
        .unwrap()
        .report(&RemoveReport { removed: len })?;

    Ok(())
}
