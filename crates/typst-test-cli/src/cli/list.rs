use super::{Context, OperationArgs};

#[derive(clap::Args, Debug, Clone)]
#[group(id = "list-args")]
pub struct Args {
    #[command(flatten)]
    pub op_args: OperationArgs,
}

pub fn run(ctx: &mut Context, args: &Args) -> anyhow::Result<()> {
    let project = ctx.collect_tests(&args.op_args, None)?;
    ctx.reporter.lock().unwrap().tests(&project)?;

    Ok(())
}
