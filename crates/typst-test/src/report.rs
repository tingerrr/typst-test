//! Live reporting of test progress.

use std::io::{self, Write};
use std::time::Duration;

use codespan_reporting::diagnostic::{Diagnostic, Label};
use codespan_reporting::term;
use color_eyre::eyre;
use ecow::eco_format;
use lib::doc::compare::{self, PageError};
use lib::project::Project;
use lib::stdx::fmt::Term;
use lib::test::{SuiteResult, Test, TestResult, TestResultKind};
use termcolor::{Color, WriteColor};
use typst::diag::{Severity, SourceDiagnostic};
use typst::WorldExt;
use typst_syntax::{FileId, Span};

use crate::ui::{self, Ui};
use crate::world::SystemWorld;

/// The padding to use for annotations while test run reporting.
const RUN_ANNOT_PADDING: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum When {
    Never,
    // TODO(tinger): make this configurable, along side the richness of
    // diagnostics
    #[allow(dead_code)]
    Failure,
    Always,
}

/// A reporter for test output and test run status reporting.
pub struct Reporter<'ui, 'p> {
    ui: &'ui Ui,
    project: &'p Project,
    world: &'p SystemWorld,

    live: bool,
    warnings: When,
    errors: bool,
    diagnostic_config: term::Config,
}

impl<'ui, 'p> Reporter<'ui, 'p> {
    pub fn new(ui: &'ui Ui, project: &'p Project, world: &'p SystemWorld, live: bool) -> Self {
        Self {
            ui,
            project,
            world,
            live,
            warnings: When::Always,
            errors: true,
            diagnostic_config: term::Config {
                display_style: term::DisplayStyle::Rich,
                tab_width: 2,
                ..Default::default()
            },
        }
    }
}

impl Reporter<'_, '_> {
    /// Reports the start of a test run.
    pub fn report_start(&self, result: &SuiteResult) -> io::Result<()> {
        let mut w = self.ui.stderr();

        ui::write_annotated(&mut w, "Starting", Color::Green, RUN_ANNOT_PADDING, |w| {
            ui::write_bold(w, |w| write!(w, "{}", result.total()))?;
            write!(w, " tests")?;

            if result.filtered() != 0 {
                write!(w, ", ")?;
                ui::write_bold(w, |w| write!(w, "{}", result.filtered()))?;
                write!(w, " ")?;
                ui::write_colored(w, Color::Yellow, |w| write!(w, "filtered"))?;
            }

            write!(w, " (run ID: ")?;
            ui::write_bold(w, |w| write!(w, "{}", result.id()))?;
            writeln!(w, ")")?;

            Ok(())
        })
    }

    /// Reports the end of a test run.
    pub fn report_end(&self, result: &SuiteResult) -> io::Result<()> {
        let mut w = self.ui.stderr();

        let color = if result.failed() == 0 {
            Color::Green
        } else if result.passed() == 0 {
            Color::Red
        } else {
            Color::Yellow
        };

        writeln!(w, "{:─>RUN_ANNOT_PADDING$}", "")?;

        ui::write_annotated(&mut w, "Summary", color, RUN_ANNOT_PADDING, |w| {
            write!(w, "[")?;
            ui::write_colored(
                w,
                duration_color(
                    result
                        .duration()
                        .checked_div(result.run() as u32)
                        .unwrap_or_default(),
                ),
                |w| write_duration(w, result.duration()),
            )?;
            write!(w, "] ")?;

            ui::write_bold(w, |w| write!(w, "{}", result.run()))?;
            write!(w, "/")?;
            ui::write_bold(w, |w| write!(w, "{}", result.expected()))?;
            write!(w, " tests run: ")?;

            if result.passed() == result.total() {
                ui::write_bold(w, |w| write!(w, "all {}", result.passed()))?;
                write!(w, " ")?;
                ui::write_colored(w, Color::Green, |w| write!(w, "passed"))?;
            } else if result.failed() == result.total() {
                ui::write_bold(w, |w| write!(w, "all {}", result.failed()))?;
                write!(w, " ")?;
                ui::write_colored(w, Color::Red, |w| write!(w, "failed"))?;
            } else {
                ui::write_bold(w, |w| write!(w, "{}", result.passed()))?;
                write!(w, " ")?;
                ui::write_colored(w, Color::Green, |w| write!(w, "passed"))?;

                write!(w, ", ")?;
                ui::write_bold(w, |w| write!(w, "{}", result.failed()))?;
                write!(w, " ")?;
                ui::write_colored(w, Color::Red, |w| write!(w, "failed"))?;
            }

            if result.filtered() != 0 {
                write!(w, ", ")?;
                ui::write_bold(w, |w| write!(w, "{}", result.filtered()))?;
                write!(w, " ")?;
                ui::write_colored(w, Color::Yellow, |w| write!(w, "filtered"))?;
            }

            if result.skipped() != 0 {
                write!(w, ", ")?;
                ui::write_bold(w, |w| write!(w, "{}", result.skipped()))?;
                write!(w, " ")?;
                ui::write_colored(w, Color::Yellow, |w| write!(w, "skipped"))?;
            }

            writeln!(w)?;

            Ok(())
        })?;

        // TODO(tinger): report failures, mean and avg time

        Ok(())
    }

