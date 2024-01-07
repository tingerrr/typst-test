use std::ffi::OsStr;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::{fs, io};

use clap::{ColorChoice, Parser};
use project::fs::Fs;
use project::test::context::ContextResult;
use project::ScaffoldMode;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use termcolor::Color;
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

fn run<W: termcolor::WriteColor>(
    w: &mut W,
    project: &Project,
    fs: &Fs,
    typst: PathBuf,
    fail_fast: bool,
    compare: bool,
    filter: Option<String>,
) -> anyhow::Result<()> {
    // TODO: fail_fast currently doesn't really do anything other than returning early, other tests
    //       still run, this makes sense as we're not stopping the other threads just yet
    let ctx = Context::new(&project, &fs, typst, fail_fast);

    let filter = filter.as_deref().unwrap_or_default();
    ctx.prepare()?;
    let handles: Vec<_> = project
        .tests()
        .par_iter()
        .filter(|test| test.name().contains(filter))
        .map(|test| test.run(&ctx, compare))
        .collect();

    // NOTE: inner result ignored as it is registered anyway, see above
    let _ = handles.into_iter().collect::<ContextResult>()?;
    ctx.cleanup()?;

    let results = ctx.results().clone();
    let pad = results
        .iter()
        .map(|(t, _)| t.name().len())
        .max()
        .unwrap_or_default();

    let present_ok = |w: &mut dyn termcolor::WriteColor, n: &str| -> io::Result<()> {
        write!(w, "{n:<pad$} ")?;
        util::term::with_color(
            w,
            |c| c.set_bold(true).set_fg(Some(Color::Green)),
            format_args!("success"),
        )?;
        writeln!(w)
    };

    // NOTE: removing the type hint makes causes the first usage to infer a longer lifetime than the
    //       latter usage can satisfy

    // TODO: long test names will cause compile errors to become harde to read, perhaps they should
    //       be shown above, i.e by bounding the maxiumum allowd padding

    let present_err = |w: &mut dyn termcolor::WriteColor, n: &str, e| -> io::Result<()> {
        write!(w, "{n:<pad$} ")?;
        util::term::with_color(
            w,
            |c| c.set_bold(true).set_fg(Some(Color::Red)),
            format_args!("failed\n"),
        )?;

        let pad = " ".repeat(pad + 1);
        match e {
            TestFailure::Preparation(e) => writeln!(w, "{pad}{e}")?,
            TestFailure::Cleanup(e) => writeln!(w, "{pad}{e}")?,
            TestFailure::Compilation(e) => {
                let present_buffer = |w: &mut dyn termcolor::WriteColor,
                                      buffer: &[u8],
                                      name: &str|
                 -> io::Result<()> {
                    if buffer.is_empty() {
                        return Ok(());
                    }

                    if let Ok(s) = std::str::from_utf8(buffer) {
                        util::term::with_color(
                            w,
                            |c| c.set_bold(true),
                            format_args!("{pad}┏━ {name}\n"),
                        )?;
                        for line in s.lines() {
                            util::term::with_color(
                                w,
                                |c| c.set_bold(true),
                                format_args!("{pad}┃"),
                            )?;
                            writeln!(w, "{line}")?;
                        }
                        util::term::with_color(
                            w,
                            |c| c.set_bold(true),
                            format_args!("{pad}┗━ {name}\n"),
                        )?;
                    } else {
                        writeln!(w, "{pad}{name} was not valid utf8:")?;
                        writeln!(w, "{pad}{buffer:?}")?;
                    }

                    Ok(())
                };

                writeln!(w, "{pad}compilation failed")?;
                present_buffer(w, &e.output.stdout, "stdout")?;
                present_buffer(w, &e.output.stderr, "stderr")?;
            }
            TestFailure::Comparison(CompareFailure::PageCount { output, reference }) => {
                writeln!(
                    w,
                    "{pad}expected {reference} page{}, got {output} page{}",
                    if reference == 1 { "" } else { "s" },
                    if output == 1 { "" } else { "s" },
                )?;
            }
            TestFailure::Comparison(CompareFailure::Page { pages }) => {
                for (p, f) in pages {
                    writeln!(w, "{pad}page {p}: {f}")?;
                }
            }
            TestFailure::Comparison(CompareFailure::MissingOutput) => {
                writeln!(w, "{pad}no output generated")?;
            }
            TestFailure::Comparison(CompareFailure::MissingReferences) => {
                writeln!(w, "{pad}no references given")?;
                util::term::with_color(
                    w,
                    |c| c.set_bold(true).set_fg(Some(Color::Cyan)),
                    format_args!("{pad}hint: "),
                )?;
                writeln!(w, "use `typst-test update {n}` to accept the output test")?;
            }
        }

        Ok(())
    };

    if !results.is_empty() {
        for (test, res) in results {
            match res {
                Ok(_) => present_ok(w, test.name())?,
                Err(e) => {
                    present_err(w, test.name(), e)?;
                }
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
        cli::Command::Add { folder, test } => {
            fs.add_test(test.clone(), folder)?;
            println!("added {test}");
            return Ok(());
        }
        cli::Command::Remove { test } => {
            fs.remove_test(test.clone())?;
            println!("removed {test}");
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
        cli::Command::Update { test_filter } => {
            project.add_tests(fs.load_tests()?);
            let tests = test_filter
                .as_deref()
                .map(|f| project.filter_tests(f))
                .unwrap_or_else(|| project.tests().iter().collect());

            run(
                &mut stream,
                &project,
                &fs,
                args.typst,
                true,
                false,
                test_filter.clone(),
            )?;
            fs.update_tests(tests.par_iter().map(|t| *t))?;
            for test in tests {
                println!("updated {}", test.name());
            }
            return Ok(());
        }
        cli::Command::Compile(args) => (args, false),
        cli::Command::Run(args) => (args, true),
    };

    project.add_tests(fs.load_tests()?);
    run(
        &mut stream,
        &project,
        &fs,
        args.typst,
        test_args.fail_fast,
        compare,
        test_args.test_filter,
    )
}
