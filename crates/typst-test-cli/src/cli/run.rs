use std::collections::BTreeMap;
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Instant;

use rayon::prelude::*;

use super::{
    CompareArgs, CompileArgs, Configure, Context, ExportArgs, OperationArgs, Run, RunArgs,
};
use crate::project::Project;
use crate::report::{Summary, ANNOT_PADDING};
use crate::test::runner::{Event, EventPayload, Runner, RunnerConfig};
use crate::test::Stage;

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

impl Run for Args {
    fn compile_args(&self) -> &CompileArgs {
        &self.compile_args
    }

    fn run_args(&self) -> &RunArgs {
        &self.run_args
    }

    fn op_args(&self) -> &OperationArgs {
        &self.op_args
    }
}

pub fn run(ctx: &mut Context, args: &Args) -> anyhow::Result<()> {
    args.run(ctx)
}

pub fn run_impl(ctx: &mut Context, runner: Runner<'_>, run_args: &RunArgs) -> anyhow::Result<()> {
    let done_annot = if runner.config().compare() {
        "ok"
    } else if runner.config().update() {
        "updated"
    } else {
        "compiled"
    };

    ctx.reporter
        .lock()
        .unwrap()
        .test_start(runner.config().update())?;

    let start = Instant::now();
    runner.run_prepare_hook()?;

    let len = runner.project().matched().len();

    let failed_compilation = AtomicUsize::new(0);
    let failed_comparison = AtomicUsize::new(0);
    let failed_otherwise = AtomicUsize::new(0);
    let passed = AtomicUsize::new(0);

    rayon::scope(|scope| {
        let (tx, rx) = mpsc::channel();
        if ctx.args.global.output.format.is_pretty() {
            scope.spawn({
                let reporter = Arc::clone(&ctx.reporter);
                let failed_compilation = &failed_compilation;
                let failed_comparison = &failed_comparison;
                let failed_otherwise = &failed_otherwise;
                let passed = &passed;
                let world = runner.world();

                move |_| {
                    reporter.lock().unwrap().with_indent(2, |reporter| {
                        let mut tests = BTreeMap::new();
                        let mut count = 0;

                        // TODO: track times by comparing stage instants
                        while let Ok(Event {
                            test,
                            instant: _,
                            message: _,
                            payload,
                        }) = rx.recv()
                        {
                            let id = test.id();
                            match payload {
                                EventPayload::StartedTest => {
                                    tests.insert(id.clone(), (test, "start"));
                                }
                                EventPayload::StartedStage(stage) => {
                                    tests.get_mut(id).unwrap().1 = match stage {
                                        Stage::Preparation => "prepare",
                                        Stage::Hooks => "hook",
                                        Stage::Loading => "load",
                                        Stage::Compilation => "compile",
                                        Stage::Saving => "save",
                                        Stage::Rendering => "render",
                                        Stage::Comparison => "compare",
                                        Stage::Update => "update",
                                        Stage::Cleanup => "cleanup",
                                    };
                                }
                                EventPayload::FinishedStage(_) => {
                                    continue;
                                }
                                EventPayload::FailedStage(stage) => match stage {
                                    Stage::Compilation => {
                                        failed_compilation.fetch_add(1, Ordering::SeqCst);
                                    }
                                    Stage::Comparison => {
                                        failed_comparison.fetch_add(1, Ordering::SeqCst);
                                    }
                                    _ => {
                                        failed_otherwise.fetch_add(1, Ordering::SeqCst);
                                    }
                                },
                                EventPayload::FinishedTest => {
                                    tests.remove(id);
                                    reporter.test_success(&test, done_annot).unwrap();
                                    count += 1;
                                    passed.fetch_add(1, Ordering::SeqCst);
                                }
                                EventPayload::FailedTest(failure) => {
                                    tests.remove(id);
                                    reporter.test_failure(&test, failure, world).unwrap();
                                    count += 1;
                                }
                            }

                            for (test, msg) in tests.values() {
                                reporter.test_progress(test, msg).unwrap();
                            }

                            reporter
                                .write_annotated(
                                    "tested",
                                    termcolor::Color::Cyan,
                                    ANNOT_PADDING,
                                    |reporter| {
                                        writeln!(
                                            reporter,
                                            "{} / {} ({} tests running)",
                                            count,
                                            len,
                                            tests.len(),
                                        )
                                    },
                                )
                                .unwrap();

                            // clear the progress lines
                            reporter.clear_last_lines(tests.len() + 1).unwrap();
                        }
                    });
                }
            });
        }

        let res = runner.project().matched().par_iter().try_for_each(
            |(_, test)| -> Result<(), Option<anyhow::Error>> {
                match runner.test(test).run(tx.clone()) {
                    Ok(Ok(_)) => Ok(()),
                    Ok(Err(_)) => {
                        if runner.config().no_fail_fast() {
                            Ok(())
                        } else {
                            Err(None)
                        }
                    }
                    Err(err) => Err(Some(
                        err.context(format!("Fatal error when running test {}", test.id())),
                    )),
                }
            },
        );

        drop(tx);

        let time = start.elapsed();

        if let Err(Some(err)) = res {
            return Err(err);
        }

        runner.run_cleanup_hook()?;

        if !run_args.summary {
            writeln!(ctx.reporter.lock().unwrap())?;
        }

        let summary = Summary {
            total: runner.project().matched().len() + runner.project().filtered().len(),
            filtered: runner.project().filtered().len(),
            failed_compilation: failed_compilation.load(Ordering::SeqCst),
            failed_comparison: failed_comparison.load(Ordering::SeqCst),
            failed_otherwise: failed_otherwise.load(Ordering::SeqCst),
            passed: passed.load(Ordering::SeqCst),
            time,
        };

        let is_ok = summary.is_ok();
        ctx.reporter.lock().unwrap().test_summary(
            summary,
            runner.config().update(),
            run_args.summary,
        )?;

        if !is_ok {
            ctx.test_failure(|_| Ok(()))?;
        }

        Ok(())
    })
}
