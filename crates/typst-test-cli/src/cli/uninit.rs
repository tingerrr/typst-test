use std::io::Write;

use typst_test_lib::test_set;
use typst_test_lib::util;

use super::Context;

pub fn run(ctx: &mut Context) -> anyhow::Result<()> {
    let mut project = ctx.ensure_project()?;
    project.collect_tests(test_set::builtin::all())?;
    let count = project.matched().len();

    // TODO: confirmation?

    project.uninit()?;
    writeln!(
        ctx.reporter.lock().unwrap(),
        "Removed {} {}",
        count,
        util::fmt::plural(count, "test"),
    )?;

    Ok(())
}
