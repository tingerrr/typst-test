use std::io;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};

use clap::ColorChoice;
use termcolor::{Color, WriteColor};
use typst_test_lib::config::Config;
use typst_test_lib::test::id::Identifier;
use typst_test_lib::test_set;
use typst_test_lib::test_set::{DynTestSet, TestSetExpr};

use crate::fonts::FontSearcher;
use crate::project;
use crate::project::Project;
use crate::report::Reporter;

pub mod add;
pub mod compare;
pub mod compile;
pub mod edit;
pub mod init;
pub mod list;
pub mod remove;
pub mod run;
pub mod status;
pub mod uninit;
pub mod update;
pub mod util;

/// The separator used for multiple paths.
const ENV_PATH_SEP: char = if cfg!(windows) { ';' } else { ':' };

/// Typst-test exited successfully.
pub const EXIT_OK: u8 = 0;

/// At least one test failed.
pub const EXIT_TEST_FAILURE: u8 = 1;

/// The requested operation failed gracefully.
pub const EXIT_OPERATION_FAILURE: u8 = 2;

/// An unexpected error occured.
pub const EXIT_ERROR: u8 = 3;

pub struct Context<'a> {
    pub args: &'a Args,
    pub reporter: Arc<Mutex<Reporter>>,
    exit_code: u8,
}

impl<'a> Context<'a> {
    pub fn new(args: &'a Args, reporter: Reporter) -> Self {
        Self {
            args,
            reporter: Arc::new(Mutex::new(reporter)),
            exit_code: EXIT_OK,
        }
    }

