use color_eyre::eyre;
use lib::doc::render::{self, Origin};
use lib::test_set::eval;

use super::{CompileArgs, Context, Direction, ExportArgs, FilterArgs, RunArgs, CANCELLED};
use crate::cli::TestFailure;
use crate::report::Reporter;
use crate::runner::{Action, Runner, RunnerConfig};

#[derive(clap::Args, Debug, Clone)]
#[group(id = "update-args")]
pub struct Args {
    #[command(flatten)]
    pub compile: CompileArgs,

    #[command(flatten)]
    pub export: ExportArgs,

    #[command(flatten)]
    pub run: RunArgs,

    #[command(flatten)]
    pub filter: FilterArgs,
}

pub fn run(ctx: &mut Context, args: &Args) -> eyre::Result<()> {
    let project = ctx.project()?;
    let mut set = ctx.test_set(&args.filter)?;
    set.add_intersection(eval::Set::built_in_persistent());
    let suite = ctx.collect_tests(&project, &set)?;
    let world = ctx.world(&args.compile)?;

    let runner = Runner::new(
        &project,
        &suite,
        &world,
        RunnerConfig {
            promote_warnings: args.compile.promote_warnings,
            optimize: !args.export.no_optimize_references,
            fail_fast: !args.run.no_fail_fast,
            pixel_per_pt: render::ppi_to_ppp(args.export.render.pixel_per_inch),
            action: Action::Update {
                export: true,
                origin: args
                    .export
                    .render
                    .direction
                    .map(|dir| match dir {
                        Direction::Ltr => Origin::TopLeft,
                        Direction::Rtl => Origin::TopRight,
                    })
                    .unwrap_or_default(),
            },
            cancellation: &CANCELLED,
        },
    );

    let reporter = Reporter::new(
        ctx.ui,
        &project,
        &world,
        ctx.ui.can_live_report() && ctx.args.global.output.verbose == 0,
    );
    let result = runner.run(&reporter)?;

    if !result.is_complete_pass() {
        eyre::bail!(TestFailure);
    }

    Ok(())
}
