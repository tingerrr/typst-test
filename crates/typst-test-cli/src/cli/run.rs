use std::ops::Not;

use color_eyre::eyre;
use lib::doc::compare::Strategy;
use lib::doc::render::{self, Origin};

use super::{
    CompareArgs, CompileArgs, Context, Direction, ExportArgs, FilterArgs, RunArgs, CANCELLED,
};
use crate::cli::TestFailure;
use crate::report::Reporter;
use crate::runner::{Action, Runner, RunnerConfig};

#[derive(clap::Args, Debug, Clone)]
#[group(id = "run-args")]
pub struct Args {
    #[command(flatten)]
    pub compile: CompileArgs,

    /// Do not compare tests
    #[arg(long, short = 'C')]
    pub no_compare: bool,

    #[command(flatten)]
    pub compare: CompareArgs,

    /// Do not export any documents
    #[arg(long, short = 'E')]
    pub no_export: bool,

    #[command(flatten)]
    pub export: ExportArgs,

    #[command(flatten)]
    pub run: RunArgs,

    #[command(flatten)]
    pub filter: FilterArgs,
}

pub fn run(ctx: &mut Context, args: &Args) -> eyre::Result<()> {
    let project = ctx.project()?;
    let set = ctx.test_set(&args.filter)?;
    let suite = ctx.collect_tests(&project, &set)?;
    let world = ctx.world(&args.compile)?;

    let origin = args
        .export
        .render
        .direction
        .map(|dir| match dir {
            Direction::Ltr => Origin::TopLeft,
            Direction::Rtl => Origin::TopRight,
        })
        .unwrap_or_default();

    let runner = Runner::new(
        &project,
        &suite,
        &world,
        RunnerConfig {
            fail_fast: !args.run.no_fail_fast,
            pixel_per_pt: render::ppi_to_ppp(args.export.render.pixel_per_inch),
            action: Action::Run {
                strategy: args.no_compare.not().then_some(Strategy::Simple {
                    max_delta: args.compare.max_delta,
                    max_deviation: args.compare.max_deviation,
                }),
                export: !args.no_export,
                origin,
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
