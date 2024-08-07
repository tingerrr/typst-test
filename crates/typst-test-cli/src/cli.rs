use std::fmt::Display;
use std::io;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{mpsc, Arc};

use chrono::{DateTime, Utc};
use clap::ColorChoice;
use termcolor::Color;
use thiserror::Error;
use typst_test_lib::store::vcs::{Git, Vcs};
use typst_test_lib::test::id::Identifier;
use typst_test_lib::test_set::builtin::{IdentifierPattern, IdentifierTarget, IdentifierTestSet};
use typst_test_lib::test_set::{DynTestSet, Eval};
use typst_test_lib::{compare, render, test_set};
use typst_test_stdx::fmt::Term;

use crate::error::{Failure, OperationFailure};
use crate::fonts::FontSearcher;
use crate::package::PackageStorage;
use crate::project::Project;
use crate::report::{Format, Reporter};
use crate::test::runner::{Event, Progress, Runner, RunnerConfig};
use crate::ui::Ui;
use crate::world::SystemWorld;
use crate::{project, ui};

pub mod add;
pub mod config;
pub mod debug;
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

/// An unexpected error occurred.
pub const EXIT_ERROR: u8 = 3;

#[derive(Debug, Error)]
#[error("root '{}' not found", .0.display())]
pub struct RootNotFound(pub PathBuf);

impl Failure for RootNotFound {
    fn report(&self, ui: &Ui) -> io::Result<()> {
        ui.error_with(|w| writeln!(w, "Root '{}' not found", self.0.display()))
    }
}

#[derive(Debug, Error)]
#[error("must be in project root")]
pub struct NoProject;

impl Failure for NoProject {
    fn report(&self, ui: &Ui) -> io::Result<()> {
        ui.error_hinted(
            "Must be in project root",
            "You can pass the project root using '--root <path>'",
        )
    }
}

#[derive(Debug, Error)]
pub struct ProjectNotInitialized(pub Option<String>);

impl Display for ProjectNotInitialized {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0.as_deref() {
            Some(name) => write!(f, "Package '{name}' was not initialized"),
            None => write!(f, "Project was not initialized"),
        }
    }
}

impl Failure for ProjectNotInitialized {
    fn report(&self, ui: &Ui) -> io::Result<()> {
        ui.error_with(|w| {
            if let Some(name) = self.0.as_deref() {
                write!(w, "Package ")?;
                ui::write_colored(w, Color::Cyan, |w| write!(w, "{name}"))?
            } else {
                write!(w, "Project ")?;
            }
            writeln!(w, " was not initialized")
        })
    }
}

#[derive(Debug, Error)]
#[error(transparent)]
pub struct TestSetFailure(anyhow::Error);

impl Failure for TestSetFailure {
    fn report(&self, ui: &Ui) -> io::Result<()> {
        ui.error_with(|w| writeln!(w, "Couldn't parse test set expression:\n{:?}", self.0))
    }
}

#[derive(Debug, Error)]
#[error("matched no tests")]
pub struct NoTests;

impl Failure for NoTests {
    fn report(&self, ui: &Ui) -> io::Result<()> {
        ui.error("Matched no tests")
    }
}

#[derive(Debug, Error)]
#[error("matched more than one test without a confirmation for operation {0}")]
pub struct Aborted(String);

impl Failure for Aborted {
    fn report(&self, ui: &Ui) -> io::Result<()> {
        ui.error_hinted_with(
            |w| writeln!(w, "Matched more than one test without confirmation"),
            |w| {
                writeln!(
                    w,
                    "Pass `--all` to {} more than one test at a time without a prompt",
                    self.0
                )
            },
        )
    }
}

#[derive(Debug, Error)]
#[error("unsupported export mode used")]
pub struct UnsupportedExport;

impl Failure for UnsupportedExport {
    fn report(&self, ui: &Ui) -> io::Result<()> {
        ui.error("PDF and SVG export are not yet supported")
    }
}

pub struct Context<'a> {
    pub args: &'a Args,
    pub reporter: &'a Reporter,
}

impl<'a> Context<'a> {
    pub fn new(args: &'a Args, reporter: &'a Reporter) -> Self {
        tracing::debug!(args = ?args, "creating context");

        Self { args, reporter }
    }

    pub fn try_discover_vcs(&mut self) -> anyhow::Result<Option<Box<dyn Vcs + Sync>>> {
        tracing::debug!("looking for vcs root");

        let start = if let Some(root) = &self.args.global.root {
            root.canonicalize()?
        } else {
            std::env::current_dir()?
        };

        for ancestor in start.ancestors() {
            if ancestor.join(".git").try_exists()? {
                tracing::info!(root = ?ancestor, "found git root");
                return Ok(Some(Box::new(Git::new(ancestor.to_path_buf())?)));
            }
        }

        Ok(None)
    }