    /// Clears the last line, i.e the status output.
    pub fn clear_status(&self) -> io::Result<()> {
        if !self.live {
            return Ok(());
        }

        write!(self.ui.stderr(), "\x1B[0F\x1B[0J")
    }

    /// Reports the current status of an ongoing test run.
    pub fn report_status(&self, result: &SuiteResult) -> io::Result<()> {
        if !self.live {
            return Ok(());
        }

        let mut w = self.ui.stderr();

        let duration = result.timestamp().elapsed();

        ui::write_annotated(&mut w, "", Color::Black, RUN_ANNOT_PADDING, |w| {
            write!(w, "[")?;
            ui::write_colored(
                w,
                duration_color(
                    duration
                        .checked_div(result.run() as u32)
                        .unwrap_or_default(),
                ),
                |w| write_duration(w, duration),
            )?;
            write!(w, "] ")?;

            ui::write_bold(w, |w| write!(w, "{}", result.run()))?;
            write!(w, "/")?;
            ui::write_bold(w, |w| write!(w, "{}", result.expected()))?;
            write!(w, " tests run: ")?;

            if result.passed() == result.total() {
                ui::write_bold(w, |w| write!(w, "all {}", result.passed()))?;
                write!(w, " ")?;
                ui::write_colored(w, Color::Green, |w| write!(w, "passed"))?;
            } else if result.failed() == result.total() {
                ui::write_bold(w, |w| write!(w, "all {}", result.failed()))?;
                write!(w, " ")?;
                ui::write_colored(w, Color::Red, |w| write!(w, "failed"))?;
            } else {
                ui::write_bold(w, |w| write!(w, "{}", result.passed()))?;
                write!(w, " ")?;
                ui::write_colored(w, Color::Green, |w| write!(w, "passed"))?;

                write!(w, ", ")?;
                ui::write_bold(w, |w| write!(w, "{}", result.failed()))?;
                write!(w, " ")?;
                ui::write_colored(w, Color::Red, |w| write!(w, "failed"))?;
            }

            if result.filtered() != 0 {
                write!(w, ", ")?;
                ui::write_bold(w, |w| write!(w, "{}", result.filtered()))?;
                write!(w, " ")?;
                ui::write_colored(w, Color::Yellow, |w| write!(w, "filtered"))?;
            }

            writeln!(w)?;

            Ok(())
        })?;

        Ok(())
    }

    /// Report that a test has passed.
    pub fn report_test_pass(
        &self,
        test: &Test,
        duration: Duration,
        warnings: &[SourceDiagnostic],
    ) -> eyre::Result<()> {
        ui::write_annotated(
            &mut self.ui.stderr(),
            "pass",
            Color::Green,
            RUN_ANNOT_PADDING,
            |w| {
                write!(w, "[")?;
                ui::write_colored(w, duration_color(duration), |w| write_duration(w, duration))?;
                write!(w, "] ")?;
                ui::write_test_id(w, test.id())?;
                writeln!(w)?;

                self.write_diagnostics(
                    w,
                    if self.warnings == When::Always {
                        warnings
                    } else {
                        &[]
                    },
                    &[],
                )?;

                Ok(())
            },
        )?;

        Ok(())
    }

