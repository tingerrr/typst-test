use std::io::Write;

use super::Context;
use crate::cli::{CompileArgs, Configure, ExportArgs, OperationArgs, RunArgs};
use crate::project::Project;
use crate::report::reports::SummaryReport;
use crate::report::LiveReporterState;
use crate::test::runner::RunnerConfig;

#[derive(clap::Args, Debug, Clone)]
#[group(id = "export-args")]
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
    let project = ctx.collect_tests(&args.op_args, None)?;
    let world = ctx.build_world(&project, &args.compile_args)?;
    let (runner, rx) = ctx.build_runner(&project, &world, args)?;

    ctx.reporter.lock().unwrap().run_start("Exporting")?;

    let summary = if !args.run_args.summary {
        rayon::scope(|scope| {
            let ctx = &mut ctx;
            let world = &world;
            let project = &project;

            scope.spawn(move |_| {
                let reporter = ctx.reporter.lock().unwrap();
                let mut w = reporter.ui().stderr();
                let mut state = LiveReporterState::new(&mut w, "exported", project.matched().len());
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
        .lock()
        .unwrap()
        .report(&SummaryReport::new("exported", &summary))?;

    Ok(())
}
