use super::{Context, Global, MutationArgs};
use crate::cli::{bail_if_uninit, CliResult};

#[derive(clap::Args, Debug, Clone)]
pub struct Args {
    #[command(flatten)]
    pub mutation: MutationArgs,
}

pub fn run(ctx: Context, global: &Global, args: &Args) -> anyhow::Result<CliResult> {
    bail_if_uninit!(ctx);

    let matcher = global.matcher.matcher();
    ctx.project.collect_tests(matcher)?;

    match ctx.project.matched().len() {
        0 => return Ok(CliResult::operation_failure("Matched no tests")),
        1 => {}
        _ if args.mutation.all => {}
        _ => {
            return Ok(CliResult::hinted_operation_failure(
                "Matched more than one test",
                "Pass `--all` to remove more than one test at a time",
            ))
        }
    }

    ctx.project.delete_tests()?;
    ctx.reporter.tests_success(&ctx.project, "removed")?;

    Ok(CliResult::Ok)
}
