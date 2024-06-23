use super::{CliResult, Context, Global, MutationArgs};
use crate::cli::bail_if_uninit;

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
                "Pass `--all` to edit more than one test at a time",
            ))
        }
    }

    // TODO: changing test kind
    todo!();

    Ok(CliResult::Ok)
}
