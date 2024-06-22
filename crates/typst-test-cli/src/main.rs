use std::io::{ErrorKind, Write};
use std::process::ExitCode;

use clap::{ColorChoice, Parser};
use termcolor::{Color, WriteColor};
use tracing::Level;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::prelude::*;
use tracing_tree::HierarchicalLayer;
use typst_test_lib::compare;
use typst_test_lib::config::Config;
use typst_test_lib::store::test::matcher::{IdentifierMatcher, Matcher};

use self::cli::CliResult;
use self::project::Project;
use self::report::Reporter;

mod cli;
mod project;
mod report;
mod test;
mod util;

const IS_OUTPUT_STDERR: bool = false;

fn main() -> ExitCode {
    let mut args = cli::Args::parse();

    if !args.format.is_pretty() {
        args.color = ColorChoice::Never;
    }

    if args.verbose >= 1 {
        tracing_subscriber::registry()
            .with(
                HierarchicalLayer::new(4)
                    .with_targets(true)
                    .with_ansi(util::term::color(args.color, IS_OUTPUT_STDERR)),
            )
            .with(Targets::new().with_target(
                std::env!("CARGO_CRATE_NAME"),
                match args.verbose {
                    1 => Level::ERROR,
                    2 => Level::WARN,
                    3 => Level::INFO,
                    4 => Level::DEBUG,
                    _ => Level::TRACE,
                },
            ))
            .init();
    }

    // TODO: simpler output when using plain
    let mut reporter = Reporter::new(
        util::term::color_stream(args.color, IS_OUTPUT_STDERR),
        args.format,
    );

    let res = reporter.with_indent(2, |r| main_impl(args, r));

    let exit_code = match res {
        Ok(cli_res) => match cli_res {
            CliResult::Ok => cli::EXIT_OK,
            CliResult::TestFailure => cli::EXIT_TEST_FAILURE,
            CliResult::OperationFailure { message, hint } => {
                writeln!(reporter, "{message}").unwrap();
                if let Some(hint) = hint {
                    reporter.hint(&hint.to_string()).unwrap();
                }
                cli::EXIT_OPERATION_FAILURE
            }
        },
        Err(err) => {
            writeln!(
                reporter,
                "typst-test ran into an unexpected error, this is most likely a bug\n\
                Please consider reporting this at {}/issues/new",
                std::env!("CARGO_PKG_REPOSITORY")
            )
            .unwrap();

            reporter
                .with_indent(2, |r| {
                    r.write_annotated("Error:", Color::Red, |r| writeln!(r, "{err:?}"))
                })
                .unwrap();

            cli::EXIT_ERROR
        }
    };

    // NOTE: ensure we completely reset the terminal to standard
    reporter.reset().unwrap();
    write!(reporter, "").unwrap();
    ExitCode::from(exit_code)
}

fn main_impl(args: cli::Args, reporter: &mut Reporter) -> anyhow::Result<CliResult> {
    let root = match args.root {
        Some(root) => root,
        None => {
            let pwd = std::env::current_dir()?;
            match project::try_find_project_root(&pwd)? {
                Some(root) => root.to_path_buf(),
                None => {
                    return Ok(CliResult::hinted_operation_failure(
                        "Must be inside a typst project",
                        "You can pass the project root using '--root <path>'",
                    ));
                }
            }
        }
    };

    if !root.try_exists()? {
        return Ok(CliResult::operation_failure(format!(
            "Root '{}' directory not found",
            root.display(),
        )));
    }

    let manifest = match project::try_open_manifest(&root) {
        Ok(manifest) => manifest,
        Err(project::Error::InvalidManifest(err)) => {
            reporter.write_annotated("warning:", Color::Yellow, |this| {
                tracing::error!(?err, "Couldn't parse manifest");
                writeln!(this, "Error while parsing manifest, skipping")?;
                writeln!(this, "{}", err.message())
            })?;

            None
        }
        Err(err) => anyhow::bail!(err),
    };

    let manifest_config = manifest
        .as_ref()
        .and_then(|m| {
            m.tool
                .as_ref()
                .map(|t| t.get_section::<Config>("typst-test"))
        })
        .transpose()?
        .flatten();

    let config = util::result::ignore(
        std::fs::read_to_string(root.join("typst-test.toml")).map(Some),
        |err| err.kind() == ErrorKind::NotFound,
    )?;

    let config = config.map(|c| toml::from_str(&c)).transpose()?;

    if manifest_config.is_some() && config.is_some() {
        reporter.write_annotated("warning:", Color::Yellow, |this| {
            writeln!(
                this,
                "Ignoring manifest config in favor of 'typst-test.toml'"
            )
        })?;
    }

    let mut project = Project::new(
        root,
        config.or(manifest_config).unwrap_or_default(),
        manifest,
    );

    // TODO: report ignored or make sure we include them in listing
    let mut matcher = Matcher::default();
    if let Some(term) = args.filter.filter {
        matcher.name(Some(IdentifierMatcher::Simple {
            term: term.into(),
            exact: args.filter.exact,
        }));
    }

    let ctx = cmd::Context {
        project: &mut project,
        reporter,
        matcher,
    };

    let (runner_args, compare) = match args.cmd {
        cli::Command::Init { no_example } => return cmd::init(ctx, no_example),
        cli::Command::Uninit => return cmd::uninit(ctx),
        cli::Command::Clean => return cmd::clean(ctx),
        cli::Command::Add {
            test,
            ephemeral,
            compile_only,
            no_template,
        } => return cmd::add(ctx, test, ephemeral, compile_only, no_template),
        cli::Command::Edit => return cmd::edit(ctx, args.filter.all),
        cli::Command::Remove => return cmd::remove(ctx, args.filter.all),
        cli::Command::Status => return cmd::status(ctx),
        cli::Command::List => return cmd::list(ctx),
        cli::Command::Update {
            runner_args,
            all: _,
        } => return cmd::update(ctx, runner_args.summary, args.fail_fast),
        cli::Command::Compile(runner_args) => (runner_args, false),
        cli::Command::Run(runner_args) => (runner_args, true),
    };

    let compare = compare.then_some(compare::Strategy::default());

    cmd::run(ctx, runner_args.summary, args.fail_fast, compare)
}

