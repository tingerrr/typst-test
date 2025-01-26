use std::io::Write;

use color_eyre::eyre;
use lib::stdx;
use lib::stdx::fmt::Term;
use termcolor::Color;

use super::{Context, FilterArgs};
use crate::ui;

#[derive(clap::Args, Debug, Clone)]
#[group(id = "uninit-args")]
pub struct Args {
    /// Whether to the skip confirmation prompt
    #[arg(long, short)]
    pub force: bool,

    #[command(flatten)]
    pub filter: FilterArgs,
}

pub fn run(ctx: &mut Context, args: &Args) -> eyre::Result<()> {
    let project = ctx.project()?;
    let suite = ctx.collect_all_tests(&project)?;

    let len = suite.len();

    let confirmed = args.force
        || ctx.ui.prompt_yes_no(
            format!(
                "confirm deletion of {len} {}",
                Term::simple("test").with(len)
            ),
            false,
        )?;

    if !confirmed {
        ctx.error_aborted()?;
    }

    stdx::fs::remove_dir(project.paths().test_root(), true)?;

    let mut w = ctx.ui.stderr();

    write!(w, "Uninitialized ")?;
    if let Some(package) = project.manifest_package_info() {
        write!(w, "package ")?;
        ui::write_colored(&mut w, Color::Cyan, |w| write!(w, "{}", package.name))?
    } else {
        write!(w, "project")?;
    }
    let count = suite.matched().len();
    writeln!(
        w,
        ", removed {} {}",
        count,
        Term::simple("test").with(count),
    )?;

    Ok(())
}