    pub fn ensure_project(&mut self) -> anyhow::Result<Project> {
        tracing::debug!("looking for project");

        let root = match &self.args.global.root {
            Some(root) => root.to_path_buf(),
            None => {
                let pwd = std::env::current_dir()?;
                match typst_project::try_find_project_root(&pwd)? {
                    Some(root) => {
                        if !root.try_exists()? {
                            anyhow::bail!(OperationFailure::from(RootNotFound(root.to_path_buf())));
                        }
                        root.to_path_buf()
                    }
                    None => {
                        anyhow::bail!(OperationFailure::from(NoProject));
                    }
                }
            }
        };

        tracing::info!(?root, "found project root");
        let manifest = match project::try_open_manifest(&root) {
            Ok(manifest) => manifest,
            Err(err) => {
                if let Some(err) = err.root_cause().downcast_ref::<toml::de::Error>() {
                    tracing::error!(?err, "Couldn't parse manifest");

                    self.reporter.ui().warning_with(|w| {
                        writeln!(w, "Error while parsing manifest, skipping")?;
                        writeln!(w, "{}", err.message())
                    })?;
                    None
                } else {
                    anyhow::bail!(err)
                }
            }
        };

        let vcs = self.try_discover_vcs()?;

        Project::new(root, vcs, manifest)
    }

    pub fn ensure_init(&mut self) -> anyhow::Result<Project> {
        let project = self.ensure_project()?;

        tracing::debug!("ensuring project is initalized");
        if !project.is_init()? {
            anyhow::bail!(OperationFailure::from(ProjectNotInitialized(
                project.manifest().map(|m| m.package.name.to_owned())
            )));
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
                anyhow::bail!(OperationFailure::from(TestSetFailure(err)));
            }
        };

        tracing::debug!("collecting tests");
        project.collect_tests(test_set)?;

        let len = project.matched().len();

        match (len, op_requires_confirm_for_many.into()) {
            (0, _) => {
                anyhow::bail!(OperationFailure::from(NoTests));
            }
            (1, _) => {}
            (_, None) => {}
            (_, Some(op)) => {
                // Explicitly passing more than one test implies `--all`
                let confirmed = op_args.all
                    || !op_args.tests.is_empty()
                    || self.reporter.ui().prompt_yes_no(
                        format!("{op} {len} {}", Term::simple("test").with(len)),
                        false,
                    )?;

                if !confirmed {
                    anyhow::bail!(OperationFailure::from(Aborted(op.to_owned())));
                }
            }
        }

        tracing::debug!(
            matched = ?project.matched().len(),
            filtered = ?project.filtered().len(),
            "collected tests",
        );
        Ok(project)
    }

    pub fn build_world(
        &mut self,
        project: &Project,
        compile_args: &CompileArgs,
    ) -> anyhow::Result<SystemWorld> {
        let world = SystemWorld::new(
            project.root().to_path_buf(),
            self.args.global.fonts.searcher(),
            PackageStorage::from_args(&self.args.global.package),
            compile_args.now,
        )?;

        Ok(world)
    }

    pub fn build_runner<'p, C: Configure>(
        &mut self,
        project: &'p Project,
        world: &'p SystemWorld,
        args: &C,
    ) -> anyhow::Result<(Runner<'p>, mpsc::Receiver<Event>)> {
        let mut config = RunnerConfig::default();
        args.configure(self, project, &mut config)?;

        let (progress, rx) = Progress::new(project);
        Ok((config.build(progress, project, world), rx))
    }

    pub fn run(&mut self) -> anyhow::Result<()> {
        self.args.cmd.run(self)
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
    "  ", ansi!("3"; b), "  An unexpected error occurred",
);

#[derive(clap::Args, Debug, Clone)]
pub struct GlobalArgs {
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
    pub expression: Option<String>,

    /// Allow operating on more than one test if multiple tests match
    ///
    /// This is not required for comparing or compiling, but for editing,
    /// updating or removing tests. This is implied if a user explicitly passes
    /// multiple tests as positional arguments.
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
        let _span = tracing::debug_span!("building test set");

        let test_set = match self.expression.clone() {
            Some(expr) => {
                tracing::debug!("compiling test set");
                let expr = test_set::parse(&expr)?;
                expr.eval(&test_set::eval::Context::builtin())?
                    .to_test_set()?
            }
            None => {
                if self.tests.is_empty() {
                    tracing::debug!("compiling default test set");
                    test_set::builtin::default()
                } else {
                    tracing::debug!(
                        tests = ?self.tests,
                        "building strict test set from explicit tests",
                    );
                    self.tests
                        .iter()
                        .map(|id| IdentifierTestSet {
                            pattern: IdentifierPattern::Exact(id.to_inner()),
                            target: IdentifierTarget::Name,
                        })
                        .fold(test_set::builtin::none(), |acc, it| {
                            Arc::new(test_set::builtin::InfixTestSet::union(acc, it))
                        })
                }
            }
        };

