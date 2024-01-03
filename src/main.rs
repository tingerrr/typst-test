use std::path::PathBuf;
use std::{fs, thread};

use clap::Parser;
use project::test::context::ContextResult;
use project::ScaffoldMode;
use tracing::Level;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::prelude::*;
use tracing_tree::HierarchicalLayer;

use self::project::test::context::Context;
use self::project::Project;
use crate::project::test::{CompareFailure, TestFailure};

mod cli;
mod project;
mod util;

fn run(
    mut project: Project,
    typst: PathBuf,
    fail_fast: bool,
    compare: bool,
    filter: Option<String>,
) -> anyhow::Result<()> {
    project.load_tests()?;

    // TODO: fail_fast currently doesn't really do anything other than returning early, other tests
    //       still run, this makes sense as we're not stopping the other threads just yet
    let ctx = Context::new(project.clone(), typst, fail_fast);

    // wow rust makes this so easy
    // TODO: inner result ignored as it is registered anyway, see above
    let _ = thread::scope(|scope| {
        let filter = filter.as_deref().unwrap_or_default();

        let handles: Vec<_> = project
            .tests()
            .into_iter()
            .filter(|test| test.name().contains(filter))
            .map(|test| scope.spawn(|| test.run(&ctx, compare)))
            .collect();

        handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect::<ContextResult>()
    })?;

    let present_ok = |n: &str| {
        println!("{}: success", n);
    };

    // removing the type hint makes causes the first usage to infer a longer lifetime than the
    // latter usage can satisfy
    let present_err = |n: &str, e| {
        println!("{}: failed", n);

        match e {
            TestFailure::Preparation(e) => println!("  {}", e),
            TestFailure::Cleanup(e) => println!("  {}", e),
            TestFailure::Compilation(e) => {
                let present_buffer = |buffer: Vec<u8>| {
                    if buffer.is_empty() {
                        return;
                    }

                    if let Ok(s) = std::str::from_utf8(&buffer) {
                        for line in s.lines() {
                            println!("    {line}");
                        }
                    } else {
                        println!("    buffer was not valid utf8:");
                        println!("    {buffer:?}");
                    }
                };

                println!("  compilation failed");
                present_buffer(e.output.stdout);
                present_buffer(e.output.stderr);
            }
            TestFailure::Comparison(CompareFailure::PageCount { output, reference }) => {
                println!(
                    "  expected {} page{}, got {} page{}",
                    reference,
                    if reference == 1 { "" } else { "s" },
                    output,
                    if output == 1 { "" } else { "s" },
                );
            }
            TestFailure::Comparison(CompareFailure::Page { pages }) => {
                for p in pages {
                    println!("  page {}: {}", p.0, p.1);
                }
            }
        }
    };

    for (test, res) in ctx.results().clone() {
        println!();

        match res {
            Ok(_) => present_ok(test.name()),
            Err(e) => {
                present_err(test.name(), e);
            }
        }
    }

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args = cli::Args::parse();

    anyhow::ensure!(!args.interactive, "interactive mode is not yet implemented");

    tracing_subscriber::registry()
        .with(HierarchicalLayer::new(4).with_targets(true))
        .with(Targets::new().with_target(std::env!("CARGO_CRATE_NAME"), Level::TRACE))
        .init();

    let root = if let Some(root) = args.root {
        let root = fs::canonicalize(root)?;
        anyhow::ensure!(
            project::is_project_root(&root)?,
            "--root must contain a typst.toml manifest file",
        );
        root
    } else {
        let pwd = std::env::current_dir()?;
        if let Some(root) = project::try_find_project_root(&pwd)? {
            root
        } else {
            anyhow::bail!("must be inside a typst project or pass the project root using --root");
        }
    };

    let mut project = Project::new(root);

    let (test, compare) = match args.cmd {
        cli::Command::Init => {
            project::try_create_tests_scaffold(project.root(), ScaffoldMode::WithExample)?;
            return Ok(());
        }
        cli::Command::Uninit => {
            project::try_remove_tests_scaffold(project.root())?;
            return Ok(());
        }
        cli::Command::Clean => {
            util::fs::ensure_remove_dir(project::test_out_dir(project.root()), true)?;
            util::fs::ensure_remove_dir(project::test_diff_dir(project.root()), true)?;
            return Ok(());
        }
        cli::Command::Status => {
            project.load_tests()?;
            let tests = project.tests();
            if tests.is_empty() {
                println!("No tests detected");
            } else {
                println!("Tests detected:");
                for test in tests {
                    println!("  {}", test.name());
                }
            }

            return Ok(());
        }
        cli::Command::Update(args) => {
            project.load_tests()?;
            project.update_tests(args.test)?;
            return Ok(());
        }
        cli::Command::Compile(args) => (args.test, false),
        cli::Command::Run(args) => (args.test, true),
    };

    run(project, args.typst, args.fail_fast, compare, test)
}
