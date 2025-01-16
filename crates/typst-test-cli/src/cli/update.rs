use color_eyre::eyre;
use lib::doc::render::{self, Origin};

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

    // TODO(tinger): see test_set API
    let set = ctx.test_set(&FilterArgs {
        expression: format!("( {} ) & persistent()", args.filter.expression),
        ..args.filter.clone()
    })?;

    let suite = ctx.collect_tests(&project, &set)?;
    let world = ctx.world(&args.compile)?;

    let runner = Runner::new(
        &project,
        &suite,
        &world,
        RunnerConfig {
            promote_warnings: args.compile.promote_warnings,
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

    if project.vcs().is_some() {
        ctx.ui.warning_hinted(
            "Updated references are not compressed, but persisted in a repository",
            "Consider using a program like `oxipng` to reduce repository bloat",
        )?;
    }

    if !result.is_complete_pass() {
        eyre::bail!(TestFailure);
    }

    Ok(())
}
