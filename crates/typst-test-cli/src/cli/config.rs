use std::collections::BTreeMap;
use std::io;
use std::io::Write;

use serde::Serialize;
use termcolor::{Color, WriteColor};
use thiserror::Error;
use typst_test_lib::config::Config;
use typst_test_stdx::fmt::Separators;

use super::Context;
use crate::error::Failure;
use crate::report::{Report, Verbosity};
use crate::ui;
use crate::ui::{Indented, Ui};

pub mod default;
pub mod get;
pub mod list;
pub mod set;

#[derive(clap::Args, Debug, Clone)]
#[group(id = "config-args")]
pub struct Args {
    /// The sub command to run
    #[command(subcommand)]
    pub cmd: Command,
}

#[derive(clap::Subcommand, Debug, Clone)]
pub enum Command {
    /// Get a single config value
    #[command()]
    Get(get::Args),

    /// Set a single config value
    #[command()]
    Set(set::Args),

    /// List the full config
    #[command()]
    List,

    /// Show the default config
    #[command()]
    Default(default::Args),
}

impl Command {
    pub fn run(&self, ctx: &mut Context) -> anyhow::Result<()> {
        match self {
            Command::Get(args) => get::run(ctx, args),
            Command::Set(args) => set::run(ctx, args),
            Command::List => list::run(ctx),
            Command::Default(args) => default::run(ctx, args),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ConfigJson<'c> {
    Pretty {
        #[serde(flatten)]
        inner: BTreeMap<&'c str, &'c Option<String>>,
    },
    Toml {
        #[serde(flatten)]
        inner: &'c Config,
    },
}

impl Report for ConfigJson<'_> {
    fn report<W: WriteColor>(&self, mut writer: W, verbosity: Verbosity) -> anyhow::Result<()> {
        match self {
            ConfigJson::Pretty { inner } => {
                ui::write_bold(&mut writer, |w| writeln!(w, "Config"))?;

                let writer = &mut Indented::new(writer, 2);
                let pad = Ord::min(
                    inner
                        .iter()
                        .map(|(&k, _)| k.len())
                        .max()
                        .unwrap_or(usize::MAX),
                    50,
                );

                for (&k, &v) in inner {
                    if verbosity <= Verbosity::Less && v.is_none() {
                        continue;
                    }

                    ui::write_colored(writer, Color::Cyan, |w| write!(w, "{:<pad$}", k))?;
                    write!(writer, " = ")?;
                    if let Some(v) = v {
                        ui::write_colored(writer, Color::Green, |w| writeln!(w, "{:?}", v))?;
                    } else {
                        ui::write_colored(writer, Color::Magenta, |w| writeln!(w, "null"))?;
                    }
                }
            }
            ConfigJson::Toml { inner } => {
                let mut doc = toml_edit::DocumentMut::new();
                inner.write_into(&mut doc)?;
                // NOTE: DocumentMut::fmt writes a newline by itself
                write!(&mut writer, "{doc}")?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Error)]
#[error("unknown config keys {0:?}")]
pub struct UnknownConfigKeys(Vec<String>);

impl Failure for UnknownConfigKeys {
    fn report(&self, ui: &Ui) -> io::Result<()> {
        ui.error_with(|w| {
            writeln!(
                w,
                "Unknown config keys: {}",
                Separators::comma_and().with(self.0.iter())
            )
        })
    }
}
