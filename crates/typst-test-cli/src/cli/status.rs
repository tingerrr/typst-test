use typst_test_lib::test_set::eval::AllMatcher;

use super::Context;

pub fn run(ctx: &mut Context) -> anyhow::Result<()> {
    let mut project = ctx.ensure_init()?;
    project.collect_tests(AllMatcher)?;
    project.load_template()?;

    ctx.reporter.lock().unwrap().project(&project)?;

    Ok(())
}
