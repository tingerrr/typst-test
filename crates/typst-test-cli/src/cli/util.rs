use super::{CliResult, Context, Global};

mod fonts;

#[derive(clap::Args, Debug, Clone)]
pub struct Args {
    #[command(subcommand)]
    pub cmd: Command,
}

#[derive(clap::Subcommand, Debug, Clone)]
pub enum Command {
    /// List all available fonts
    Fonts(fonts::Args),
}

impl Command {
    pub fn run(&self, ctx: Context, global: &Global) -> anyhow::Result<CliResult> {
        match self {
            Command::Fonts(args) => fonts::run(ctx, global, args),
        }
    }
}
