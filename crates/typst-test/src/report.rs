use std::borrow::Cow;
use std::fmt::{Debug, Display};
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;
use std::{fmt, io};

use semver::Version;
use termcolor::{Color, ColorSpec, HyperlinkSpec, WriteColor};

use crate::project::test::{CompareFailure, ComparePageFailure, Test, TestFailure, UpdateFailure};
use crate::project::Project;
use crate::util;

pub const ANNOT_PADDING: usize = 7;
pub const MAX_TEST_LIST: usize = 10;

pub struct Summary {
    pub total: usize,
    pub filtered: usize,
    pub compiled: usize,
    pub compared: Option<usize>,
    pub updated: Option<usize>,
    pub time: Duration,
}

impl Summary {
    pub fn run(&self) -> usize {
        self.total - self.filtered
    }

    pub fn is_ok(&self) -> bool {
        self.passed() == self.run()
    }

    pub fn is_total_fail(&self) -> bool {
        self.passed() == 0
    }

    pub fn passed(&self) -> usize {
        self.updated.or(self.compared).unwrap_or(self.compiled)
    }
}

fn write_with<W: WriteColor + ?Sized>(
    w: &mut W,
    set: impl FnOnce(&mut ColorSpec) -> &mut ColorSpec,
    unset: impl FnOnce(&mut ColorSpec) -> &mut ColorSpec,
    f: impl FnOnce(&mut W) -> io::Result<()>,
) -> io::Result<()> {
    w.set_color(set(&mut ColorSpec::new()))?;
    f(w)?;
    w.set_color(unset(&mut ColorSpec::new()))?;
    Ok(())
}

fn write_bold<W: WriteColor + ?Sized>(
    w: &mut W,
    f: impl FnOnce(&mut W) -> io::Result<()>,
) -> io::Result<()> {
    write_with(w, |c| c.set_bold(true), |c| c.set_bold(false), f)
}

fn write_bold_colored<W: WriteColor + ?Sized>(
    w: &mut W,
    annot: impl Display,
    color: Color,
) -> io::Result<()> {
    write_with(
        w,
        |c| c.set_bold(true).set_fg(Some(color)),
        |c| c.set_bold(false).set_fg(None),
        |w| write!(w, "{annot}"),
    )
}

fn write_program_buffer(reporter: &mut Reporter, name: &str, buffer: &[u8]) -> io::Result<()> {
    if buffer.is_empty() {
        return Ok(());
    }

    let lossy = String::from_utf8_lossy(buffer);
    if matches!(lossy, Cow::Owned(_)) {
        reporter.hint(format!("{name} was not valid UTF8"))?;
    }

    write_bold(reporter, |w| writeln!(w, "┏━ {name}"))?;
    for line in lossy.lines() {
        write_bold(reporter, |w| write!(w, "┃"))?;
        writeln!(reporter, "{line}")?;
    }
    write_bold(reporter, |w| writeln!(w, "┗━ {name}"))?;

    Ok(())
}

pub struct Reporter {
    writer: Box<dyn WriteColor + Send + Sync + 'static>,

    // fmt::Write indenting fields
    indents: Vec<isize>,
    need_indent: bool,
    spec: Option<ColorSpec>,
}

impl Debug for Reporter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reporter")
            .field("indets", &self.indents)
            .field("need_indent", &self.need_indent)
            .field("spec", &self.spec)
            .finish_non_exhaustive()
    }
}