    pub fn ensure_project(&mut self) -> anyhow::Result<Project> {
        let root = match &self.args.global.root {
            Some(root) => root.to_path_buf(),
            None => {
                let pwd = std::env::current_dir()?;
                match typst_project::try_find_project_root(&pwd)? {
                    Some(root) => root.to_path_buf(),
                    None => {
                        self.operation_failure(|r| {
                            writeln!(r, "Must be inside a typst project")?;
                            r.hint("You can pass the project root using '--root <path>'")?;
                            Ok(())
                        })?;
                        anyhow::bail!("No project");
                    }
                }
            }
        };

        if !root.try_exists()? {
            self.operation_failure(|r| {
                writeln!(r, "Root '{}' directory not found", root.display())
            })?;
            anyhow::bail!("Root not found");
        }

        let manifest = match project::try_open_manifest(&root) {
            Ok(manifest) => manifest,
            Err(project::Error::InvalidManifest(err)) => {
                self.reporter.lock().unwrap().write_annotated(
                    "warning:",
                    Color::Yellow,
                    None,
                    |this| {
                        tracing::error!(?err, "Couldn't parse manifest");
                        writeln!(this, "Error while parsing manifest, skipping")?;
                        writeln!(this, "{}", err.message())
                    },
                )?;

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

        Ok(Project::new(
            root,
            manifest_config.unwrap_or_default(),
            manifest,
        ))
    }

    pub fn ensure_init(&mut self) -> anyhow::Result<Project> {
        let project = self.ensure_project()?;

        if !project.is_init()? {
            self.set_operation_failure();
            let mut reporter = self.reporter.lock().unwrap();
            reporter.operation_failure(|r| {
                write!(r, "Project '{}' was not initialized", project.name())
            })?;
            anyhow::bail!("Project was not initalized");
        }

        Ok(project)
    }

    pub fn collect_tests(
        &mut self,
        op_args: &OperationArgs,
        op_requires_confirm_for_many: impl Into<Option<&'static str>>,
    ) -> anyhow::Result<Project> {
        let mut project = self.ensure_init()?;

        let test_set = match op_args.test_set() {
            Ok(test_set) => test_set,
            Err(err) => {
                self.set_operation_failure();
                self.operation_failure(|r| {
                    writeln!(r, "Couldn't parse test set expression:\n{err}")
                })?;
                anyhow::bail!(err);
            }
        };

        project.collect_tests(test_set)?;

        match (project.matched().len(), op_requires_confirm_for_many.into()) {
            (0, _) => {
                self.set_operation_failure();
                self.operation_failure(|r| writeln!(r, "Matched no tests"))?;
                anyhow::bail!("Matched no tests");
            }
            (1, _) => {}
            (_, None) => {}
            // Explicitly passing more than one test implies `--all`
            (_, Some(_)) if op_args.all || !op_args.tests.is_empty() => {}
            (_, Some(op)) => {
                self.operation_failure(|r| {
                    writeln!(r, "Matched more than one test")?;
                    r.hint(format!("Pass `--all` to {op} more than one test at a time"))
                })?;

                anyhow::bail!(
                    "Matched more than one test without a confirmation for operation {op}"
                );
            }
        }

        Ok(project)
    }

    fn set_operation_failure(&mut self) {
        self.exit_code = EXIT_OPERATION_FAILURE;
    }

    pub fn operation_failure(
        &mut self,
        f: impl FnOnce(&mut Reporter) -> io::Result<()>,
    ) -> io::Result<()> {
        self.set_operation_failure();
        self.reporter.lock().unwrap().operation_failure(f)?;
        Ok(())
    }

    fn set_test_failure(&mut self) {
        self.exit_code = EXIT_TEST_FAILURE;
    }

    pub fn test_failure(
        &mut self,
        f: impl FnOnce(&mut Reporter) -> io::Result<()>,
    ) -> io::Result<()> {
        self.set_test_failure();
        f(&mut *self.reporter.lock().unwrap())
    }

    pub fn run(&mut self) -> anyhow::Result<()> {
        self.args.cmd.run(self)
    }

    fn set_unexpected_error(&mut self) {
        self.exit_code = EXIT_ERROR;
    }

    pub fn unexpected_error(
        &mut self,
        f: impl FnOnce(&mut Reporter) -> io::Result<()>,
    ) -> io::Result<()> {
        self.set_unexpected_error();
        f(&mut *self.reporter.lock().unwrap())
    }

    pub fn is_operation_failure(&self) -> bool {
        self.exit_code == EXIT_OPERATION_FAILURE
    }

    pub fn exit(self) -> ExitCode {
        let mut reporter = self.reporter.lock().unwrap();
        reporter.reset().unwrap();
        write!(reporter, "").unwrap();
        ExitCode::from(self.exit_code)
    }
}

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

#[derive(clap::Args, Debug, Clone)]
pub struct Global {
    /// The project root directory
    #[arg(long, short, env = "TYPST_ROOT", global = true)]
    pub root: Option<PathBuf>,

    /// The amount of threads to use.
    #[arg(long, short, global = true)]
    pub jobs: Option<usize>,

    #[command(flatten, next_help_heading = "Font Options")]
    pub fonts: FontArgs,

    #[command(flatten, next_help_heading = "Package Options")]
    pub package: PackageArgs,

    #[command(flatten, next_help_heading = "Output Options")]
    pub output: OutputArgs,
}

#[derive(clap::Args, Debug, Clone)]
pub struct OperationArgs {
    // reason: as above, clap does not ignore the extra formatting
    #[allow(rustdoc::bare_urls)]
    /// A test set expression for the given operation
    ///
    /// See https://github.com/tingerrr/typst-test for an introduction on the
    /// test set language.
    #[arg(long, short, conflicts_with = "tests")]
    pub expression: Option<TestSetExpr>,

    /// Allow operating on more than one test if multiple tests match
    ///
    /// This is not requried for comparing or compiling, but for editing,
    /// updating or removing tests.
    #[arg(long, short)]
    pub all: bool,

    /// The tests to use
    ///
    /// This matches any tests which exactly match the given identifiers.
    ///
    /// Consider using `-e '...'` for more complicated test selections.
    #[arg(required = false)]
    pub tests: Vec<Identifier>,
}

impl OperationArgs {
    pub fn test_set(&self) -> anyhow::Result<DynTestSet> {
        Ok(match self.expression.clone() {
            Some(expr) => expr.build(&*test_set::BUILTIN_TESTSETS)?,
            None => {
                if self.tests.is_empty() {
                    test_set::builtin::default()
                } else {
                    self.tests
                        .iter()
                        .map(|id| test_set::builtin::name_string(id.to_inner(), true))
                        .fold(test_set::builtin::none(), |acc, it| {
                            test_set::expr::union(acc, it)
                        })
                }
            }
        })
    }
}

#[derive(clap::Args, Debug, Clone)]
pub struct FontArgs {
    /// Do not read system fonts
    #[arg(long, global = true)]
    pub ignore_system_fonts: bool,

    /// Add a directory to read fonts from (can be repeated)
    #[arg(
        long = "font-path",
        env = "TYPST_FONT_PATHS",
        value_name = "DIR",
        value_delimiter = ENV_PATH_SEP,
        global = true,
    )]
    pub font_paths: Vec<PathBuf>,
}

