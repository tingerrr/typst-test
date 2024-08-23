use std::collections::BTreeMap;
use std::fmt::Debug;
use std::io;
use std::io::Write;

use codespan_reporting::diagnostic::{Diagnostic, Label};
use codespan_reporting::term;
use ecow::eco_format;
use serde::Serialize;
use termcolor::{Color, WriteColor};
use typst::diag::{Severity, SourceDiagnostic};
use typst::syntax::{FileId, Source, Span};
use typst::WorldExt;
use typst_test_lib::compare;
use typst_test_lib::store::test::Test;
use typst_test_lib::test::id::Identifier;
use typst_test_lib::test::ReferenceKind;
use typst_test_stdx::fmt::Term;

use crate::project::Project;
use crate::test::runner::{Event, EventPayload, Summary};
use crate::test::{CompareFailure, Stage, TestFailure};
use crate::ui;
use crate::ui::{Indented, Live, Ui};
use crate::world::SystemWorld;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Target {
    /// The target for primary output like listing tests.
    Primary,

    /// The target for errors and warnings.
    Secondary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, clap::ValueEnum)]
pub enum Verbosity {
    /// Do not do any reporting other than errors and warnings.
    Quiet = 0,

    /// Only report the most important messages.
    Less = 1,

    /// Report all messages in full detail.
    All = 255,
}

/// The format to use for primary outputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, clap::ValueEnum)]
pub enum Format {
    /// Write the output messages in a human friendly format.
    Human,

    /// Serialize output messages into striaght into JSON.
    Json,
}

/// A type which can be serialized as JSON directly or presented in a human readable fashion.
pub trait Report: Serialize {
    /// Presents this report in a human readable fashion.
    fn report<W: WriteColor>(&self, writer: W, verbosity: Verbosity) -> anyhow::Result<()>;
}

#[derive(Debug)]
pub struct Reporter {
    ui: Ui,
    verbosity: Verbosity,
    format: Format,
}

impl Reporter {
    /// Creates a new reporter.
    pub fn new(ui: Ui, verbosity: Verbosity, format: Format) -> Self {
        Self {
            ui,
            verbosity,
            format,
        }
    }

    /// Returns a reference to the [`Ui`].
    pub fn ui(&self) -> &Ui {
        &self.ui
    }

    /// Executes the closure with the given target.
    pub fn with_target<F, R>(&self, target: Target, f: F) -> R
    where
        F: FnOnce(&mut dyn WriteColor) -> R,
    {
        match target {
            Target::Primary => f(&mut self.ui.stdout()),
            Target::Secondary => f(&mut self.ui.stderr()),
        }
    }

    /// Executes the closure with the given target.
    pub fn report<R: Report>(&self, report: &R) -> anyhow::Result<()> {
        self.with_target(Target::Primary, |w| {
            if self.format == Format::Json {
                serde_json::to_writer_pretty(w, report)?;
            } else if self.verbosity > Verbosity::Quiet {
                report.report(w, self.verbosity)?;
            }

            Ok(())
        })
    }

    /// Writes the header of a starting run.
    ///
    /// This unconditionally reports to the secondary target.
    pub fn run_start(&self, operation: &str) -> io::Result<()> {
        self.with_target(Target::Secondary, |w| {
            ui::write_bold(w, |w| writeln!(w, "{operation} tests"))
        })?;

        Ok(())
    }
}

/// Common report PODs for stable JSON representation of internal entites.
pub mod reports {
    use ui::Heading;

    use super::*;