    /// Report that a test has failed and show its output and failure reason.
    pub fn report_test_fail(
        &self,
        test: &Test,
        result: &TestResult,
        diff_hint: bool,
    ) -> eyre::Result<()> {
        ui::write_annotated(
            &mut self.ui.stderr(),
            "fail",
            Color::Red,
            RUN_ANNOT_PADDING,
            |w| {
                write!(w, "[")?;
                ui::write_colored(w, duration_color(result.duration()), |w| {
                    write_duration(w, result.duration())
                })?;
                write!(w, "] ")?;
                ui::write_test_id(w, test.id())?;
                writeln!(w)?;

                match result.kind() {
                    Some(TestResultKind::FailedCompilation { error, reference }) => {
                        writeln!(
                            w,
                            "Compilation of {} failed",
                            if *reference { "reference" } else { "test" },
                        )?;

                        self.write_diagnostics(
                            w,
                            if self.warnings != When::Never {
                                result.warnings()
                            } else {
                                &[]
                            },
                            if self.errors { &error.0 } else { &[] },
                        )?;
                    }
                    Some(TestResultKind::FailedComparison(compare::Error {
                        output,
                        reference,
                        pages,
                    })) => {
                        if output != reference {
                            writeln!(
                                w,
                                "Expected {reference} {}, got {output} {}",
                                Term::simple("page").with(*reference),
                                Term::simple("page").with(*output),
                            )?;
                        }

                        for (p, e) in pages {
                            let p = p + 1;
                            match e {
                                PageError::Dimensions { output, reference } => {
                                    writeln!(w, "Page {p} had different dimensions")?;
                                    w.write_with(2, |w| {
                                        writeln!(w, "Output: {}", output)?;
                                        writeln!(w, "Reference: {}", reference)
                                    })?;
                                }
                                PageError::SimpleDeviations { deviations } => {
                                    writeln!(
                                        w,
                                        "Page {p} had {deviations} {}",
                                        Term::simple("deviation").with(*deviations),
                                    )?;
                                }
                            }
                        }

                        if diff_hint {
                            ui::write_hint_with(w, None, |w| {
                                writeln!(
                                    w,
                                    "Diff images have been saved at '{}'",
                                    self.project.paths().test_diff_dir(test.id()).display()
                                )
                            })?;
                        }
                    }
                    _ => unreachable!(),
                }

                Ok(())
            },
        )?;

        Ok(())
    }

    fn write_diagnostics<W: WriteColor>(
        &self,
        writer: &mut W,
        warnings: &[SourceDiagnostic],
        errors: &[SourceDiagnostic],
    ) -> io::Result<()> {
        // TODO(tinger): don't use io::ErrorKind::Other

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
            .with_labels(
                resolve_label(self.world, diagnostic.span)
                    .into_iter()
                    .collect(),
            );

            term::emit(writer, &self.diagnostic_config, self.world, &diag)
                .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;

            // Stacktrace-like helper diagnostics.
            for point in &diagnostic.trace {
                let message = point.v.to_string();
                let help = Diagnostic::help()
                    .with_message(message)
                    .with_labels(resolve_label(self.world, point.span).into_iter().collect());

                term::emit(writer, &self.diagnostic_config, self.world, &help)
                    .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
            }
        }

        Ok(())
    }
}

fn resolve_label(world: &SystemWorld, span: Span) -> Option<Label<FileId>> {
    Some(Label::primary(span.id()?, world.range(span)?))
}

/// Writes a padded duration in human readable form
fn write_duration<W: Write>(w: &mut W, duration: Duration) -> io::Result<()> {
    let s = duration.as_secs();
    let ms = duration.subsec_millis();
    let us = duration.subsec_micros().saturating_sub(ms * 1000);

    write!(w, "{s: >2}s")?;
    write!(w, " {ms: >3}ms")?;
    write!(w, " {us: >3}µs")?;

    Ok(())
}

/// Returns the color to use for a test's duration.
fn duration_color(duration: Duration) -> Color {
    match duration.as_secs() {
        0 if duration.is_zero() => Color::Rgb(128, 128, 128),
        0 => Color::Green,
        1..=5 => Color::Yellow,
        _ => Color::Red,
    }
}
