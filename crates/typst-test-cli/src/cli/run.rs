use std::collections::BTreeMap;
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Mutex};
use std::time::Instant;

use comemo::Prehashed;
use rayon::prelude::*;
use termcolor::Color;

use super::{Context, Global};
use crate::cli::{bail_if_uninit, CliResult};
use crate::fonts::FontSlot;
use crate::report::Summary;
use crate::test::runner::{Event, EventPayload, RunnerConfig};
use crate::test::Stage;

#[derive(clap::Parser, Debug, Clone)]
pub struct Args {
    /// Whether to abort after the first failure
    ///
    /// Keep in mind that because tests are run in parallel, this may not stop
    /// immediately. But it will not schedule any new tests to run after one
    /// failure has been detected.
    #[arg(long)]
    pub fail_fast: bool,

    /// Whether to save temporary output
    #[arg(long)]
    pub save_temporary: bool,

    /// Show a summary of the test run instread of the individual test results
    #[arg(long)]
    pub summary: bool,
}

pub fn run<F>(ctx: Context, global: &Global, args: &Args, f: F) -> anyhow::Result<CliResult>
where
    F: FnOnce(&mut RunnerConfig) -> &mut RunnerConfig,
{
    bail_if_uninit!(ctx);

    let matcher = global.matcher.matcher();
    ctx.project.collect_tests(matcher)?;

    if ctx.project.matched().is_empty() {
        return Ok(CliResult::operation_failure(format!(
            "Project '{}' did not contain any tests",
            ctx.project.name(),
        )));
    }

    let searcher = global.fonts.searcher();

    // TODO: port proper typst-cli impl
    let mut world = typst_test_lib::_dev::GlobalTestWorld::new(
        ctx.project.root().to_path_buf(),
        typst_test_lib::library::augmented_default_library(),
    );

    world.fonts = searcher
        .fonts
        .iter()
        .map(FontSlot::get)
        .map(Option::unwrap)
        .collect();

    world.book = Prehashed::new(searcher.book);

    let mut config = RunnerConfig::default();
    config.with_save_temporary(true);
    f(&mut config);
    let runner = config.build(ctx.project, &world);

    let done_annot = if runner.config().compare() {
        "ok"
    } else if runner.config().update() {
        "updated"
    } else {
        "compiled"
    };

    ctx.reporter.test_start(runner.config().update())?;

    let start = Instant::now();
    runner.prepare()?;

    let len = ctx.project.matched().len();

    let failed_compilation = AtomicUsize::new(0);
    let failed_comparison = AtomicUsize::new(0);
    let failed_otherwise = AtomicUsize::new(0);
    let passed = AtomicUsize::new(0);

    let reporter = Mutex::new(ctx.reporter);
    rayon::scope(|scope| {
        let (tx, rx) = mpsc::channel();
        scope.spawn({
            let reporter = &reporter;
            let failed_compilation = &failed_compilation;
            let failed_comparison = &failed_comparison;
            let failed_otherwise = &failed_otherwise;
            let passed = &passed;

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
                                reporter.test_failure(&test, failure).unwrap();
                                count += 1;
                            }
                        }

                        for (_, (test, msg)) in &tests {
                            reporter.test_progress(test, msg).unwrap();
                        }

                        reporter
                            .write_annotated("tested", Color::Cyan, |reporter| {
                                writeln!(
                                    reporter,
                                    "{} / {} ({} tests running)",
                                    count,
                                    len,
                                    tests.len(),
                                )
                            })
                            .unwrap();

                        // clear the progress lines
                        print!("\x1B[{}F\x1B[0J", tests.len() + 1);
                    }
                });
            }
        });

        let res = ctx.project.matched().par_iter().try_for_each(
            |(_, test)| -> Result<(), Option<anyhow::Error>> {
                match runner.test(test).run(tx.clone()) {
                    Ok(Ok(_)) => Ok(()),
                    Ok(Err(_)) => {
                        if runner.config().fail_fast() {
                            Err(None)
                        } else {
                            Ok(())
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

        runner.cleanup()?;

        if !args.summary {
            writeln!(reporter.lock().unwrap())?;
        }

        let summary = Summary {
            total: ctx.project.matched().len() + ctx.project.filtered().len(),
            filtered: ctx.project.filtered().len(),
            failed_compilation: failed_compilation.load(Ordering::SeqCst),
            failed_comparison: failed_comparison.load(Ordering::SeqCst),
            failed_otherwise: failed_otherwise.load(Ordering::SeqCst),
            passed: passed.load(Ordering::SeqCst),
            time,
        };

        let is_ok = summary.is_ok();
        reporter
            .lock()
            .unwrap()
            .test_summary(summary, runner.config().update(), args.summary)?;

        Ok(if is_ok {
            CliResult::Ok
        } else {
            CliResult::TestFailure
        })
    })
}