    #[derive(Debug, Serialize)]
    pub struct TestJson<'t> {
        pub id: &'t str,
        pub kind: &'static str,
    }

    impl<'t> TestJson<'t> {
        pub fn new(test: &'t Test) -> Self {
            Self {
                id: test.id().as_str(),
                kind: match test.ref_kind() {
                    Some(ReferenceKind::Ephemeral) => "ephemeral",
                    Some(ReferenceKind::Persistent) => "persistent",
                    None => "compile-only",
                },
            }
        }
    }

    #[derive(Debug, Serialize)]
    pub struct PackageJson<'p> {
        pub name: &'p str,
        pub version: String,
    }

    #[derive(Debug, Serialize)]
    pub struct ProjectJson<'p> {
        pub package: Option<PackageJson<'p>>,
        pub vcs: Option<String>,
        pub tests: Vec<TestJson<'p>>,
        pub template_path: Option<String>,
    }

    impl<'p> ProjectJson<'p> {
        pub fn new(project: &'p Project) -> Self {
            Self {
                package: project.manifest().map(|m| PackageJson {
                    name: &m.package.name,
                    version: m.package.version.to_string(),
                }),
                vcs: project.vcs().map(|vcs| vcs.to_string()),
                tests: project.matched().values().map(TestJson::new).collect(),
                template_path: project.template_path().map(|p| p.display().to_string()),
            }
        }
    }

    #[derive(Serialize)]
    pub struct FailedJson {
        pub compilation: usize,
        pub comparison: usize,
        pub otherwise: usize,
    }

    #[derive(Serialize)]
    pub struct DurationJson {
        pub seconds: u64,
        pub nanoseconds: u32,
    }

    /// A stable and slightly less complete serialization of [`Summary`].
    #[derive(Serialize)]
    pub struct SummaryReport {
        #[serde(skip)]
        pub operation: &'static str,
        pub total: usize,
        pub filtered: usize,
        pub passed: usize,
        pub failed: FailedJson,
        pub time: DurationJson,
    }

    impl SummaryReport {
        pub fn new(operation: &'static str, summary: &Summary) -> Self {
            Self {
                operation,
                total: summary.total,
                filtered: summary.filtered,
                passed: summary.passed,
                failed: FailedJson {
                    compilation: summary.failed_compilation,
                    comparison: summary.failed_comparison,
                    otherwise: summary.failed_otherwise,
                },
                time: DurationJson {
                    seconds: summary.time.as_secs(),
                    nanoseconds: summary.time.subsec_nanos(),
                },
            }
        }

        pub fn run(&self) -> usize {
            self.total - self.filtered
        }

        pub fn is_ok(&self) -> bool {
            self.passed == self.run()
        }

        pub fn is_total_fail(&self) -> bool {
            self.passed == 0
        }
    }

    impl Report for SummaryReport {
        fn report<W: WriteColor>(&self, writer: W, _verbosity: Verbosity) -> anyhow::Result<()> {
            let w = &mut Heading::new(writer, "Summary");

            let color = if self.is_ok() {
                Color::Green
            } else if self.is_total_fail() {
                Color::Red
            } else {
                Color::Yellow
            };

            ui::write_bold_colored(w, color, |w| write!(w, "{}", self.passed))?;
            write!(w, " / ")?;
            ui::write_bold(w, |w| write!(w, "{}", self.run()))?;
            write!(w, " {}.", self.operation)?;

            if self.failed.compilation != 0 {
                write!(w, " ")?;
                ui::write_bold_colored(w, Color::Red, |w| {
                    write!(w, "{}", self.failed.compilation)
                })?;
                write!(w, " failed compilations.")?;
            }

            if self.failed.comparison != 0 {
                write!(w, " ")?;
                ui::write_bold_colored(w, Color::Red, |w| write!(w, "{}", self.failed.comparison))?;
                write!(w, " failed comparisons.")?;
            }

            if self.failed.otherwise != 0 {
                write!(w, " ")?;
                ui::write_bold_colored(w, Color::Red, |w| write!(w, "{}", self.failed.otherwise))?;
                write!(w, " failed otherwise.")?;
            }

            if self.filtered != 0 {
                write!(w, " ")?;
                ui::write_bold_colored(w, Color::Yellow, |w| write!(w, "{}", self.filtered))?;
                write!(w, " filtered out.")?;
            }

            let secs = self.time.seconds;
            match (secs / 60, secs) {
                (0, 0) => writeln!(w)?,
                (0, s) => writeln!(w, " took {s} {}", Term::simple("second").with(s as usize))?,
                (m, s) => writeln!(
                    w,
                    " took {m} {} {s} {}",
                    Term::simple("minute").with(m as usize),
                    Term::simple("second").with(s as usize)
                )?,
            }

            Ok(())
        }
    }
}

