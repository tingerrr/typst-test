use super::{CompareArgs, CompileArgs, Configure, Context, ExportArgs, OperationArgs, RunArgs};
use crate::error::TestFailure;
use crate::project::Project;
use crate::report::reports::SummaryReport;
use crate::report::LiveReporterState;
use crate::test::runner::RunnerConfig;

#[derive(clap::Args, Debug, Clone)]
#[group(id = "run-args")]
pub struct Args {
    #[command(flatten)]
    pub compile_args: CompileArgs,

    /// Do not compare tests
    #[arg(long, short = 'C')]
    pub no_compare: bool,

    #[command(flatten)]
    pub compare_args: CompareArgs,

    /// Do not export any documents
    #[arg(long, short = 'E')]
    pub no_export: bool,

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
        if !self.no_compare {
            self.compare_args.configure(ctx, project, config)?;
        }
        if !self.no_export {
            self.export_args.configure(ctx, project, config)?;
        }
        self.run_args.configure(ctx, project, config)?;

        Ok(())
    }
}

pub fn run(mut ctx: &mut Context, args: &Args) -> anyhow::Result<()> {
    let project = ctx.collect_tests(&args.op_args, None)?;
    let world = ctx.build_world(&project, &args.compile_args)?;
    let (runner, rx) = ctx.build_runner(&project, &world, args)?;

    ctx.reporter.run_start("Running")?;

    let summary = if !args.run_args.summary {
        rayon::scope(|scope| {
            let ctx = &mut ctx;
            let world = &world;
            let project = &project;

            scope.spawn(move |_| {
                let reporter = ctx.reporter;
                let mut w = reporter.ui().stderr();
                let mut state = LiveReporterState::new(&mut w, "tested", project.matched().len());
                while let Ok(event) = rx.recv() {
                    state.event(world, event).unwrap();
                }

                state.finish().unwrap();
            });

            runner.run()
        })?
    } else {
        runner.run()?
    };

    ctx.reporter
        .report(&SummaryReport::new("passed", &summary))?;

    if !summary.is_ok() {
        anyhow::bail!(TestFailure);
    }

    Ok(())
}
