use super::{Context, OperationArgs};

#[derive(clap::Args, Debug, Clone)]
#[group(id = "remove-args")]
pub struct Args {
    #[command(flatten)]
    pub op_args: OperationArgs,
}

pub fn run(ctx: &mut Context, args: &Args) -> anyhow::Result<()> {
    let mut project = ctx.collect_tests(&args.op_args, "remove")?;

    project.delete_tests()?;
    ctx.reporter
        .lock()
        .unwrap()
        .tests_success(&project, "removed")?;

    Ok(())
}
