use color_eyre::eyre;

use super::Context;

pub mod clean;
pub mod fonts;

#[derive(clap::Args, Debug, Clone)]
#[group(id = "util-args")]
pub struct Args {
    /// The sub command to run
    #[command(subcommand)]
    pub cmd: Command,
}

#[derive(clap::Subcommand, Debug, Clone)]
pub enum Command {
    /// Remove test output artifacts
    #[command()]
    Clean,

    /// List all available fonts
    #[command()]
    Fonts(fonts::Args),
}

impl Command {
    pub fn run(&self, ctx: &mut Context) -> eyre::Result<()> {
        match self {
            Command::Clean => clean::run(ctx),
            Command::Fonts(args) => fonts::run(ctx, args),
        }
    }
}
