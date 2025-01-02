use std::io::Write;

use color_eyre::eyre;
use lib::test::Kind as TestKind;
use termcolor::Color;

use super::{Context, FilterArgs};
use crate::json::TestJson;
use crate::ui;
use crate::ui::Indented;

#[derive(clap::Args, Debug, Clone)]
#[group(id = "list-args")]
pub struct Args {
    /// Print a JSON describing the project to stdout
    #[arg(long)]
    pub json: bool,

    #[command(flatten)]
    pub filter: FilterArgs,
}

pub fn run(ctx: &mut Context, args: &Args) -> eyre::Result<()> {
    let project = ctx.project()?;
    let set = ctx.test_set(&args.filter)?;
    let suite = ctx.collect_tests(&project, &set)?;

    if args.json {
        serde_json::to_writer_pretty(
            ctx.ui.stdout(),
            &suite
                .matched()
                .values()
                .map(TestJson::new)
                .collect::<Vec<_>>(),
        )?;

        return Ok(());
    }

    let mut w = ctx.ui.stderr();

    ui::write_bold(&mut w, |w| writeln!(w, "Tests"))?;

    let w = &mut Indented::new(w, 2);

    // NOTE(tinger): max padding of 50 should be enough for most cases
    let pad = Ord::min(
        suite
            .matched()
            .keys()
            .map(|id| id.len())
            .max()
            .unwrap_or(usize::MAX),
        50,
    );

    for (id, test) in suite.matched() {
        write!(w, "{: <pad$} ", id)?;
        let color = match test.kind() {
            TestKind::Ephemeral => Color::Yellow,
            TestKind::Persistent => Color::Green,
            TestKind::CompileOnly => Color::Yellow,
        };
        ui::write_bold_colored(w, color, |w| writeln!(w, "{}", test.kind().as_str()))?;
    }

    Ok(())
}
