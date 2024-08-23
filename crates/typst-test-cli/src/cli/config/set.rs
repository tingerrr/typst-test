use serde::Serialize;
use termcolor::{Color, WriteColor};

use crate::cli::config::UnknownConfigKeys;
use crate::cli::Context;
use crate::error::OperationFailure;
use crate::report::{Report, Verbosity};
use crate::ui;

#[derive(clap::Args, Debug, Clone)]
#[group(id = "config-set-args")]
pub struct Args {
    /// The key to set
    #[arg()]
    key: String,

    /// The value to set the key to or nothing to unset it
    #[arg()]
    value: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SetReport<'c> {
    key: &'c str,
    old: Option<&'c str>,
    new: Option<&'c str>,
}

impl Report for SetReport<'_> {
    fn report<W: WriteColor>(&self, mut writer: W, _verbosity: Verbosity) -> anyhow::Result<()> {
        write!(writer, "Set ")?;
        ui::write_colored(&mut writer, Color::Cyan, |w| write!(w, "{}", self.key))?;
        write!(writer, " from ")?;
        if let Some(old) = self.old {
            ui::write_colored(&mut writer, Color::Green, |w| write!(w, "{:?}", old))?;
        } else {
            ui::write_colored(&mut writer, Color::Magenta, |w| write!(w, "null"))?;
        }
        write!(writer, " to ")?;
        if let Some(new) = self.new {
            ui::write_colored(&mut writer, Color::Green, |w| writeln!(w, "{:?}", new))?;
        } else {
            ui::write_colored(&mut writer, Color::Magenta, |w| writeln!(w, "null"))?;
        }

        Ok(())
    }
}

pub fn run(ctx: &mut Context, args: &Args) -> anyhow::Result<()> {
    let mut project = ctx.ensure_project()?;

    // TODO: validation
    let Ok(val) = project.config_mut().get_mut(&args.key) else {
        anyhow::bail!(OperationFailure::from(UnknownConfigKeys(vec![args
            .key
            .clone()])));
    };

    let mut old = args.value.clone();
    std::mem::swap(val, &mut old);

    project.write_config()?;

    ctx.reporter.report(&SetReport {
        key: &args.key,
        old: old.as_deref(),
        new: args.value.as_deref(),
    })?;
    Ok(())
}