mod cmd {
    use std::collections::BTreeMap;
    use std::io::Write;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{mpsc, Mutex};
    use std::time::Instant;

    use rayon::prelude::*;
    use termcolor::Color;
    use typst_test_lib::compare;
    use typst_test_lib::store::test::matcher::Matcher;
    use typst_test_lib::test::id::Identifier;
    use typst_test_lib::test::ReferenceKind;

    use crate::cli::CliResult;
    use crate::project::{Project, ScaffoldOptions};
    use crate::report::{Reporter, Summary};
    use crate::test::runner::{Event, EventPayload, RunnerConfig};
    use crate::test::Stage;
    use crate::util;

    pub struct Context<'a> {
        pub project: &'a mut Project,
        pub reporter: &'a mut Reporter,
        pub matcher: Matcher,
    }

    macro_rules! bail_gracefully {
        (if_uninit; $project:expr) => {
            if !$project.is_init()? {
                return Ok(CliResult::operation_failure(format!(
                    "Project '{}' was not initialized",
                    $project.name(),
                )));
            }
        };
    }

    pub fn init(ctx: Context, no_example: bool) -> anyhow::Result<CliResult> {
        if ctx.project.is_init()? {
            return Ok(CliResult::operation_failure(format!(
                "Project '{}' was already initialized",
                ctx.project.name(),
            )));
        }

        let mut options = ScaffoldOptions::empty();
        options.set(ScaffoldOptions::EXAMPLE, !no_example);

        ctx.project.init(options)?;
        writeln!(ctx.reporter, "Initialized project '{}'", ctx.project.name())?;

        Ok(CliResult::Ok)
    }

    pub fn uninit(ctx: Context) -> anyhow::Result<CliResult> {
        bail_gracefully!(if_uninit; ctx.project);

        ctx.project.collect_tests(ctx.matcher.clone())?;
        let count = ctx.project.matched().len();

        ctx.project.uninit()?;
        writeln!(
            ctx.reporter,
            "Removed {} {}",
            count,
            util::fmt::plural(count, "test"),
        )?;

        Ok(CliResult::Ok)
    }

    pub fn clean(ctx: Context) -> anyhow::Result<CliResult> {
        bail_gracefully!(if_uninit; ctx.project);

        ctx.project.collect_tests(ctx.matcher.clone())?;

        ctx.project.clean_artifacts()?;
        writeln!(ctx.reporter, "Removed test artifacts")?;

        Ok(CliResult::Ok)
    }

    pub fn add(
        ctx: Context,
        name: String,
        ephemeral: bool,
        compile_only: bool,
        no_template: bool,
    ) -> anyhow::Result<CliResult> {
        bail_gracefully!(if_uninit; ctx.project);

        let Ok(id) = Identifier::new(name) else {
            return Ok(CliResult::operation_failure("Invalid test name"));
        };

        ctx.project.collect_tests(ctx.matcher.clone())?;
        ctx.project.load_template()?;

        if ctx.project.matched().contains_key(&id) {
            return Ok(CliResult::operation_failure(format!(
                "Test '{}' already exists",
                id,
            )));
        }

        let kind = if ephemeral {
            Some(ReferenceKind::Ephemeral)
        } else if compile_only {
            None
        } else {
            Some(ReferenceKind::Persistent)
        };

        ctx.project.create_test(id.clone(), kind, !no_template)?;
        let test = &ctx.project.matched()[&id];
        ctx.reporter.test_added(test)?;

        Ok(CliResult::Ok)
    }

    pub fn remove(ctx: Context, all: bool) -> anyhow::Result<CliResult> {
        bail_gracefully!(if_uninit; ctx.project);

        ctx.project.collect_tests(ctx.matcher.clone())?;

        match ctx.project.matched().len() {
            0 => return Ok(CliResult::operation_failure("Matched no tests")),
            1 => {}
            _ if all => {}
            _ => {
                return Ok(CliResult::hinted_operation_failure(
                    "Matched more than one test",
                    "Pass `--all` to remove more than one test at a time",
                ))
            }
        }

        ctx.project.delete_tests()?;
        ctx.reporter.tests_success(&ctx.project, "removed")?;

        Ok(CliResult::Ok)
    }

    pub fn edit(ctx: Context, all: bool) -> anyhow::Result<CliResult> {
        bail_gracefully!(if_uninit; ctx.project);

        ctx.project.collect_tests(ctx.matcher.clone())?;

        match ctx.project.matched().len() {
            0 => return Ok(CliResult::operation_failure("Matched no tests")),
            1 => {}
            _ if all => {}
            _ => {
                return Ok(CliResult::hinted_operation_failure(
                    "Matched more than one test",
                    "Pass `--all` to edit more than one test at a time",
                ))
            }
        }

        // TODO: changing test kind

        Ok(CliResult::Ok)
    }

    pub fn update(ctx: Context, summary: bool, fail_fast: bool) -> anyhow::Result<CliResult> {
        run_tests(
            ctx,
            summary,
            true,
            |ctx| ctx.with_fail_fast(fail_fast).with_update(true),
            "updated",
        )
    }

    pub fn status(ctx: Context) -> anyhow::Result<CliResult> {
        bail_gracefully!(if_uninit; ctx.project);

        ctx.project.collect_tests(ctx.matcher.clone())?;
        ctx.project.load_template()?;

        ctx.reporter.project(ctx.project)?;

        Ok(CliResult::Ok)
    }

    pub fn list(ctx: Context) -> anyhow::Result<CliResult> {
        bail_gracefully!(if_uninit; ctx.project);

        ctx.project.collect_tests(ctx.matcher.clone())?;
        ctx.reporter.tests(ctx.project)?;

        Ok(CliResult::Ok)
    }

    pub fn run(
        ctx: Context,
        summary: bool,
        fail_fast: bool,
        compare: Option<compare::Strategy>,
    ) -> anyhow::Result<CliResult> {
        run_tests(
            ctx,
            summary,
            false,
            |ctx| ctx.with_fail_fast(fail_fast).with_compare_strategy(compare),
            if compare.is_some() { "ok" } else { "compiled" },
        )
    }

    fn run_tests<F>(
        ctx: Context,
        summary_only: bool,
        update: bool,
        f: F,
        done_annot: &str,
    ) -> anyhow::Result<CliResult>
    where
        F: FnOnce(&mut RunnerConfig) -> &mut RunnerConfig,
    {
        bail_gracefully!(if_uninit; ctx.project);

        ctx.project.collect_tests(ctx.matcher.clone())?;

        if ctx.project.matched().is_empty() {
            return Ok(CliResult::operation_failure(format!(
                "Project '{}' did not contain any tests",
                ctx.project.name(),
            )));
        }

        // TODO: port proper typst-cli impl
        let world = typst_test_lib::_dev::GlobalTestWorld::new(
            ctx.project.root().to_path_buf(),
            typst_test_lib::library::augmented_default_library(),
        );

        let mut config = RunnerConfig::default();
        config.with_save_temporary(true);
        f(&mut config);
        let runner = config.build(ctx.project, &world);

        ctx.reporter.test_start(update)?;

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

                    // clear progress line after we're done
                    print!("\x1B[1F\x1B[0J");
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

            if !summary_only {
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
                .test_summary(summary, update, summary_only)?;

            Ok(if is_ok {
                CliResult::Ok
            } else {
                CliResult::TestFailure
            })
        })
    }
}
