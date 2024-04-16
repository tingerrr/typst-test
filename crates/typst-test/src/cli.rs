use std::fmt::Display;
use std::path::PathBuf;

use clap::ColorChoice;

#[repr(u8)]
pub enum CliResult {
    /// Typst-test ran succesfully.
    Ok = EXIT_OK,

    /// At least one test failed.
    TestFailure = EXIT_TEST_FAILURE,

    /// The requested operation failed gracefully.
    OperationFailure {
        message: Box<dyn Display>,
        hint: Option<Box<dyn Display>>,
    } = EXIT_OPERATION_FAILURE,
}

impl CliResult {
    pub fn operation_failure<M>(message: M) -> Self
    where
        M: Display + 'static,
    {
        Self::OperationFailure {
            message: Box::new(message) as _,
            hint: None,
        }
    }

    pub fn hinted_operation_failure<M, H>(message: M, hint: H) -> Self
    where
        M: Display + 'static,
        H: Display + 'static,
    {
        Self::OperationFailure {
            message: Box::new(message) as _,
            hint: Some(Box::new(hint) as _),
        }
    }
}

/// Typst-test ran succesfully.
pub const EXIT_OK: u8 = 0;

/// At least one test failed.
pub const EXIT_TEST_FAILURE: u8 = 1;

/// The requested operation failed gracefully.
pub const EXIT_OPERATION_FAILURE: u8 = 2;

/// An unexpected error occured.
pub const EXIT_ERROR: u8 = 3;

macro_rules! ansi {
    ($s:expr; b) => {
        concat!("\x1B[1m", $s, "\x1B[0m")
    };
    ($s:expr; u) => {
        concat!("\x1B[4m", $s, "\x1B[0m")
    };
    ($s:expr;) => {
        $s
    };
    ($s:expr; $first:ident $( + $rest:tt)*) => {
        ansi!(ansi!($s; $($rest)*); $first)
    };
}

// NOTE: we use clap style formatting here and keep it simple to avoid a proc macro dependency for
// a single use of static ansi formatting
#[rustfmt::skip]
static AFTER_LONG_ABOUT: &str = concat!(
    ansi!("Exit Codes:\n"; u + b),
    "  ", ansi!("0"; b), "  Success\n",
    "  ", ansi!("1"; b), "  At least one test failed\n",
    "  ", ansi!("2"; b), "  The requested operation failed\n",
    "  ", ansi!("3"; b), "  An unexpected error occured",
);

/// Execute, compare and update visual regression tests for typst
#[derive(clap::Parser, Debug)]
#[clap(after_long_help = AFTER_LONG_ABOUT)]
pub struct Args {
    /// The project root directory
    #[arg(long, global = true)]
    pub root: Option<PathBuf>,

    /// A path to the typst binary to execute the tests with
    #[arg(long, global = true, default_value = "typst")]
    pub typst: PathBuf,

    /// Whether to abort after the first test failure
    ///
    /// Keep in mind that because tests are run in parallel, this may not stop
    /// immediately. But it will not schedule any new tests to run after one
    /// failure has been detected.
    #[arg(long, global = true)]
    pub fail_fast: bool,

    /// When to use colorful output
    ///
    /// If set to auto, color will only be enabled if a capable terminal is
    /// detected.
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

    /// Produce more logging output [-v .. -vvvvv]
    ///
    /// Logs are written to stderr, the increasing number of verbose flags
    /// corresponds to the log levels ERROR, WARN, INFO, DEBUG, TRACE.
    #[arg(long, short, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    #[command(subcommand)]
    pub cmd: Command,
}

#[derive(clap::Subcommand, Debug, Clone)]
pub enum Command {
    /// Initialize the current project with a test directory
    Init {
        /// Do not create a default example test
        #[arg(long)]
        no_example: bool,

        /// Do not create a default .ignore file
        #[arg(long)]
        no_ignore: bool,

        /// Do not create a default .gitignore file
        #[arg(long)]
        no_gitignore: bool,
    },

    /// Remove the test directory from the current project
    Uninit,

    /// Remove test output artifacts
    Clean,

    /// Show information about the current project
    #[command(alias = "st")]
    Status,

    /// List the tests in the current project
    #[command(alias = "ls")]
    List,

    /// Compile and compare tests
    #[command(alias = "r")]
    Run(RunnerArgs),

    /// Compile tests
    #[command(alias = "c")]
    Compile(RunnerArgs),

    /// Update tests
    #[command(alias = "u")]
    Update {
        /// Whether reference images should not be optimized for size
        ///
        /// This will significantly speed up the updating process, but images
        /// may be much larger than needed, increasing repository bloat.
        #[arg(long)]
        no_optimize: bool,

        #[command(flatten)]
        runner_args: RunnerArgs,
    },

    /// Add a new test
    ///
    /// The default test simply contains `Hello World`, if a
    /// `tests/template.typ` file is given, it is used instead.
    #[command(alias = "a")]
    Add {
        /// Whether to open the test script
        #[arg(long, short)]
        open: bool,

        /// The name of the test to add
        test: String,
    },

    /// Edit an existing new test
    #[command(alias = "e")]
    Edit {
        /// The name of the test to edit
        test: String,
    },

    /// Remove a test
    #[command(alias = "rm")]
    Remove {
        /// The name of the test to remove
        test: String,
    },
}

#[derive(clap::Parser, Debug, Clone)]
pub struct RunnerArgs {
    /// Show a summary of the test run instread of the individual test results
    #[arg(long, global = true)]
    pub summary: bool,

    #[command(flatten)]
    pub filter: TestFilter,
}

#[derive(clap::Parser, Debug, Clone)]
pub struct TestFilter {
    /// Whether the test filter should be an exact match
    #[arg(long, short)]
    pub exact: bool,

    /// A filter for which tests to run, any test matching this filter is
    /// run
    pub filter: Option<String>,
}