pub struct LiveReporterState<W> {
    live: Live<W>,
    progress_annot: &'static str,
    tests: BTreeMap<Identifier, (Test, &'static str)>,
    count: usize,
    total: usize,
}

impl<W> LiveReporterState<W> {
    pub fn new(writer: W, progress_annot: &'static str, total: usize) -> Self {
        Self {
            live: Live::new(writer),
            progress_annot,
            tests: BTreeMap::new(),
            count: 0,
            total,
        }
    }
}

fn test_failure<W: WriteColor + ?Sized>(
    w: &mut W,
    test: &Test,
    error: TestFailure,
    world: &SystemWorld,
    max_indent: impl Into<Option<usize>>,
) -> io::Result<()> {
    ui::write_annotated(w, "failed", Color::Red, max_indent, |w| {
        writeln!(w, "{}", test.id())?;

        // if self.verbosity == Verbosity::Quiet {
        //     return Ok(());
        // }

        // TODO: less elaborate reporting for `Less`

        let w = &mut Indented::new(w, 2);

        match error {
            TestFailure::Compilation(e) => {
                writeln!(
                    w,
                    "Compilation of {} failed",
                    if e.is_ref { "references" } else { "test" },
                )?;

                // TODO: pass warnings + report warnings on success too
                print_diagnostics(w, world, e.error.0.as_slice(), &[]).unwrap();
            }
            TestFailure::Comparison(CompareFailure::Visual {
                error:
                    compare::Error {
                        output,
                        reference,
                        pages,
                    },
                diff_dir,
            }) => {
                if output != reference {
                    writeln!(
                        w,
                        "Expected {reference} {}, got {output} {}",
                        Term::simple("page").with(reference),
                        Term::simple("page").with(output),
                    )?;
                }

                for (p, e) in pages {
                    let p = p + 1;
                    match e {
                        compare::PageError::Dimensions { output, reference } => {
                            writeln!(w, "Page {p} had different dimensions")?;
                            w.write_with(2, |w| {
                                writeln!(w, "Output: {}", output)?;
                                writeln!(w, "Reference: {}", reference)
                            })?;
                        }
                        compare::PageError::SimpleDeviations { deviations } => {
                            writeln!(
                                w,
                                "Page {p} had {deviations} {}",
                                Term::simple("deviation").with(deviations),
                            )?;
                        }
                    }
                }

                if let Some(diff_dir) = diff_dir {
                    ui::write_hint_with(w, None, |w| {
                        writeln!(w, "Diff images have been saved at '{}'", diff_dir.display())
                    })?;
                }
            }
        }

        Ok(())
    })?;

    Ok(())
}

impl<W: WriteColor> LiveReporterState<W> {
    pub fn event(&mut self, world: &SystemWorld, event: Event) -> io::Result<()> {
        // TODO: track times by comparing stage instants

        let pad = [
            "prepare",
            "hook",
            "load",
            "compile",
            "save",
            "render",
            "compare",
            "update",
            "cleanup",
            "ok",
            self.progress_annot,
        ]
        .iter()
        .map(|s| s.len())
        .max();

        self.live.live(|w| {
            let id = event.test.id();
            match event.payload {
                EventPayload::StartedTest => {
                    self.tests.insert(id.clone(), (event.test, "start"));
                }
                EventPayload::StartedStage(stage) => {
                    self.tests.get_mut(id).unwrap().1 = match stage {
                        Stage::Preparation => "prepare",
                        Stage::Hooks => "hook",
                        Stage::Loading => "load",
                        Stage::Compilation => "compile",
                        Stage::Saving => "save",
                        Stage::Rendering => "render",
                        Stage::Comparison => "compare",
                        Stage::Update => "update",
                        Stage::Cleanup => "cleanup",
                    };
                }
                EventPayload::FinishedStage(_) => {}
                EventPayload::FailedStage(_) => {}
                EventPayload::FinishedTest => {
                    self.tests.remove(id);
                    ui::write_annotated(w, "ok", Color::Green, pad, |w| {
                        writeln!(w, "{}", event.test.id())
                    })?;
                    self.count += 1;
                    w.reset_lines();
                }
                EventPayload::FailedTest(failure) => {
                    self.tests.remove(id);
                    test_failure(w, &event.test, failure, world, pad)?;
                    self.count += 1;
                    w.reset_lines();
                }
            }

            for (test, msg) in self.tests.values() {
                ui::write_annotated(w, msg, Color::Yellow, pad, |w| writeln!(w, "{}", test.id()))?;
            }

            ui::write_annotated(w, self.progress_annot, Color::Cyan, pad, |w| {
                writeln!(
                    w,
                    "{} / {} ({} tests running)",
                    self.count,
                    self.total,
                    self.tests.len(),
                )
            })?;

            Ok(())
        })?;

        Ok(())
    }
}

type CodespanResult<T> = Result<T, CodespanError>;
type CodespanError = codespan_reporting::files::Error;

fn print_diagnostics<W: WriteColor>(
    writer: &mut W,
    world: &SystemWorld,
    errors: &[SourceDiagnostic],
    warnings: &[SourceDiagnostic],
) -> Result<(), codespan_reporting::files::Error> {
    let config = term::Config {
        display_style: term::DisplayStyle::Rich,
        tab_width: 2,
        ..Default::default()
    };

    for diagnostic in warnings.iter().chain(errors) {
        let diag = match diagnostic.severity {
            Severity::Error => Diagnostic::error(),
            Severity::Warning => Diagnostic::warning(),
        }
        .with_message(diagnostic.message.clone())
        .with_notes(
            diagnostic
                .hints
                .iter()
                .map(|e| (eco_format!("hint: {e}")).into())
                .collect(),
        )
        .with_labels(label(world, diagnostic.span).into_iter().collect());

        term::emit(writer, &config, world, &diag)?;

        // Stacktrace-like helper diagnostics.
        for point in &diagnostic.trace {
            let message = point.v.to_string();
            let help = Diagnostic::help()
                .with_message(message)
                .with_labels(label(world, point.span).into_iter().collect());

            term::emit(writer, &config, world, &help)?;
        }
    }

    Ok(())
}

fn label(world: &SystemWorld, span: Span) -> Option<Label<FileId>> {
    Some(Label::primary(span.id()?, world.range(span)?))
}

impl<'a> codespan_reporting::files::Files<'a> for SystemWorld {
    type FileId = FileId;
    type Name = String;
    type Source = Source;

