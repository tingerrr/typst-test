use typst_test_lib::test::id::Identifier;
use typst_test_lib::test::ReferenceKind;

use super::{CliResult, Context, Global};
use crate::cli::bail_if_uninit;

#[derive(clap::Args, Debug, Clone)]
pub struct Args {
    /// Whether this test creates it's references on the fly
    ///
    /// An ephemeral test consistes of two scripts which are compared
    /// against each other. The reference script must be called `ref.typ`.
    #[arg(long, short)]
    pub ephemeral: bool,

    /// Whether this test has no references at all
    #[arg(long, short, conflicts_with = "ephemeral")]
    pub compile_only: bool,

    /// Ignore the test template for this test
    #[arg(long)]
    pub no_template: bool,

    /// The name of the test to add
    pub test: Identifier,
}

pub fn run(ctx: Context, global: &Global, args: &Args) -> anyhow::Result<CliResult> {
    bail_if_uninit!(ctx);

    let matcher = global.matcher.matcher();
    ctx.project.collect_tests(matcher)?;
    ctx.project.load_template()?;

    if ctx.project.matched().contains_key(&args.test) {
        return Ok(CliResult::operation_failure(format!(
            "Test '{}' already exists",
            args.test,
        )));
    }

    let kind = if args.ephemeral {
        Some(ReferenceKind::Ephemeral)
    } else if args.compile_only {
        None
    } else {
        Some(ReferenceKind::Persistent)
    };

    ctx.project
        .create_test(args.test.clone(), kind, !args.no_template)?;
    let test = &ctx.project.matched()[&args.test];
    ctx.reporter.test_added(test)?;

    Ok(CliResult::Ok)
}
