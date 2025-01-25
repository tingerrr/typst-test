use std::io::Write;

use color_eyre::eyre;
use lib::project::Project;
use lib::stdx;
use lib::test::{Id, Test};
use termcolor::Color;

use super::Context;
use crate::cli::OperationFailure;
use crate::ui;

#[derive(clap::Parser, Debug, Clone)]
#[group(id = "init-args")]
pub struct Args {
    /// Do not create a default example test
    #[arg(long)]
    pub no_example: bool,
}

pub fn run(ctx: &mut Context, args: &Args) -> eyre::Result<()> {
    let root = ctx.root()?;

    // NOTE(tinger): we use Project discover here because it has the benefit of
    // also checking for the vcs
    let Some(project) = Project::discover(root, true)? else {
        eyre::bail!(OperationFailure);
    };

    if project.paths().test_root().try_exists()? {
        ctx.error_project_already_initialized(project.manifest().map(|m| m.package.name.as_str()))?;
        eyre::bail!(OperationFailure);
    }

    stdx::fs::create_dir(project.paths().test_root(), false)?;

    if !args.no_example {
        Test::create_default(
            project.paths(),
            Id::new("example").expect("the id is valid and unique"),
        )?;
    }

    let mut w = ctx.ui.stderr();

    write!(w, "Initialized ")?;
    if let Some(package) = project.manifest_package_info() {
        write!(w, "package ")?;
        ui::write_colored(&mut w, Color::Cyan, |w| writeln!(w, "{}", package.name))?
    } else {
        writeln!(w, "project")?;
    }

    Ok(())
}
