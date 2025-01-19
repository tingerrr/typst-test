use color_eyre::eyre;

use super::Context;

pub mod about;
pub mod clean;
pub mod fonts;
pub mod migrate;

#[derive(clap::Args, Debug, Clone)]
#[group(id = "util-args")]
pub struct Args {
    /// The sub command to run
    #[command(subcommand)]
    pub cmd: Command,
}

#[derive(clap::Subcommand, Debug, Clone)]
pub enum Command {
    /// Print information about this program
    #[command()]
    About,

    /// Remove test output artifacts
    #[command()]
    Clean,

    /// List all available fonts
    #[command()]
    Fonts(fonts::Args),

    /// Migrate the test structure to the new version
    #[command()]
    Migrate(migrate::Args),
}

impl Command {
    pub fn run(&self, ctx: &mut Context) -> eyre::Result<()> {
        match self {
            Command::About => about::run(ctx),
            Command::Clean => clean::run(ctx),
            Command::Fonts(args) => fonts::run(ctx, args),
            Command::Migrate(args) => migrate::run(ctx, args),
        }
    }
}
