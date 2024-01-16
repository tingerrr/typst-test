use std::collections::HashSet;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::{fs, io};

use clap::{ColorChoice, Parser};
use project::fs::Fs;
use project::test::context::ContextResult;
use project::test::Test;
use project::ScaffoldMode;
use rayon::prelude::*;
use report::Reporter;
use tracing::Level;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::prelude::*;
use tracing_tree::HierarchicalLayer;

use self::project::test::context::Context;
use self::project::Project;

mod cli;
mod project;
mod report;
mod util;

fn run(
    mut reporter: Reporter,
    project: &Project,
    fs: &Fs,
    typst: PathBuf,
    fail_fast: bool,
    compare: bool,
) -> anyhow::Result<()> {
    if project.tests().is_empty() {
        reporter.raw(|w| writeln!(w, "No tests detected for {}", project.name()))?;
        return Ok(());
    }

    // TODO: fail_fast currently doesn't really do anything other than returning early, other tests
    //       still run, this makes sense as we're not stopping the other threads just yet
    let ctx = Context::new(project, fs, typst, fail_fast);
    ctx.prepare()?;
    let handles: Vec<_> = project
        .tests()
        .par_iter()
        .map(|test| test.run(&ctx, compare, reporter.clone()))
        .collect();

    // NOTE: inner result ignored as it is reported anyway, see above
    let _ = handles.into_iter().collect::<ContextResult>()?;
    ctx.cleanup()?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
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

    let root = if let Some(root) = args.root {
        let canonical_root = fs::canonicalize(&root)?;
        if !project::fs::is_project_root(&canonical_root)? {
            tracing::warn!("project root doesn't contain manifest");
        }
        root.to_path_buf()
    } else {
        let pwd = std::env::current_dir()?;
        if let Some(root) = project::fs::try_find_project_root(&pwd)? {
            root.to_path_buf()
        } else {
            anyhow::bail!("must be inside a typst project or pass the project root using --root");
        }
    };

    let manifest = project::fs::try_open_manifest(&root)?;
    let mut project = Project::new(manifest);
    let reporter = Reporter::new(util::term::color_stream(args.color, false));
    let fs = Fs::new(root, "tests".into(), reporter.clone());

    let filter_tests = |tests: &mut HashSet<Test>, filter, exact| match (filter, exact) {
        (Some(f), true) => {
            tests.retain(|t| t.name() == f);
        }
        (Some(f), false) => {
            tests.retain(|t| t.name().contains(&f));
        }
        (None, true) => {
            tracing::warn!("no filter given, --exact is meaning less");
        }
        (None, false) => {}
    };

    let (test_args, compare) = match args.cmd {
        cli::Command::Init { no_example } => {
            let mode = if no_example {
                ScaffoldMode::NoExample
            } else {
                ScaffoldMode::WithExample
            };

            if fs.init(mode)? {
                println!("initialized tests for {}", project.name());
            } else {
                println!(
                    "could not initialize tests for {}, {:?} already exists",
                    project.name(),
                    fs.tests_root_dir()
                );
            }
            return Ok(());
        }
        cli::Command::Uninit => {
            fs.uninit()?;
            println!("removed tests for {}", project.name());
            return Ok(());
        }
        cli::Command::Clean => {
            fs.clean_artifacts()?;
            println!("removed test artifacts for {}", project.name());
            return Ok(());
        }
        cli::Command::Add { open, test } => {
            let test = Test::new(test);
            fs.add_test(&test)?;
            reporter.test_success(test.name(), "added")?;

            if open {
                // BUG: this may fail silently if path doesn exist
                open::that_detached(fs.test_file(&test))?;
            }

            return Ok(());
        }
        cli::Command::Edit { test } => {
            let test = fs.find_test(&test)?;
            open::that_detached(fs.test_file(&test))?;
            return Ok(());
        }
        cli::Command::Remove { test } => {
            let test = fs.find_test(&test)?;
            fs.remove_test(test.name())?;
            reporter.test_success(test.name(), "removed")?;
            return Ok(());
        }
        cli::Command::Status => {
            project.add_tests(fs.load_tests()?);
            let tests = project.tests();

            if let Some(manifest) = project.manifest() {
                println!(
                    "Package: {}:{}",
                    manifest.package.name, manifest.package.version
                );

                // TODO: list [tool.typst-test] settings
            }

            if tests.is_empty() {
                println!("Tests: none");
            } else {
                println!("Tests:");
                for test in tests {
                    println!("  {}", test.name());
                }
            }

            return Ok(());
        }
        cli::Command::Update { test_filter, exact } => {
            let mut tests = fs.load_tests()?;
            filter_tests(&mut tests, test_filter, exact);
            project.add_tests(tests);
            reporter.set_padding(project.tests().iter().map(|t| t.name().len()).max());

            run(reporter.clone(), &project, &fs, args.typst, true, false)?;

            let tests = project.tests();
            fs.update_tests(tests.par_iter())?;
            return Ok(());
        }
        cli::Command::Compile(args) => (args, false),
        cli::Command::Run(args) => (args, true),
    };

    let mut tests = fs.load_tests()?;
    filter_tests(&mut tests, test_args.test_filter, test_args.exact);
    project.add_tests(tests);
    reporter.set_padding(project.tests().iter().map(|t| t.name().len()).max());

    run(
        reporter.clone(),
        &project,
        &fs,
        args.typst,
        test_args.fail_fast,
        compare,
    )
}
