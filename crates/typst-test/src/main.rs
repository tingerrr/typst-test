use std::io::{ErrorKind, Write};
use std::process::ExitCode;

use clap::{ColorChoice, Parser};
use cli::OutputFormat;
use config::Config;
use project::test::Filter;
use termcolor::{Color, WriteColor};
use tracing::Level;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::prelude::*;
use tracing_tree::HierarchicalLayer;

use self::cli::CliResult;
use self::project::Project;
use self::report::Reporter;

mod cli;
mod config;
mod project;
mod report;
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
                    r.write_annotated("Error:", Color::Red, |r| writeln!(r, "{err}"))
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
        Err(_) => {
            reporter.write_annotated("warning: ", Color::Yellow, |this| {
                writeln!(this, "Error while parsing manifest, skipping")
            })?;

            None
        }
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
        reporter.write_annotated("warning: ", Color::Yellow, |this| {
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

    let (runner_args, compare) = match args.cmd {
        cli::Command::Init {
            no_example,
            no_ignore,
            no_gitignore,
        } => return cmd::init(&mut project, reporter, no_example, no_ignore, no_gitignore),
        cli::Command::Uninit => return cmd::uninit(&mut project, reporter),
        cli::Command::Clean => return cmd::clean(&mut project, reporter),
        cli::Command::Add { open, test } => return cmd::add(&mut project, reporter, test, open),
        cli::Command::Edit { test } => return cmd::edit(&mut project, reporter, test),
        cli::Command::Remove { test } => return cmd::remove(&mut project, reporter, test),
        cli::Command::Status => return cmd::status(&mut project, reporter, args.typst),
        cli::Command::List => return cmd::list(&mut project, reporter),
        cli::Command::Update {
            runner_args,
            no_optimize,
        } => {
            return cmd::update(
                &mut project,
                reporter,
                runner_args.summary,
                args.typst,
                runner_args
                    .filter
                    .filter
                    .map(|f| Filter::new(f, runner_args.filter.exact)),
                util::term::color(args.color, IS_OUTPUT_STDERR),
                args.fail_fast,
                !no_optimize,
            )
        }
        cli::Command::Compile(runner_args) => (runner_args, false),
        cli::Command::Run(runner_args) => (runner_args, true),
    };

    cmd::run(
        &mut project,
        reporter,
        runner_args.summary,
        args.typst,
        runner_args
            .filter
            .filter
            .map(|f| Filter::new(f, runner_args.filter.exact)),
        util::term::color(args.color, IS_OUTPUT_STDERR),
        args.fail_fast,
        compare,
    )
}

mod cmd {
    use std::io::Write;
    use std::path::PathBuf;
    use std::str::FromStr;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;
    use std::time::Instant;

    use anyhow::Context as _;
    use rayon::prelude::*;
    use semver::Version;

    use crate::cli::CliResult;
    use crate::project::test::context::Context;
    use crate::project::test::{Filter, Stage};
    use crate::project::{Project, ScaffoldOptions};
    use crate::report::{Reporter, Summary};
    use crate::util;

    macro_rules! bail_gracefully {
        (if_no_typst; $project:expr; $typst:expr) => {
            if let Err(which::Error::CannotFindBinaryPath) = which::which($typst) {
                return Ok(CliResult::hinted_operation_failure(
                    format!("No typst binary '{}' found in PATH", $typst.display()),
                    "You can pass the correct typst binary using '--typst <path or name>'",
                ));
            }
        };
        (if_uninit; $project:expr) => {
            if !$project.is_init()? {
                return Ok(CliResult::operation_failure(format!(
                    "Project '{}' was not initialized",
                    $project.name(),
                )));
            }
        };
        (if_test_not_found; $project:expr; $test:expr => $name:ident) => {
            let Some($name) = $project.get_test($test) else {
                return Ok(CliResult::operation_failure(format!(
                    "Test '{}' could not be found",
                    $test,
                )));
            };
        };
        (if_test_not_new; $project:expr; $name:expr) => {
            if $project.get_test($name).is_some() {
                return Ok(CliResult::operation_failure(format!(
                    "Test '{}' already exists",
                    $name,
                )));
            }
        };
        (if_no_tests_found; $project:expr) => {
            if $project.tests().is_empty() {
                return Ok(CliResult::operation_failure(format!(
                    "Project '{}' did not contain any tests",
                    $project.name(),
                )));
            }
        };
        (if_no_tests_match; $project:expr; $filter:expr) => {
            if let Some(filter) = $filter {
                match filter {
                    Filter::Exact(f) => {
                        $project.tests_mut().retain(|n, _| n == f);
                    }
                    Filter::Contains(f) => {
                        $project.tests_mut().retain(|n, _| n.contains(f));
                    }
                }

                if $project.tests().is_empty() {
                    return Ok(CliResult::operation_failure(format!(
                        "Filter '{}' did not match any tests",
                        filter.value(),
                    )));
                }
            }
        };
    }

    pub fn init(
        project: &mut Project,
        reporter: &mut Reporter,
        no_example: bool,
        no_ignore: bool,
        no_gitignore: bool,
    ) -> anyhow::Result<CliResult> {
        if project.is_init()? {
            return Ok(CliResult::operation_failure(format!(
                "Project '{}' was already initialized",
                project.name(),
            )));
        }

        let mut options = ScaffoldOptions::empty();
        options.set(ScaffoldOptions::EXAMPLE, !no_example);
        options.set(ScaffoldOptions::IGNORE, !no_ignore);
        options.set(ScaffoldOptions::GITIGNORE, !no_gitignore);

        project.init(options)?;
        writeln!(reporter, "Initialized project '{}'", project.name())?;

        Ok(CliResult::Ok)
    }

    pub fn uninit(project: &mut Project, reporter: &mut Reporter) -> anyhow::Result<CliResult> {
        bail_gracefully!(if_uninit; project);

        project.discover_tests()?;
        let count = project.tests().len();

        project.uninit()?;
        writeln!(
            reporter,
            "Removed {} {}",
            count,
            util::fmt::plural(count, "test"),
        )?;

        Ok(CliResult::Ok)
    }

    pub fn clean(project: &mut Project, reporter: &mut Reporter) -> anyhow::Result<CliResult> {
        bail_gracefully!(if_uninit; project);

        project.discover_tests()?;

        project.clean_artifacts()?;
        writeln!(reporter, "Removed test artifacts")?;

        Ok(CliResult::Ok)
    }

    pub fn add(
        project: &mut Project,
        reporter: &mut Reporter,
        name: String,
        open: bool,
    ) -> anyhow::Result<CliResult> {
        bail_gracefully!(if_uninit; project);

        project.discover_tests()?;
        project.load_template()?;

        bail_gracefully!(if_test_not_new; project; &name);

        let (test, has_ref) = project.create_test(&name)?;
        reporter.test_added(test, !has_ref)?;

        if open {
            // NOTE: because test from create_test extends the mutable borrow
            // we must end it early
            let test = project.find_test(&name)?;

            // BUG: this may fail silently if the path doesn't exist
            open::that_detached(test.test_file(project))?;
        }

        Ok(CliResult::Ok)
    }

    pub fn remove(
        project: &mut Project,
        reporter: &mut Reporter,
        name: String,
    ) -> anyhow::Result<CliResult> {
        bail_gracefully!(if_uninit; project);

        project.discover_tests()?;
        bail_gracefully!(if_test_not_found; project; &name => test);

        project.remove_test(test.name())?;
        reporter.test_success(test, "removed")?;

        Ok(CliResult::Ok)
    }

    pub fn edit(
        project: &mut Project,
        reporter: &mut Reporter,
        name: String,
    ) -> anyhow::Result<CliResult> {
        bail_gracefully!(if_uninit; project);

        project.discover_tests()?;
        bail_gracefully!(if_test_not_found; project; &name => test);

        open::that_detached(test.test_file(project))?;
        reporter.test_success(test, "opened")?;

        Ok(CliResult::Ok)
    }

    pub fn update(
        project: &mut Project,
        reporter: &mut Reporter,
        summary: bool,
        typst: PathBuf,
        filter: Option<Filter>,
        color: bool,
        fail_fast: bool,
        optimize: bool,
    ) -> anyhow::Result<CliResult> {
        run_tests(
            project,
            reporter,
            summary,
            filter,
            false,
            true,
            |project| {
                let mut ctx = Context::new(project, typst);
                ctx.with_color(color)
                    .with_fail_fast(fail_fast)
                    .with_compare(false)
                    .with_update(true)
                    .with_optimize(optimize);
                ctx
            },
            "updated",
        )
    }

    pub fn status(
        project: &mut Project,
        reporter: &mut Reporter,
        typst: PathBuf,
    ) -> anyhow::Result<CliResult> {
        bail_gracefully!(if_uninit; project);

        project.discover_tests()?;
        project.load_template()?;

        let path = which::which(&typst).ok();
        reporter.project(project, typst, path)?;

        Ok(CliResult::Ok)
    }

    pub fn list(project: &mut Project, reporter: &mut Reporter) -> anyhow::Result<CliResult> {
        bail_gracefully!(if_uninit; project);

        project.discover_tests()?;
        reporter.tests(project)?;

        Ok(CliResult::Ok)
    }

    pub fn run(
        project: &mut Project,
        reporter: &mut Reporter,
        summary: bool,
        typst: PathBuf,
        filter: Option<Filter>,
        color: bool,
        fail_fast: bool,
        compare: bool,
    ) -> anyhow::Result<CliResult> {
        run_tests(
            project,
            reporter,
            summary,
            filter,
            compare,
            false,
            |project| {
                let mut ctx = Context::new(project, typst);
                ctx.with_color(color)
                    .with_fail_fast(fail_fast)
                    .with_compare(compare)
                    .with_update(false);
                ctx
            },
            if compare { "ok" } else { "compiled" },
        )
    }

    fn run_tests(
        project: &mut Project,
        reporter: &mut Reporter,
        summary_only: bool,
        filter: Option<Filter>,
        compare: bool,
        update: bool,
        prepare_ctx: impl FnOnce(&Project) -> Context,
        done_annot: &str,
    ) -> anyhow::Result<CliResult> {
        bail_gracefully!(if_uninit; project);

        project.discover_tests()?;
        let pre_filter = project.tests().len();
        bail_gracefully!(if_no_tests_found; project);
        bail_gracefully!(if_no_tests_match; project; &filter);
        let post_filter = project.tests().len();

        let mut ctx = prepare_ctx(project);
        bail_gracefully!(if_no_typst; project; ctx.typst());

        let version = util::command::parse_stdout(
            ctx.typst(),
            &["--version"],
            |stdout| -> anyhow::Result<Version> {
                // "typst <x>.<y>.<z> (<hash>)"
                Ok(Version::from_str(
                    stdout.split(' ').nth(1).context("version wasn't given")?,
                )?)
            },
        )??;

        ctx.with_typst_version(Some(version));

        reporter.test_start(update)?;

        let compiled = AtomicUsize::new(0);
        let compared = compare.then_some(AtomicUsize::new(0));
        let updated = update.then_some(AtomicUsize::new(0));

        let maybe_increment = |c: &Option<AtomicUsize>| {
            if let Some(c) = c {
                c.fetch_add(1, Ordering::SeqCst);
            }
        };

        let time = reporter.with_indent(2, |reporter| {
            let start = Instant::now();
            ctx.prepare()?;

            let reporter = Mutex::new(reporter);
            let res = project.tests().par_iter().try_for_each(
                |(_, test)| -> Result<(), Option<anyhow::Error>> {
                    match ctx.test(test).run() {
                        Ok(Ok(_)) => {
                            compiled.fetch_add(1, Ordering::SeqCst);
                            maybe_increment(&compared);
                            maybe_increment(&updated);

                            if !summary_only {
                                reporter
                                    .lock()
                                    .unwrap()
                                    .test_success(test, done_annot)
                                    .map_err(|e| Some(e.into()))?;
                            }
                            Ok(())
                        }
                        Ok(Err(err)) => {
                            if err.stage() > Stage::Compilation {
                                compiled.fetch_add(1, Ordering::SeqCst);
                            }

                            if err.stage() > Stage::Comparison {
                                maybe_increment(&compared);
                            }

                            if err.stage() > Stage::Update {
                                maybe_increment(&updated);
                            }

                            if !summary_only {
                                reporter
                                    .lock()
                                    .unwrap()
                                    .test_failure(test, err)
                                    .map_err(|e| Some(e.into()))?;
                            }
                            if ctx.fail_fast() {
                                Err(None)
                            } else {
                                Ok(())
                            }
                        }
                        Err(err) => Err(Some(err.into())),
                    }
                },
            );

            if let Err(Some(err)) = res {
                return Err(err);
            }

            ctx.cleanup()?;
            Ok(start.elapsed())
        })?;

        if !summary_only {
            writeln!(reporter)?;
        }

        let summary = Summary {
            total: pre_filter,
            filtered: pre_filter - post_filter,
            compiled: compiled.into_inner(),
            compared: compared.map(AtomicUsize::into_inner),
            updated: updated.map(AtomicUsize::into_inner),
            time,
        };

        let is_ok = summary.is_ok();
        reporter.test_summary(summary, update, summary_only)?;

        Ok(if is_ok {
            CliResult::Ok
        } else {
            CliResult::TestFailure
        })
    }
}
