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

    /// Whether to interactively ask for updates when tests change
    #[arg(long, short, global = true)]
    pub interactive: bool,

    /// The sub command to execute
    #[command(subcommand)]
    pub cmd: Option<Command>,
}

#[derive(clap::Subcommand, Debug, Clone)]
pub enum Command {
    /// Initialize a tests directory for the current project
    Init,

    /// Remove the tests directory from the current project
    Uninit,

    /// Remove test output and temporary artifacts
    Clean,

    /// Show informaiton about the current project's tests
    Status,

    /// Compile and compare tests
    Run,

    /// Compile tests
    Compile,

    /// Compare tests
    Compare,

    /// Update tests
    Update,
}
