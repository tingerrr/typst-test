use std::io::Write;

use typst_test_lib::test_set;

use super::Context;

pub fn run(ctx: &mut Context) -> anyhow::Result<()> {
    let mut project = ctx.ensure_project()?;
    project.collect_tests(test_set::builtin::all())?;

    project.clean_artifacts()?;
    writeln!(ctx.reporter.lock().unwrap(), "Removed test artifacts")?;

    Ok(())
}
