use std::io::{ErrorKind, Write};
use std::process::ExitCode;

use clap::{ColorChoice, Parser};
use termcolor::{Color, WriteColor};
use tracing::Level;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::prelude::*;
use tracing_tree::HierarchicalLayer;
use typst_test_lib::config::Config;

use self::cli::CliResult;
use self::project::Project;
use self::report::Reporter;

mod cli;
mod fonts;
mod project;
mod report;
mod test;
mod util;

const IS_OUTPUT_STDERR: bool = false;

fn main() -> ExitCode {
    let mut args = cli::Args::parse();

    if !args.global.format.is_pretty() {
        args.global.color = ColorChoice::Never;
    }

    if args.global.verbose >= 1 {
        tracing_subscriber::registry()
            .with(
                HierarchicalLayer::new(4)
                    .with_targets(true)
                    .with_ansi(util::term::color(args.global.color, IS_OUTPUT_STDERR)),
            )
            .with(Targets::new().with_target(
                std::env!("CARGO_CRATE_NAME"),
                match args.global.verbose {
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
        util::term::color_stream(args.global.color, IS_OUTPUT_STDERR),
        args.global.format,
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
    let root = match &args.global.root {
        Some(root) => root.to_path_buf(),
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

    // TODO: util commands don't need project
    let mut project = Project::new(
        root,
        config.or(manifest_config).unwrap_or_default(),
        manifest,
    );

    let ctx = cli::Context {
        project: &mut project,
        reporter,
    };

    args.cmd.run(ctx, &args.global)
}
