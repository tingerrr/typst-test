use std::io;
use std::io::{IsTerminal, Write};
use std::process::ExitCode;

use clap::{ColorChoice, Parser};
use error::{OperationFailure, TestFailure};
use tracing::Level;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::prelude::*;
use tracing_tree::HierarchicalLayer;
use ui::Ui;

use crate::cli::Context;
use crate::report::Reporter;

mod cli;
mod error;
mod kit;
mod project;
mod report;
mod test;
mod ui;
mod world;

fn is_color(color: clap::ColorChoice, is_stderr: bool) -> bool {
    match color {
        clap::ColorChoice::Auto => {
            if is_stderr {
                io::stderr().is_terminal()
            } else {
                io::stdout().is_terminal()
            }
        }
        clap::ColorChoice::Always => true,
        clap::ColorChoice::Never => false,
    }
}

const IS_OUTPUT_STDERR: bool = false;

fn main() -> ExitCode {
    match main_impl() {
        Ok(code) => code,
        Err(err) => {
            eprintln!("{err:?}");
            ExitCode::from(cli::EXIT_ERROR)
        }
    }
}

fn main_impl() -> anyhow::Result<ExitCode> {
    let args = cli::Args::parse();

    // BUG: this interferes with the live printing
    if args.global.output.verbose >= 1 {
        tracing_subscriber::registry()
            .with(
                HierarchicalLayer::new(4)
                    .with_targets(true)
                    .with_ansi(is_color(args.global.output.color, IS_OUTPUT_STDERR)),
            )
            .with(Targets::new().with_target(
                std::env!("CARGO_CRATE_NAME"),
                match args.global.output.verbose {
                    1 => Level::ERROR,
                    2 => Level::WARN,
                    3 => Level::INFO,
                    4 => Level::DEBUG,
                    _ => Level::TRACE,
                },
            ))
            .init();
    }

    let cc = match args.global.output.color {
        ColorChoice::Auto => termcolor::ColorChoice::Auto,
        ColorChoice::Always => termcolor::ColorChoice::Always,
        ColorChoice::Never => termcolor::ColorChoice::Never,
    };

    // TODO: simpler output when using plain
    let reporter = Reporter::new(
        Ui::new(cc, cc),
        report::Verbosity::All,
        args.global.output.format,
    );

    if let Some(jobs) = args.global.jobs {
        let jobs = if jobs < 2 {
            reporter
                .ui()
                .warning("at least 2 threads are needed, using 2")?;
            2
        } else {
            jobs
        };

        rayon::ThreadPoolBuilder::new()
            .num_threads(jobs)
            .build_global()
            .ok();
    }

    let mut ctx = Context::new(&args, &reporter);

    let exit_code = match ctx.run() {
        Ok(()) => cli::EXIT_OK,
        Err(err) => 'err: {
            let root = err.root_cause();

            for cause in err.chain() {
                if let Some(TestFailure) = cause.downcast_ref() {
                    break 'err cli::EXIT_TEST_FAILURE;
                }

                if let Some(OperationFailure(err)) = cause.downcast_ref() {
                    err.report(ctx.reporter.ui())?;
                    break 'err cli::EXIT_OPERATION_FAILURE;
                }
            }

            // FIXME: https://github.com/serde-rs/json/issues/1169
            if root
                .downcast_ref()
                .and_then(serde_json::Error::io_error_kind)
                .or_else(|| root.downcast_ref().map(io::Error::kind))
                .is_some_and(|kind| kind == io::ErrorKind::BrokenPipe)
            {
                break 'err cli::EXIT_OK;
            }

            ctx.reporter.ui().error_with(|w| {
                writeln!(
                    w,
                    "typst-test ran into an unexpected error, this is most likely a bug"
                )?;
                writeln!(
                    w,
                    "Please consider reporting this at {}/issues/new",
                    std::env!("CARGO_PKG_REPOSITORY")
                )
            })?;
            if !std::env::var("RUST_BACKTRACE").is_ok_and(|var| var == "full") {
                ctx.reporter.ui().hint_with(|w| {
                    writeln!(
                        w,
                        "consider running with the environment variable RUST_BACKTRACE set to 'full' when reporting issues\n",
                    )
                })?;
            }
            ctx.reporter.ui().error_with(|w| writeln!(w, "{err:?}"))?;

            cli::EXIT_OPERATION_FAILURE
        }
    };

    ctx.reporter.ui().flush()?;

    Ok(ExitCode::from(exit_code))
}