    fn name(&'a self, id: FileId) -> CodespanResult<Self::Name> {
        let vpath = id.vpath();
        Ok(if let Some(package) = id.package() {
            format!("{package}{}", vpath.as_rooted_path().display())
        } else {
            // Try to express the path relative to the working directory.
            vpath
                .resolve(self.root())
                // .and_then(|abs| pathdiff::diff_paths(abs, self.workdir()))
                // .as_deref()
                .unwrap_or_else(|| vpath.as_rootless_path().to_path_buf())
                .to_string_lossy()
                .into()
        })
    }

    fn source(&'a self, id: FileId) -> CodespanResult<Self::Source> {
        Ok(self.lookup(id))
    }

    fn line_index(&'a self, id: FileId, given: usize) -> CodespanResult<usize> {
        let source = self.lookup(id);
        source
            .byte_to_line(given)
            .ok_or_else(|| CodespanError::IndexTooLarge {
                given,
                max: source.len_bytes(),
            })
    }

    fn line_range(&'a self, id: FileId, given: usize) -> CodespanResult<std::ops::Range<usize>> {
        let source = self.lookup(id);
        source
            .line_to_range(given)
            .ok_or_else(|| CodespanError::LineTooLarge {
                given,
                max: source.len_lines(),
            })
    }

    fn column_number(&'a self, id: FileId, _: usize, given: usize) -> CodespanResult<usize> {
        let source = self.lookup(id);
        source.byte_to_column(given).ok_or_else(|| {
            let max = source.len_bytes();
            if given <= max {
                CodespanError::InvalidCharBoundary { given }
            } else {
                CodespanError::IndexTooLarge { given, max }
            }
        })
    }
}
