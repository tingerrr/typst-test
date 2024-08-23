use super::ConfigJson;
use crate::cli::Context;

pub fn run(ctx: &mut Context) -> anyhow::Result<()> {
    let project = ctx.ensure_project()?;

    ctx.reporter.report(&ConfigJson::Pretty {
        inner: project.config().pairs().collect(),
    })?;

    Ok(())
}