impl FontArgs {
    pub fn searcher(&self) -> FontSearcher {
        let mut searcher = FontSearcher::new();
        searcher.search(
            self.font_paths.iter().map(PathBuf::as_path),
            !self.ignore_system_fonts,
        );

        searcher
    }
}

#[derive(clap::Args, Debug, Clone)]
pub struct PackageArgs {
    /// Custom path to local packages, defaults to system-dependent location
    #[clap(long, env = "TYPST_PACKAGE_PATH", value_name = "DIR")]
    pub package_path: Option<PathBuf>,

    /// Custom path to package cache, defaults to system-dependent location
    #[clap(long, env = "TYPST_PACKAGE_CACHE_PATH", value_name = "DIR")]
    pub package_cache_path: Option<PathBuf>,

    /// Path to a custom CA certificate to use when making network requests
    #[clap(long, visible_alias = "cert", env = "TYPST_CERT")]
    pub certificate: Option<PathBuf>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct OutputArgs {
    /// The output format to use
    ///
    /// Using anything but pretty implies --color=never
    #[arg(long, visible_alias = "fmt", default_value = "pretty", global = true)]
    pub format: OutputFormat,

    /// When to use colorful output
    ///
    /// If set to auto, color will only be enabled if a capable terminal is
    /// detected.
    #[clap(
        long,
        value_name = "WHEN",
        require_equals = true,
        num_args = 0..=1,
        default_value = "auto",
        default_missing_value = "always",
        global = true,
    )]
    pub color: ColorChoice,

    /// Produce more logging output [-v ... -vvvvv]
    ///
    /// Logs are written to stderr, the increasing number of verbose flags
    /// corresponds to the log levels ERROR, WARN, INFO, DEBUG, TRACE.
    #[arg(long, short, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,
}

// TODO: add json
#[derive(clap::ValueEnum, Debug, Clone, Copy)]
pub enum OutputFormat {
    /// Pretty human-readible color output
    Pretty,

    /// Plain output for script processing
    Plain,
}

impl OutputFormat {
    pub fn is_pretty(&self) -> bool {
        matches!(self, Self::Pretty)
    }
}

/// Run and manage tests for typst projects
#[derive(clap::Parser, Debug, Clone)]
#[clap(after_long_help = AFTER_LONG_ABOUT)]
pub struct Args {
    #[command(flatten)]
    pub global: Global,

    /// The command to run
    #[command(subcommand)]
    pub cmd: Command,
}

#[derive(clap::Subcommand, Debug, Clone)]
pub enum Command {
    /// Initialize the current project with a test directory
    #[command()]
    Init(init::Args),

    /// Remove the test directory from the current project
    #[command()]
    Uninit,

    /// Show information about the current project
    #[command(visible_alias = "st")]
    Status,

    /// List the tests in the current project
    #[command(visible_alias = "ls")]
    List(list::Args),

    /// Compile tests
    #[command(visible_alias = "c")]
    Compile(compile::Args),

    /// Compile and compare tests
    #[command(name = "run", visible_alias = "r")]
    Compare(compare::Args),

    /// Compile and update tests
    #[command(visible_alias = "u")]
    Update(update::Args),

    /// Add a new test
    ///
    /// The default test simply contains `Hello World`, if a
    /// test template file is given, it is used instead.
    #[command(visible_alias = "a")]
    Add(add::Args),

    // reason: escaping this is not ignored by clap
    #[allow(rustdoc::broken_intra_doc_links)]
    /// Edit existing tests [disabled]
    #[command()]
    Edit(edit::Args),

    /// Remove tests
    #[command(visible_alias = "rm")]
    Remove(remove::Args),

    /// Utility commands
    #[command()]
    Util(util::Args),
}

impl Command {
    pub fn run(&self, ctx: &mut Context) -> anyhow::Result<()> {
        match self {
            Command::Init(args) => init::run(ctx, args),
            Command::Uninit => uninit::run(ctx),
            Command::Add(args) => add::run(ctx, args),
            Command::Edit(args) => edit::run(ctx, args),
            Command::Remove(args) => remove::run(ctx, args),
            Command::Status => status::run(ctx),
            Command::List(args) => list::run(ctx, args),
            Command::Update(args) => update::run(ctx, args),
            Command::Compare(args) => compare::run(ctx, args),
            Command::Compile(args) => compile::run(ctx, args),
            Command::Util(args) => args.cmd.run(ctx),
        }
    }
}
