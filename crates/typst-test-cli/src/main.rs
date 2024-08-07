use std::io;
use std::io::{IsTerminal, Write};
use std::process::ExitCode;

use clap::{ColorChoice, Parser};
use tracing::Level;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::prelude::*;
use tracing_tree::HierarchicalLayer;
use ui::Ui;

use crate::cli::Context;
use crate::report::Reporter;

mod cli;
mod download;
mod fonts;
mod package;
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
                .warning("at least 2 threads are needed, using 2")
                .unwrap();
            2
        } else {
            jobs
        };

        rayon::ThreadPoolBuilder::new()
            .num_threads(jobs)
            .build_global()
            .ok();
    }

    let mut ctx = Context::new(&args, reporter);

    match ctx.run() {
        Ok(()) => {}
        Err(_) if ctx.is_operation_failure() => {}
        Err(err) => 'err: {
            let root = err.root_cause();

            // FIXME: https://github.com/serde-rs/json/issues/1169
            // NOTE: we can't access the inner io error itself, but at least the
            // kind
            if root.downcast_ref().is_some_and(|err: &serde_json::Error| {
                err.io_error_kind()
                    .is_some_and(|kind| kind == io::ErrorKind::BrokenPipe)
            }) {
                break 'err;
            }

            // NOTE: we ignore broken pipes as these occur when programs close
            // the pipe before we're done writing
            if root
                .downcast_ref()
                .is_some_and(|err: &io::Error| err.kind() == io::ErrorKind::BrokenPipe)
            {
                break 'err;
            }

            ctx.unexpected_error(|r| {
                r.ui().error_with(|w| {
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
                    r.ui().hint_with(|w| {
                        writeln!(
                            w,
                            "consider running with the environment variable RUST_BACKTRACE set to 'full' when reporting issues",
                        )?;
                        writeln!(w)
                    })?;
                }
                r.ui().error_with(|w| writeln!(w, "{err:?}"))
            })
            .unwrap();
        }
    };

    ctx.exit()
}
