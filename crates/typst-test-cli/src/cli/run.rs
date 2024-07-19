use std::collections::BTreeMap;
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Instant;

use chrono::{DateTime, Utc};
use rayon::prelude::*;
use termcolor::Color;

use super::{Context, OperationArgs};
use crate::package::PackageStorage;
use crate::project::Project;
use crate::report::{Summary, ANNOT_PADDING};
use crate::test::runner::{Event, EventPayload, RunnerConfig};
use crate::test::Stage;
use crate::world::SystemWorld;

fn parse_source_date_epoch(raw: &str) -> Result<DateTime<Utc>, String> {
    let timestamp: i64 = raw
        .parse()
        .map_err(|err| format!("timestamp must be decimal integer ({err})"))?;
    DateTime::from_timestamp(timestamp, 0).ok_or_else(|| "timestamp out of range".to_string())
}

#[derive(clap::Args, Debug, Clone)]
#[group(id = "run-args")]
pub struct Args {
    /// The timestamp used for compilation.
    ///
    /// For more information, see
    /// <https://reproducible-builds.org/specs/source-date-epoch/>.
    #[clap(
        long = "creation-timestamp",
        env = "SOURCE_DATE_EPOCH",
        value_name = "UNIX_TIMESTAMP",
        value_parser = parse_source_date_epoch,
    )]
    pub now: Option<DateTime<Utc>>,

    /// Whether to abort after the first failure
    ///
    /// Keep in mind that because tests are run in parallel, this may not stop
    /// immediately. But it will not schedule any new tests to run after one
    /// failure has been detected.
    #[arg(long)]
    pub no_fail_fast: bool,

    /// Do not run hooks
    #[arg(long)]
    pub no_hooks: bool,

    /// Show a summary of the test run instead of the individual test results
    #[arg(long)]
    pub summary: bool,

    #[command(flatten)]
    pub op_args: OperationArgs,
}

pub fn run<F>(ctx: &mut Context, project: Project, args: &Args, f: F) -> anyhow::Result<()>
where
    F: FnOnce(&mut RunnerConfig) -> &mut RunnerConfig,
{
    let world = SystemWorld::new(
        project.root().to_path_buf(),
        ctx.args.global.fonts.searcher(),
        PackageStorage::from_args(&ctx.args.global.package),
        args.now,
    )?;

    let root = project.root();

    let mut config = RunnerConfig::default();
    config.with_no_fail_fast(args.no_fail_fast);
    if !args.no_hooks {
        config.with_prepare_hook(
            project
                .config()
                .prepare
                .as_deref()
                .map(|rel| root.join(rel)),
        );
        config.with_prepare_each_hook(
            project
                .config()
                .prepare_each
                .as_deref()
                .map(|rel| root.join(rel)),
        );
        config.with_cleanup_hook(
            project
                .config()
                .cleanup
                .as_deref()
                .map(|rel| root.join(rel)),
        );
        config.with_cleanup_each_hook(
            project
                .config()
                .cleanup_each
                .as_deref()
                .map(|rel| root.join(rel)),
        );
    }
    f(&mut config);
    tracing::trace!(?config, "prepared project config");
    let runner = config.build(&project, &world);

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

    let len = project.matched().len();

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
                let world = &world;

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

                            for (_, (test, msg)) in &tests {
                                reporter.test_progress(test, msg).unwrap();
                            }

                            reporter
                                .write_annotated("tested", Color::Cyan, ANNOT_PADDING, |reporter| {
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
                            reporter.clear_last_lines(tests.len() + 1).unwrap();
                        }
                    });
                }
            });
        }

        let res = project.matched().par_iter().try_for_each(
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

        if !args.summary {
            writeln!(ctx.reporter.lock().unwrap())?;
        }

        let summary = Summary {
            total: project.matched().len() + project.filtered().len(),
            filtered: project.filtered().len(),
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
            args.summary,
        )?;

        if !is_ok {
            ctx.test_failure(|_| Ok(()))?;
        }

        Ok(())
    })
}