        tracing::trace!(?test_set, "built test set");
        Ok(test_set)
    }
}

pub trait Configure {
    fn configure(
        &self,
        ctx: &mut Context,
        project: &Project,
        config: &mut RunnerConfig,
    ) -> anyhow::Result<()>;
}

fn parse_source_date_epoch(raw: &str) -> Result<DateTime<Utc>, String> {
    let timestamp: i64 = raw
        .parse()
        .map_err(|err| format!("timestamp must be decimal integer ({err})"))?;
    DateTime::from_timestamp(timestamp, 0).ok_or_else(|| "timestamp out of range".to_string())
}

#[derive(clap::Args, Debug, Clone)]
pub struct CompileArgs {
    /// The timestamp used for compilation.
    ///
    /// For more information, see
    /// <https://reproducible-builds.org/specs/source-date-epoch/>.
    #[clap(
        long = "creation-timestamp",
        env = "SOURCE_DATE_EPOCH",
        value_name = "UNIX_TIMESTAMP",
        value_parser = parse_source_date_epoch,
        global = true,
    )]
    pub now: Option<DateTime<Utc>>,
}

impl Configure for CompileArgs {
    fn configure(
        &self,
        _ctx: &mut Context,
        _project: &Project,
        config: &mut RunnerConfig,
    ) -> anyhow::Result<()> {
        tracing::trace!(compile = ?true, "configuring runner");
        config.with_compile(true);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, clap::ValueEnum)]
pub enum Direction {
    /// The document is read left-to-right.
    Ltr,

    /// The document is read right-to-left.
    Rtl,
}

#[derive(clap::Args, Debug, Clone)]
pub struct ExportArgs {
    /// Whether to save temporary output, such as ephemeral references
    #[arg(long, global = true)]
    pub no_save_temporary: bool,

    /// The document direction
    ///
    /// This is used to correctly align images with different dimensions when
    /// generating diff images.
    #[arg(long, visible_alias = "dir", global = true)]
    pub direction: Option<Direction>,

    /// Whether to output raster images
    #[arg(long, global = true)]
    pub raster: bool,

    /// Whether to putput svg images [currently unsupported]
    // reason: escaping this is not ignored by clap
    #[allow(rustdoc::broken_intra_doc_links)]
    #[arg(long, global = true)]
    pub svg: bool,

    /// Whether to output pdf documents [currently unsupported]
    // reason: escaping this is not ignored by clap
    #[allow(rustdoc::broken_intra_doc_links)]
    #[arg(long, global = true)]
    pub pdf: bool,

    /// The pixel per inch to use for raster export
    #[arg(long, visible_alias = "ppi", default_value_t = 144.0, global = true)]
    pub pixel_per_inch: f32,
}

impl Configure for ExportArgs {
    fn configure(
        &self,
        _ctx: &mut Context,
        _project: &Project,
        config: &mut RunnerConfig,
    ) -> anyhow::Result<()> {
        let render_strategy = render::Strategy {
            pixel_per_pt: render::ppi_to_ppp(self.pixel_per_inch),
        };

        if self.pdf || self.svg {
            anyhow::bail!(OperationFailure::from(UnsupportedExport));
        }

        config
            .with_render_strategy(Some(render_strategy))
            .with_no_save_temporary(self.no_save_temporary);

        if let Some(dir) = self.direction {
            config.with_diff_render_origin(Some(match dir {
                Direction::Ltr => render::Origin::TopLeft,
                Direction::Rtl => render::Origin::TopRight,
            }));
        }

        tracing::trace!(
            export_render_strategy = ?config.render_strategy(),
            no_save_temporary = ?config.no_save_temporary(),
            "configuring runner",
        );

        Ok(())
    }
}

#[derive(clap::Args, Debug, Clone)]
pub struct CompareArgs {
    /// The maximum delta in each channel of a pixel
    ///
    /// If a single channel (red/green/blue/alpha component) of a pixel differs
    /// by this much between reference and output the pixel is counted as a
    /// deviation.
    #[arg(long, default_value_t = 0, global = true)]
    pub max_delta: u8,

    /// The maximum deviation per reference
    ///
    /// If a reference and output image have more than the given deviations it's
    /// counted as a failure.
    #[arg(long, default_value_t = 0, global = true)]
    pub max_deviation: usize,
}