impl Reporter {
    pub fn new<W: WriteColor + Send + Sync + 'static>(writer: W) -> Self {
        Self {
            writer: Box::new(writer),
            indents: vec![],
            need_indent: true,
            spec: None,
        }
    }

    pub fn indent(&mut self, indent: isize) {
        self.indents.push(indent);
    }

    pub fn dedent(&mut self) {
        self.indents.pop();
    }

    pub fn dedent_all(&mut self) {
        self.indents.clear();
    }

    pub fn write_annot(&mut self, annot: &str, color: Color) -> io::Result<()> {
        self.set_color(ColorSpec::new().set_bold(true).set_fg(Some(color)))?;
        write!(self, "{annot:>ANNOT_PADDING$}")?;
        self.set_color(ColorSpec::new().set_bold(false).set_fg(None))?;
        Ok(())
    }

    pub fn hint(&mut self, hint: impl Display) -> io::Result<()> {
        write_bold_colored(self, "hint: ", Color::Cyan)?;

        self.indent("hint: ".len() as isize);
        writeln!(self, "{hint}")?;
        self.dedent();

        Ok(())
    }

    pub fn test_result(&mut self, name: &str, annot: &str, color: Color) -> io::Result<()> {
        self.write_annot(annot, color)?;
        write_bold(self, |w| writeln!(w, " {name}"))
    }

    pub fn test_success(&mut self, test: &Test, annot: &str) -> io::Result<()> {
        self.test_result(test.name(), annot, Color::Green)
    }

    pub fn test_added(&mut self, test: &Test, no_ref: bool) -> io::Result<()> {
        self.test_result(test.name(), "added", Color::Green)?;

        if no_ref {
            self.indent(ANNOT_PADDING as isize + 1);
            let hint = format!(
                "Test template used, no default reference generated\nrun 'typst-test update --exact\
                {}' to accept test",
                test.name(),
            );
            self.hint(&hint)?;
            self.dedent();
        }

        Ok(())
    }

    pub fn test_failure(&mut self, test: &Test, error: TestFailure) -> io::Result<()> {
        self.test_result(test.name(), "failed", Color::Red)?;
        self.indent(ANNOT_PADDING as isize + 1);
        match error {
            TestFailure::Preparation(e) => writeln!(self, "{e}")?,
            TestFailure::Cleanup(e) => writeln!(self, "{e}")?,
            TestFailure::Compilation(e) => {
                writeln!(self, "Compilation failed ({})", e.output.status)?;
                write_program_buffer(self, "stdout", &e.output.stdout)?;
                write_program_buffer(self, "stderr", &e.output.stderr)?;
            }
            TestFailure::Comparison(CompareFailure::PageCount { output, reference }) => {
                writeln!(
                    self,
                    "Expected {reference} {}, got {output} {}",
                    util::fmt::plural(reference, "page"),
                    util::fmt::plural(output, "page"),
                )?;
            }
            TestFailure::Comparison(CompareFailure::Page { pages, diff_dir }) => {
                for (p, e) in pages {
                    match e {
                        ComparePageFailure::Dimensions { output, reference } => {
                            writeln!(self, "Page {p} had different dimensions")?;
                            self.indent(2);
                            writeln!(self, "Output: {}x{}", output.0, output.1)?;
                            writeln!(self, "Reference: {}x{}", reference.0, reference.1)?;
                            self.dedent();
                        }
                        ComparePageFailure::Content => {
                            writeln!(self, "Page {p} did not match")?;
                        }
                    }
                }

                if let Some(diff_dir) = diff_dir {
                    self.hint(&format!(
                        "Diff images have been saved at '{}'",
                        diff_dir.display()
                    ))?;
                }
            }
            TestFailure::Comparison(CompareFailure::MissingOutput) => {
                writeln!(self, "No output was generated")?;
            }
            TestFailure::Comparison(CompareFailure::MissingReferences) => {
                writeln!(self, "No references were found")?;
                self.hint(&format!(
                    "Use 'typst-test update --exact {}' to accept the test output",
                    test.name(),
                ))?;
            }
            TestFailure::Update(UpdateFailure::Optimize { error }) => {
                writeln!(self, "Failed to optimize image")?;
                writeln!(self, "{error}")?;
            }
        }
        self.dedent();

        Ok(())
    }

    pub fn package(&mut self, package: &str, version: Option<&Version>) -> io::Result<()> {
        write_bold_colored(self, package, Color::Cyan)?;
        if let Some(version) = version {
            write!(self, ":")?;
            write_bold_colored(self, version, Color::Cyan)?;
        }

        Ok(())
    }

    pub fn project(
        &mut self,
        project: &Project,
        typst: PathBuf,
        typst_path: Option<PathBuf>,
    ) -> io::Result<()> {
        if let Some(manifest) = project.manifest() {
            write!(self, " Project ┌ ")?;
            self.package(
                &manifest.package.name.to_string(),
                Some(&manifest.package.version),
            )?;
            writeln!(self)?;

            // TODO: list [tool.typst-test] settings
        } else {
            write!(self, " Project ┌ ")?;
            write_bold_colored(self, "none", Color::Yellow)?;
            writeln!(self)?;
        }

        write!(self, "Template ├ ")?;
        if project.template().is_some() {
            write_bold_colored(self, "found", Color::Green)?;
        } else {
            write_bold_colored(self, "not found", Color::Yellow)?;
            write!(
                self,
                " (looked at '{}')",
                project.tests_root_dir().join("template.typ").display()
            )?;
        }
        writeln!(self)?;

        write!(self, "   Typst ├ ")?;
        if let Some(path) = typst_path {
            write_bold_colored(self, path.display(), Color::Green)?;
        } else {
            write_bold_colored(self, "not found", Color::Red)?;
            write!(self, " (searched for '{}')", typst.display())?;
        }
        writeln!(self)?;

        let tests = project.tests();
        if tests.is_empty() {
            write!(self, "   Tests └ ")?;
            write_bold_colored(self, "none", Color::Cyan)?;
        } else if tests.len() <= MAX_TEST_LIST {
            write!(self, "   Tests ├ ")?;
            write_bold_colored(self, tests.len(), Color::Cyan)?;
            writeln!(self)?;
            for (idx, name) in project.tests().keys().enumerate() {
                if idx == tests.len() - 1 {
                    writeln!(self, "         └ {}", name)?;
                } else {
                    writeln!(self, "         │ {}", name)?;
                }
            }
        } else {
            write!(self, "   Tests └ ")?;
            write_bold_colored(self, tests.len(), Color::Cyan)?;
            writeln!(self)?;
        }

        Ok(())
    }

    pub fn test_start(&mut self, is_update: bool) -> io::Result<()> {
        write_bold(self, |w| {
            writeln!(
                w,
                "{} tests",
                if is_update { "Updating" } else { "Running" }
            )
        })
    }

    pub fn test_summary(&mut self, summary: Summary, is_update: bool) -> io::Result<()> {
        write_bold(self, |w| writeln!(w, "Summary"))?;
        self.indent(2);

        let color = if summary.is_ok() {
            Color::Green
        } else if summary.is_total_fail() {
            Color::Red
        } else {
            Color::Yellow
        };

        write_bold_colored(self, summary.passed(), color)?;
        write!(self, " / ")?;
        write_bold(self, |w| write!(w, "{}", summary.run()))?;
        write!(self, " {}.", if is_update { "updated" } else { "passed" })?;

        if summary.filtered != 0 {
            write!(self, " ")?;
            write_bold_colored(self, summary.filtered, Color::Yellow)?;
            write!(self, " filtered out.")?;
        }

        let secs = summary.time.as_secs();
        match (secs / 60, secs) {
            (0, 0) => {}
            (0, s) => writeln!(
                self,
                " took {s} {}",
                util::fmt::plural(s as usize, "second")
            )?,
            (m, s) => writeln!(
                self,
                " took {m} {} {s} {}",
                util::fmt::plural(m as usize, "minute"),
                util::fmt::plural(s as usize, "second")
            )?,
        }

        self.dedent();

        Ok(())
    }
}

