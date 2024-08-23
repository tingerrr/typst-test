use std::io::Write;

use super::{CompileArgs, Configure, Context, ExportArgs, OperationArgs, RunArgs};
use crate::project::Project;
use crate::report::reports::SummaryReport;
use crate::report::LiveReporterState;
use crate::test::runner::RunnerConfig;

#[derive(clap::Args, Debug, Clone)]
#[group(id = "update-args")]
pub struct Args {
    #[command(flatten)]
    pub compile_args: CompileArgs,

    #[command(flatten)]
    pub export_args: ExportArgs,

    #[command(flatten)]
    pub run_args: RunArgs,

    #[command(flatten)]
    pub op_args: OperationArgs,
}

impl Configure for Args {
    fn configure(
        &self,
        ctx: &mut Context,
        project: &Project,
        config: &mut RunnerConfig,
    ) -> anyhow::Result<()> {
        self.compile_args.configure(ctx, project, config)?;
        self.export_args.configure(ctx, project, config)?;
        self.run_args.configure(ctx, project, config)?;

        Ok(())
    }
}

pub fn run(mut ctx: &mut Context, args: &Args) -> anyhow::Result<()> {
    let project = ctx.collect_tests(&args.op_args, "update")?;
    let world = ctx.build_world(&project, &args.compile_args)?;
    let (runner, rx) = ctx.build_runner(&project, &world, args)?;

    ctx.reporter.run_start("Updating")?;

    let summary = if !args.run_args.summary {
        rayon::scope(|scope| {
            let ctx = &mut ctx;
            let world = &world;
            let project = &project;

            scope.spawn(move |_| {
                let reporter = ctx.reporter;
                let mut w = reporter.ui().stderr();
                let mut state = LiveReporterState::new(&mut w, "updated", project.matched().len());
                while let Ok(event) = rx.recv() {
                    state.event(world, event).unwrap();
                }

                writeln!(w).unwrap();
            });

            runner.run()
        })?
    } else {
        runner.run()?
    };

    if !summary.is_ok() {
        ctx.set_test_failure();
    }

    ctx.reporter
        .report(&SummaryReport::new("updated", &summary))?;

    Ok(())
}
