use super::Context;

pub mod clean;
pub mod export;
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

    /// Compile and export tests and references
    #[command()]
    Export(export::Args),

    /// List all available fonts
    #[command()]
    Fonts(fonts::Args),
}

impl Command {
    pub fn run(&self, ctx: &mut Context) -> anyhow::Result<()> {
        match self {
            Command::Clean => clean::run(ctx),
            Command::Export(args) => export::run(ctx, args),
            Command::Fonts(args) => fonts::run(ctx, args),
        }
    }
}