impl fmt::Write for Reporter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_all(s.as_bytes()).map_err(|_| fmt::Error)
    }
}

impl Write for Reporter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // NOTE: not being able to write fully to stdout/stderr would be an fatal in any case, this
        // greatly simplifies code used for indentation
        self.write_all(buf).map(|_| buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }

    fn write_all(&mut self, mut buf: &[u8]) -> io::Result<()> {
        let spec = self.spec.clone().unwrap_or_default();
        let pad = " ".repeat(0usize.saturating_add_signed(self.indents.iter().sum()));

        loop {
            if self.need_indent {
                match buf.iter().position(|&b| b != b'\n') {
                    None => break self.writer.write_all(buf),
                    Some(len) => {
                        let (head, tail) = buf.split_at(len);
                        self.writer.write_all(head)?;
                        self.writer.reset()?;
                        self.writer.write_all(pad.as_bytes())?;
                        self.writer.set_color(&spec)?;
                        self.need_indent = false;
                        buf = tail;
                    }
                }
            } else {
                match buf.iter().position(|&b| b == b'\n') {
                    None => break self.writer.write_all(buf),
                    Some(len) => {
                        let (head, tail) = buf.split_at(len + 1);
                        self.writer.write_all(head)?;
                        self.need_indent = true;
                        buf = tail;
                    }
                }
            }
        }
    }
}

impl WriteColor for Reporter {
    fn supports_color(&self) -> bool {
        self.writer.supports_color()
    }

    fn set_color(&mut self, spec: &ColorSpec) -> io::Result<()> {
        self.spec = Some(spec.clone());
        self.writer.set_color(spec)
    }

    fn reset(&mut self) -> io::Result<()> {
        self.spec = None;
        self.writer.reset()
    }

    fn is_synchronous(&self) -> bool {
        self.writer.is_synchronous()
    }

    fn set_hyperlink(&mut self, link: &HyperlinkSpec) -> io::Result<()> {
        self.writer.set_hyperlink(link)
    }

    fn supports_hyperlinks(&self) -> bool {
        self.writer.supports_hyperlinks()
    }
}
