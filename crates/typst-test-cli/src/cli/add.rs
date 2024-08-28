use std::io::Write;

use serde::Serialize;
use termcolor::{Color, WriteColor};
use typst_test_lib::test::id::Identifier;
use typst_test_lib::test::ReferenceKind;
use typst_test_lib::test_set;

use super::Context;
use crate::report::reports::TestJson;
use crate::report::{Report, Verbosity};
use crate::ui;

#[derive(clap::Args, Debug, Clone)]
#[group(id = "add-args")]
pub struct Args {
    /// Whether this test creates it's references on the fly
    ///
    /// An ephemeral test consists of two scripts which are compared
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

#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct AddedReport<'t>(TestJson<'t>);

impl Report for AddedReport<'_> {
    fn report<W: WriteColor>(&self, mut writer: W, _verbosity: Verbosity) -> anyhow::Result<()> {
        write!(writer, "Added ")?;
        ui::write_colored(&mut writer, Color::Cyan, |w| write!(w, "{}", self.0.id))?;
        writeln!(writer)?;

        Ok(())
    }
}

pub fn run(ctx: &mut Context, args: &Args) -> anyhow::Result<()> {
    let mut project = ctx.ensure_init()?;
    project.collect_tests(test_set::builtin::all())?;
    project.load_template()?;

    if project.matched().contains_key(&args.test) {
        ctx.operation_failure(|r| {
            r.ui().error_with(|w| {
                writeln!(w, "Test ")?;
                ui::write_colored(w, Color::Cyan, |w| write!(w, "{}", args.test))?;
                writeln!(w, " already exists")
            })
        })?;
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
    ctx.reporter.report(&AddedReport(TestJson::new(test)))?;

    Ok(())
}
