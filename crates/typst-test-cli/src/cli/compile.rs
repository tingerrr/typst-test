use super::{run, Context};

#[derive(clap::Args, Debug, Clone)]
#[group(id = "compile-args")]
pub struct Args {
    #[command(flatten)]
    pub run_args: run::Args,
}

pub fn run(ctx: &mut Context, args: &Args) -> anyhow::Result<()> {
    let project = ctx.collect_tests(&args.run_args.op_args, None)?;

    run::run(ctx, project, &args.run_args, |ctx| {
        ctx.with_compile(true)
            .with_compare(false)
            .with_update(false)
    })
}
