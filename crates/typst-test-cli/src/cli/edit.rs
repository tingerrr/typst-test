use std::io::Write;

use typst_test_lib::test::ReferenceKind;

use super::{CompileArgs, Configure, Context, OperationArgs, RunArgs};
use crate::project::Project;
use crate::report::LiveReporterState;
use crate::test::runner::RunnerConfig;

#[derive(clap::Args, Debug, Clone)]
#[group(id = "edit-args")]
pub struct Args {
    /// The kind to set the tests too
    #[arg(long)]
    pub kind: Kind,

    #[command(flatten)]
    pub compile_args: CompileArgs,

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
        self.run_args.configure(ctx, project, config)?;

        config
            .with_compare(false)
            .with_edit_kind(Some(match self.kind {
                Kind::CompileOnly => None,
                Kind::Persistent => Some(ReferenceKind::Persistent),
                Kind::Ephemeral => Some(ReferenceKind::Ephemeral),
            }));

        Ok(())
    }
}

#[derive(clap::ValueEnum, Debug, Clone)]
pub enum Kind {
    /// Mark the selected tests as compile only
    CompileOnly,

    /// Mark the selected tests as persistent
    Persistent,

    /// Mark the selected tests as ephemeral
    Ephemeral,
}

pub fn run(mut ctx: &mut Context, args: &Args) -> anyhow::Result<()> {
    let project = ctx.collect_tests(&args.op_args, "edit")?;
    let world = ctx.build_world(&project, &args.compile_args)?;
    let (runner, rx) = ctx.build_runner(&project, &world, args)?;

    ctx.reporter.lock().unwrap().run_start("Editing")?;

    let summary = if !args.run_args.summary {
        rayon::scope(|scope| {
            let ctx = &mut ctx;
            let world = &world;
            let project = &project;

            scope.spawn(move |_| {
                let mut reporter = ctx.reporter.lock().unwrap();
                let mut state = LiveReporterState::new("edited", project.matched().len());
                while let Ok(event) = rx.recv() {
                    state.event(&mut reporter, world, event).unwrap();
                }

                writeln!(reporter).unwrap();
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
        .run_summary(summary, "edited", args.run_args.summary)?;

    Ok(())
}
