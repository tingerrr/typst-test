use std::path::PathBuf;

/// Execute, compare and update test scripts for typst
#[derive(clap::Parser, Debug)]
pub struct Args {
    /// The project root directory containing the typst.toml manifest file
    #[arg(long, global = true)]
    pub root: Option<PathBuf>,

    /// A path to the typst binary to execute the tests with
    #[arg(long, global = true, default_value = "typst")]
    pub typst: PathBuf,

    /// Whether to typst-test should abort after the first test failure
    #[arg(long, global = true)]
    pub fail_fast: bool,

    /// Produce more logging output [-v .. -vvvvv], logs are written to stderr
    #[arg(long, short, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// The sub command to execute
    #[command(subcommand)]
    pub cmd: Command,
}

#[derive(clap::Subcommand, Debug, Clone)]
pub enum Command {
    /// Initialize a tests directory for the current project
    Init {
        /// Do not create a default example
        #[arg(long)]
        no_example: bool,
    },

    /// Remove the tests directory from the current project
    Uninit,

    /// Remove test output and temporary artifacts
    Clean,

    /// Show informaiton about the current project's tests
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
    /// The a filter for which tests to run
    /// Tests containing this substring are run
    pub test: Option<String>,
}
