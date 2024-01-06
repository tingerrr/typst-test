use std::path::PathBuf;

use clap::ColorChoice;

/// Execute, compare and update visual regression tests for typst
#[derive(clap::Parser, Debug)]
pub struct Args {
    /// The project root directory
    #[arg(long, global = true)]
    pub root: Option<PathBuf>,

    /// A path to the typst binary to execute the tests with
    #[arg(long, global = true, default_value = "typst")]
    pub typst: PathBuf,

    /// Whether to abort after the first test failure
    #[arg(long, global = true)]
    pub fail_fast: bool,

    /// When to use colorful output
    /// auto = use color if a capable terminal is detected
    #[clap(
        long,
        global = true,
        value_name = "WHEN",
        require_equals = true,
        num_args = 0..=1,
        default_value = "auto",
        default_missing_value = "always",
    )]
    pub color: ColorChoice,

    /// Produce more logging output [-v .. -vvvvv], logs are written to stderr
    #[arg(long, short, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    #[command(subcommand)]
    pub cmd: Command,
}

#[derive(clap::Subcommand, Debug, Clone)]
pub enum Command {
    /// Initialize the current project with a test directory
    Init {
        /// Do not create a default example
        #[arg(long)]
        no_example: bool,
    },

    /// Remove the test directory from the current project
    Uninit,

    /// Remove test output artifacts
    Clean,

    /// Show information about the current project
    Status,

    /// Compile and compare tests
    Run(TestArgs),

    /// Compile tests
    Compile(TestArgs),

    /// Update tests
    Update(TestArgs),
}

#[derive(clap::Parser, Debug, Clone)]
pub struct TestArgs {
    /// A filter for which tests to run, any test containing this string is run
    pub test: Option<String>,
}
