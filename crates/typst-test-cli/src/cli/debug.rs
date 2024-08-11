use super::Context;

pub mod parse_test_set;

#[derive(clap::Args, Debug, Clone)]
#[group(id = "debug-args")]
pub struct Args {
    /// The sub command to run
    #[command(subcommand)]
    pub cmd: Command,
}

#[derive(clap::Subcommand, Debug, Clone)]
pub enum Command {
    /// Parse, optimize and display test set expressions
    #[command()]
    ParseTestSet(parse_test_set::Args),
}

impl Command {
    pub fn run(&self, ctx: &mut Context) -> anyhow::Result<()> {
        match self {
            Command::ParseTestSet(args) => parse_test_set::run(ctx, args),
        }
    }
}
