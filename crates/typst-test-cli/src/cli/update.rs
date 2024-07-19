use super::{run, Context};

#[derive(clap::Args, Debug, Clone)]
#[group(id = "update-args")]
pub struct Args {
    #[command(flatten)]
    pub run_args: run::Args,
}

pub fn run(ctx: &mut Context, args: &Args) -> anyhow::Result<()> {
    let project = ctx.collect_tests(&args.run_args.op_args, "update")?;

    run::run(ctx, project, &args.run_args, |ctx| {
        ctx.with_compile(true).with_update(true)
    })
}