impl Configure for CompareArgs {
    fn configure(
        &self,
        _ctx: &mut Context,
        _project: &Project,
        config: &mut RunnerConfig,
    ) -> anyhow::Result<()> {
        let compare_strategy = compare::Strategy::Visual(compare::visual::Strategy::Simple {
            max_delta: self.max_delta,
            max_deviation: self.max_deviation,
        });

        config.with_compare_strategy(Some(compare_strategy));
        tracing::trace!(
            compare_strategy = ?config.compare_strategy(),
            "configuring runner"
        );

        Ok(())
    }
}

#[derive(clap::Args, Debug, Clone)]
pub struct RunArgs {
    /// Show a summary of the test run instead of the individual test results
    #[arg(long, global = true)]
    pub summary: bool,

    /// Do not run hooks
    #[arg(long, global = true)]
    pub no_hooks: bool,

    /// Whether to abort after the first failure
    ///
    /// Keep in mind that because tests are run in parallel, this may not stop
    /// immediately. But it will not schedule any new tests to run after one
    /// failure has been detected.
    #[arg(long, global = true)]
    pub no_fail_fast: bool,
}

impl Configure for RunArgs {
    fn configure(
        &self,
        _ctx: &mut Context,
        project: &Project,
        config: &mut RunnerConfig,
    ) -> anyhow::Result<()> {
        let root = project.root();

        config.with_no_fail_fast(self.no_fail_fast);
        if !self.no_hooks {
            config.with_prepare_hook(
                project
                    .config()
                    .prepare
                    .as_deref()
                    .map(|rel| root.join(rel)),
            );
            config.with_prepare_each_hook(
                project
                    .config()
                    .prepare_each
                    .as_deref()
                    .map(|rel| root.join(rel)),
            );
            config.with_cleanup_hook(
                project
                    .config()
                    .cleanup
                    .as_deref()
                    .map(|rel| root.join(rel)),
            );
            config.with_cleanup_each_hook(
                project
                    .config()
                    .cleanup_each
                    .as_deref()
                    .map(|rel| root.join(rel)),
            );
        }

        tracing::trace!(
            hooks.prepare = ?config.prepare_hook(),
            hooks.prepare_each = ?config.prepare_each_hook(),
            hooks.cleanup = ?config.cleanup_hook(),
            hooks.cleanup_each = ?config.cleanup_each_hook(),
            no_fail_fast = ?config.no_fail_fast(),
            "configuring runner",
        );

        Ok(())
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
        let _span = tracing::debug_span!("searching for fonts");

        let mut searcher = FontSearcher::new();
        searcher.search(
            self.font_paths.iter().map(PathBuf::as_path),
            !self.ignore_system_fonts,
        );

        tracing::debug!(
            fonts = ?searcher.fonts.len(),
            included_system_fonts = ?!self.ignore_system_fonts,
            "collected fonts",
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
    #[arg(long, visible_alias = "fmt", default_value = "human", global = true)]
    pub format: Format,

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
    /// Pretty human-readable color output
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
    pub global: GlobalArgs,

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
    Uninit(uninit::Args),

    /// Show information about the current project
    #[command(visible_alias = "st")]
    Status,

    /// List the tests in the current project
    #[command(visible_alias = "ls")]
    List(list::Args),

    /// Compile and compare tests
    #[command(visible_alias = "r")]
    Run(run::Args),

    /// Compile and update tests
    #[command(visible_alias = "u")]
    Update(update::Args),

    /// Add a new test
    ///
    /// The default test simply contains `Hello World`, if a
    /// test template file is given, it is used instead.
    #[command(visible_alias = "a")]
    Add(add::Args),

    /// Edit existing tests [disabled]
    // reason: escaping this is not ignored by clap
    #[allow(rustdoc::broken_intra_doc_links)]
    #[command()]
    Edit(edit::Args),

    /// Remove tests
    #[command(visible_alias = "rm")]
    Remove(remove::Args),

    /// Display and edit config
    #[command()]
    Config(config::Args),

    /// Utility commands
    #[command()]
    Util(util::Args),

    /// Debugging commands
    #[command()]
    Debug(debug::Args),
}

impl Command {
    pub fn run(&self, ctx: &mut Context) -> anyhow::Result<()> {
        match self {
            Command::Init(args) => init::run(ctx, args),
            Command::Uninit(args) => uninit::run(ctx, args),
            Command::Add(args) => add::run(ctx, args),
            Command::Edit(args) => edit::run(ctx, args),
            Command::Remove(args) => remove::run(ctx, args),
            Command::Status => status::run(ctx),
            Command::List(args) => list::run(ctx, args),
            Command::Update(args) => update::run(ctx, args),
            Command::Run(args) => run::run(ctx, args),
            Command::Config(args) => args.cmd.run(ctx),
            Command::Util(args) => args.cmd.run(ctx),
            Command::Debug(args) => args.cmd.run(ctx),
        }
    }
}
