use super::{run, CliResult, Context, Global, MutationArgs};

#[derive(clap::Args, Debug, Clone)]
pub struct Args {
    #[command(flatten)]
    pub run: run::Args,

    #[command(flatten)]
    pub mutation: MutationArgs,
}

pub fn run(ctx: Context, global: &Global, args: &Args) -> anyhow::Result<CliResult> {
    // TODO: all confirmation
    run::run(ctx, global, &args.run, |ctx| ctx.with_update(true))
}
