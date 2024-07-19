use super::{Context, OperationArgs};

#[derive(clap::Args, Debug, Clone)]
#[group(id = "edit-args")]
pub struct Args {
    /// The sub command to run
    #[command(subcommand)]
    pub cmd: Command,

    #[command(flatten)]
    pub op_args: OperationArgs,
}

#[derive(clap::Subcommand, Debug, Clone)]
pub enum Command {}

pub fn run(ctx: &mut Context, args: &Args) -> anyhow::Result<()> {
    ctx.collect_tests(&args.op_args, "edit")?;

    // TODO: changing test kind
    todo!();
}
