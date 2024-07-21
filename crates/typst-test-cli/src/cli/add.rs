use std::io::Write;

use typst_test_lib::test::id::Identifier;
use typst_test_lib::test::ReferenceKind;
use typst_test_lib::test_set;

use super::Context;

#[derive(clap::Args, Debug, Clone)]
#[group(id = "add-args")]
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

pub fn run(ctx: &mut Context, args: &Args) -> anyhow::Result<()> {
    let mut project = ctx.ensure_init()?;
    project.collect_tests(test_set::builtin::all())?;
    project.load_template()?;

    if project.matched().contains_key(&args.test) {
        ctx.operation_failure(|r| writeln!(r, "Test '{}' already exists", args.test))?;
        anyhow::bail!("Test already exists");
    }

    let kind = if args.ephemeral {
        Some(ReferenceKind::Ephemeral)
    } else if args.compile_only {
        None
    } else {
        Some(ReferenceKind::Persistent)
    };

    project.create_test(args.test.clone(), kind, !args.no_template)?;
    let test = &project.matched()[&args.test];
    ctx.reporter.lock().unwrap().test_added(test)?;

    Ok(())
}
