use typst_test_lib::test_set;

use super::Context;

pub fn run(ctx: &mut Context) -> anyhow::Result<()> {
    let mut project = ctx.ensure_init()?;
    project.collect_tests(test_set::builtin::all())?;
    project.load_template()?;

    ctx.reporter.lock().unwrap().project(&project)?;

    Ok(())
}
