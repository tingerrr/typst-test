use std::io::Write;

use color_eyre::eyre;
use lib::test::Kind;
use termcolor::Color;

use super::Context;
use crate::json::ProjectJson;
use crate::ui;

#[derive(clap::Args, Debug, Clone)]
#[group(id = "status-args")]
pub struct Args {
    /// Print a JSON describing the project to stdout
    #[arg(long)]
    pub json: bool,
}

pub fn run(ctx: &mut Context, args: &Args) -> eyre::Result<()> {
    let project = ctx.project()?;
    let suite = ctx.collect_all_tests(&project)?;

    let delim_open = " ┌ ";
    let delim_middle = " ├ ";
    let delim_close = " └ ";

    if args.json {
        serde_json::to_writer_pretty(ctx.ui.stdout(), &ProjectJson::new(&project, &suite))?;
        return Ok(());
    }

    let mut w = ctx.ui.stderr();

    let align = ["Template", "Project", "Tests"]
        .map(str::len)
        .into_iter()
        .max()
        .unwrap();

    if let Some(package) = project.manifest_package_info() {
        write!(w, "{:>align$}{}", "Project", delim_open)?;
        ui::write_bold_colored(&mut w, Color::Cyan, |w| write!(w, "{}", &package.name))?;
        write!(w, ":")?;
        ui::write_bold_colored(&mut w, Color::Cyan, |w| write!(w, "{}", &package.version))?;
    } else {
        write!(w, "{:>align$}{}", "Project", delim_open)?;
        ui::write_bold_colored(&mut w, Color::Yellow, |w| write!(w, "none"))?;
    }
    writeln!(w)?;

    write!(w, "{:>align$}{}", "Vcs", delim_middle)?;
    if let Some(vcs) = project.vcs() {
        ui::write_bold_colored(&mut w, Color::Green, |w| write!(w, "{vcs}"))?;
    } else {
        ui::write_bold_colored(&mut w, Color::Yellow, |w| write!(w, "none"))?;
    }
    writeln!(w)?;

    if suite.matched().is_empty() {
        write!(w, "{:>align$}{}", "Tests", delim_close)?;
        ui::write_bold_colored(&mut w, Color::Cyan, |w| write!(w, "none"))?;
        writeln!(w)?;
    } else {
        let mut persistent = 0;
        let mut ephemeral = 0;
        let mut compile_only = 0;

        for test in suite.matched().values() {
            match test.kind() {
                Kind::Persistent => persistent += 1,
                Kind::Ephemeral => ephemeral += 1,
                Kind::CompileOnly => compile_only += 1,
            }
        }

        write!(w, "{:>align$}{}", "Tests", delim_middle)?;
        ui::write_bold_colored(&mut w, Color::Green, |w| write!(w, "{persistent}"))?;
        writeln!(w, " persistent")?;

        write!(w, "{:>align$}{}", "", delim_middle)?;
        ui::write_bold_colored(&mut w, Color::Green, |w| write!(w, "{ephemeral}"))?;
        writeln!(w, " ephemeral")?;

        write!(w, "{:>align$}{}", "", delim_close)?;
        ui::write_bold_colored(&mut w, Color::Yellow, |w| write!(w, "{compile_only}"))?;
        writeln!(w, " compile-only")?;
    }

    // TODO(tinger): this may be misunderstood as the package being a template
    // write!(w, "{:>align$}{}", "Template", delims.close)?;
    // if let Some(path) = suite.template() {
    //     ui::write_bold_colored(&mut w, Color::Cyan, |w| write!(w, "{path}"))?;
    // } else {
    //     ui::write_bold_colored(&mut w, Color::Green, |w| write!(w, "none"))?;
    // }
    // writeln!(w)?;

    Ok(())
}
