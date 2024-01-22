use std::io;
use std::io::{IsTerminal, Write};
use std::path::Path;
use std::process::ExitCode;

use clap::{ColorChoice, Parser};
use project::test::Filter;
use termcolor::{Color, ColorSpec, WriteColor};
use tracing::Level;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::prelude::*;
use tracing_tree::HierarchicalLayer;

use self::cli::CliResult;
use self::project::Project;
use self::report::Reporter;

mod cli;
mod project;
mod report;
mod util;

fn main() -> ExitCode {
    let args = cli::Args::parse();

    if args.verbose >= 1 {
        tracing_subscriber::registry()
            .with(
                HierarchicalLayer::new(4)
                    .with_targets(true)
                    .with_ansi(match args.color {
                        ColorChoice::Auto => io::stderr().is_terminal(),
                        ColorChoice::Always => true,
                        ColorChoice::Never => false,
                    }),
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

    let mut reporter = Reporter::new(util::term::color_stream(args.color, false));

    reporter.indent(2);
    let res = main_impl(args, &mut reporter);
    reporter.dedent();

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

            reporter.indent(2);
            reporter
                .set_color(ColorSpec::new().set_bold(true).set_fg(Some(Color::Red)))
                .unwrap();
            write!(reporter, "Error: ").unwrap();
            reporter.indent("Error: ".len() as isize);
            reporter.reset().unwrap();
            writeln!(reporter, "{err}").unwrap();
            reporter.dedent();
            reporter.dedent();

            cli::EXIT_ERROR
        }
    };

    // NOTE: ensure we completely reset the terminal to standard
    reporter.dedent_all();
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

    let manifest = project::try_open_manifest(&root)?;
    let mut project = Project::new(root, Path::new("tests"), manifest);

    let (filter, compare) = match args.cmd {
        cli::Command::Init { no_example } => return cmd::init(&mut project, reporter, no_example),
        cli::Command::Uninit => return cmd::uninit(&mut project, reporter),
        cli::Command::Clean => return cmd::clean(&mut project, reporter),
        cli::Command::Add { open, test } => return cmd::add(&mut project, reporter, test, open),
        cli::Command::Edit { test } => return cmd::edit(&mut project, reporter, test),
        cli::Command::Remove { test } => return cmd::remove(&mut project, reporter, test),
        cli::Command::Status => return cmd::status(&mut project, reporter, args.typst),
        cli::Command::Update {
            filter,
            no_optimize,
        } => {
            return cmd::update(
                &mut project,
                reporter,
                args.typst,
                filter.filter.map(|f| Filter::new(f, filter.exact)),
                args.fail_fast,
                !no_optimize,
            )
        }
        cli::Command::Compile(args) => (args, false),
        cli::Command::Run(args) => (args, true),
    };

    cmd::run(
        &mut project,
        reporter,
        args.typst,
        filter.filter.map(|f| Filter::new(f, filter.exact)),
        args.fail_fast,
        compare,
    )
}

mod cmd {
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;

    use rayon::prelude::*;

    use crate::cli::CliResult;
    use crate::project::test::context::Context;
    use crate::project::test::Filter;
    use crate::project::{Project, ScaffoldMode};
    use crate::report::Reporter;

    macro_rules! bail_gracefully {
        (if_no_typst; $project:expr; $typst:expr) => {
            if let Err(which::Error::CannotFindBinaryPath) = which::which($typst) {
                // TODO: test
                return Ok(CliResult::hinted_operation_failure(
                    "No typst binary found in PATH",
                    "You can pass the typst binary using '--typst <path or name>'",
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
    ) -> anyhow::Result<CliResult> {
        if project.is_init()? {
            return Ok(CliResult::operation_failure(format!(
                "Project '{}' was already initialized",
                project.name(),
            )));
        }

        let mode = if no_example {
            ScaffoldMode::NoExample
        } else {
            ScaffoldMode::WithExample
        };

        project.init(mode)?;
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
            "Removed {} test{}",
            count,
            if count == 1 { "" } else { "s" }
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
            // NOTE: because test form create_test extends the mutable borrow
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
        typst: PathBuf,
        filter: Option<Filter>,
        fail_fast: bool,
        optimize: bool,
    ) -> anyhow::Result<CliResult> {
        run_tests(
            project,
            reporter,
            filter,
            |project| {
                let mut ctx = Context::new(project, typst);
                ctx.with_fail_fast(fail_fast)
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

    pub fn run(
        project: &mut Project,
        reporter: &mut Reporter,
        typst: PathBuf,
        filter: Option<Filter>,
        fail_fast: bool,
        compare: bool,
    ) -> anyhow::Result<CliResult> {
        run_tests(
            project,
            reporter,
            filter,
            |project| {
                let mut ctx = Context::new(project, typst);
                ctx.with_fail_fast(fail_fast).with_compare(compare);
                ctx
            },
            if compare { "ok" } else { "compiled" },
        )
    }

    fn run_tests(
        project: &mut Project,
        reporter: &mut Reporter,
        filter: Option<Filter>,
        prepare_ctx: impl FnOnce(&Project) -> Context,
        done_annot: &str,
    ) -> anyhow::Result<CliResult> {
        bail_gracefully!(if_uninit; project);

        project.discover_tests()?;
        bail_gracefully!(if_no_tests_found; project);
        bail_gracefully!(if_no_tests_match; project; &filter);

        let ctx = prepare_ctx(project);
        bail_gracefully!(if_no_typst; project; ctx.typst());

        ctx.prepare()?;

        let reporter = Mutex::new(reporter);
        let all_ok = AtomicBool::new(true);
        let res = project.tests().par_iter().try_for_each(
            |(_, test)| -> Result<(), Option<anyhow::Error>> {
                match ctx.test(test).run() {
                    Ok(Ok(_)) => {
                        reporter
                            .lock()
                            .unwrap()
                            .test_success(test, done_annot)
                            .map_err(|e| Some(e.into()))?;
                        Ok(())
                    }
                    Ok(Err(err)) => {
                        all_ok.store(false, Ordering::Relaxed);
                        reporter
                            .lock()
                            .unwrap()
                            .test_failure(test, err)
                            .map_err(|e| Some(e.into()))?;
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

        Ok(if all_ok.into_inner() {
            CliResult::Ok
        } else {
            CliResult::TestFailure
        })
    }
}
