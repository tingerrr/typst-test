use std::collections::HashSet;
use std::ffi::OsStr;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::{fs, io};

use clap::{ColorChoice, Parser};
use project::fs::Fs;
use project::test::context::ContextResult;
use project::test::Test;
use project::ScaffoldMode;
use rayon::prelude::*;
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

fn run<W: termcolor::WriteColor>(
    w: &mut W,
    project: &Project,
    fs: &Fs,
    typst: PathBuf,
    fail_fast: bool,
    compare: bool,
) -> anyhow::Result<()> {
    // TODO: fail_fast currently doesn't really do anything other than returning early, other tests
    //       still run, this makes sense as we're not stopping the other threads just yet
    let ctx = Context::new(&project, &fs, typst, fail_fast);

    ctx.prepare()?;
    let handles: Vec<_> = project
        .tests()
        .par_iter()
        .map(|test| test.run(&ctx, compare))
        .collect();

    // NOTE: inner result ignored as it is registered anyway, see above
    let _ = handles.into_iter().collect::<ContextResult>()?;
    ctx.cleanup()?;

    let results = ctx.results().clone();
    let max_name_len = results
        .iter()
        .map(|(t, _)| t.name().len())
        .max()
        .unwrap_or_default();

    if !results.is_empty() {
        for (test, res) in results {
            match res {
                Ok(_) => report::test_success(w, max_name_len, test.name())?,
                Err(err) => report::test_failure(w, max_name_len, test.name(), err)?,
            }
        }
    } else {
        writeln!(w, "No tests detected for {}", project.name())?;
    }

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

    // TODO: read manifest to get project name
    let (root, canonical_root) = if let Some(root) = args.root {
        let canonical_root = fs::canonicalize(&root)?;
        if !project::fs::is_project_root(&canonical_root)? {
            tracing::warn!("project root doesn't contain typst.toml");
        }
        (root.to_path_buf(), canonical_root)
    } else {
        let pwd = std::env::current_dir()?;
        if let Some(root) = project::fs::try_find_project_root(&pwd)? {
            let canonical_root = fs::canonicalize(&root)?;
            (root, canonical_root)
        } else {
            anyhow::bail!("must be inside a typst project or pass the project root using --root");
        }
    };

    let name = canonical_root
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("<unknown project name>")
        .to_owned();

    let mut project = Project::new(name);
    let fs = Fs::new(root);

    let mut stream = util::term::color_stream(args.color, false);

    let filter_tests = |tests: &mut HashSet<Test>, filter, exact| match (filter, exact) {
        (Some(f), true) => {
            tests.retain(|t| t.name() == f);
        }
        (Some(f), false) => {
            tests.retain(|t| t.name().contains(&f));
        }
        (None, _) => {
            tracing::warn!("no filter given, --exact is meaning less");
        }
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
                    fs.test_dir()
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
        cli::Command::Add { folder, open, test } => {
            let test = Test::new(test, folder);
            fs.add_test(&test)?;
            report::test_added(&mut stream, test.name())?;

            if open {
                // BUG: this may fail silently if path doesn exist
                open::that_detached(fs.resolve_script_path(&test))?;
            }

            return Ok(());
        }
        cli::Command::Edit { test } => {
            let test = fs.find_test(&test)?;
            open::that_detached(fs.resolve_script_path(&test))?;
            return Ok(());
        }
        cli::Command::Remove { test } => {
            let test = fs.find_test(&test)?;
            fs.remove_test(test.name())?;
            report::test_removed(&mut stream, test.name())?;
            return Ok(());
        }
        cli::Command::Status => {
            project.add_tests(fs.load_tests()?);
            let tests = project.tests();
            if tests.is_empty() {
                println!("No tests detected for {}", project.name());
            } else {
                println!("Tests detected for {}:", project.name());
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

            run(&mut stream, &project, &fs, args.typst, true, false)?;
            let tests = project.tests();

            fs.update_tests(tests.par_iter())?;

            let max_name_len = tests
                .iter()
                .map(|t| t.name().len())
                .max()
                .unwrap_or_default();

            for test in tests {
                report::test_updated(&mut stream, max_name_len, test.name())?;
            }
            return Ok(());
        }
        cli::Command::Compile(args) => (args, false),
        cli::Command::Run(args) => (args, true),
    };

    let mut tests = fs.load_tests()?;
    filter_tests(&mut tests, test_args.test_filter, test_args.exact);
    project.add_tests(tests);

    run(
        &mut stream,
        &project,
        &fs,
        args.typst,
        test_args.fail_fast,
        compare,
    )
}
