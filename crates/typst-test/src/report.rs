use std::borrow::Cow;
use std::fmt::{Debug, Display};
use std::io::Write;
use std::path::PathBuf;
use std::{fmt, io};

use semver::Version;
use termcolor::{Color, ColorSpec, HyperlinkSpec, WriteColor};

use crate::project::test::{CompareFailure, Test, TestFailure, UpdateFailure};
use crate::project::Project;

pub const ANNOT_PADDING: usize = 7;
pub const MAX_TEST_LIST: usize = 10;

fn write_bold_colored<W: WriteColor + ?Sized>(
    w: &mut W,
    annot: impl Display,
    color: Color,
) -> io::Result<()> {
    w.set_color(ColorSpec::new().set_bold(true).set_fg(Some(color)))?;
    write!(w, "{annot}")?;
    w.set_color(ColorSpec::new().set_bold(false).set_fg(None))?;
    Ok(())
}

fn write_program_buffer(reporter: &mut Reporter, name: &str, buffer: &[u8]) -> io::Result<()> {
    if buffer.is_empty() {
        return Ok(());
    }

    let mut frame = ColorSpec::new();
    frame.set_bold(true);

    let mut no_frame = ColorSpec::new();
    no_frame.set_bold(false);

    let lossy = String::from_utf8_lossy(buffer);
    if matches!(lossy, Cow::Owned(_)) {
        reporter.hint(format!("{name} was not valid UTF8"))?;
    }

    reporter.set_color(&frame)?;
    writeln!(reporter, "┏━ {name}")?;
    reporter.set_color(&no_frame)?;
    for line in lossy.lines() {
        reporter.set_color(&frame)?;
        write!(reporter, "┃")?;
        reporter.set_color(&no_frame)?;
        writeln!(reporter, "{line}")?;
    }
    reporter.set_color(&frame)?;
    writeln!(reporter, "┗━ {name}")?;
    reporter.set_color(&no_frame)?;

    Ok(())
}

pub struct Reporter {
    writer: Box<dyn WriteColor + Send + Sync + 'static>,

    // fmt::Write indenting fields
    indents: Vec<isize>,
    need_indent: bool,
    last_io_on_fmt_error: Option<io::Error>,
}

impl Debug for Reporter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "..")
    }
}

impl Reporter {
    pub fn new<W: WriteColor + Send + Sync + 'static>(writer: W) -> Self {
        Self {
            writer: Box::new(writer),
            indents: vec![],
            need_indent: true,
            last_io_on_fmt_error: None,
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
        self.reset()?;
        Ok(())
    }

    pub fn hint(&mut self, hint: impl Display) -> io::Result<()> {
        self.set_color(ColorSpec::new().set_bold(true).set_fg(Some(Color::Cyan)))?;
        write!(self, "hint: ")?;
        self.set_color(ColorSpec::new().set_bold(false).set_fg(None))?;

        self.indent("hint: ".len() as isize);
        writeln!(self, "{hint}")?;
        self.dedent();

        Ok(())
    }

    pub fn test_result(&mut self, name: &str, annot: &str, color: Color) -> io::Result<()> {
        self.write_annot(annot, color)?;
        self.set_color(ColorSpec::new().set_bold(true))?;
        writeln!(self, " {name}")?;
        self.reset()
    }

    pub fn test_success(&mut self, test: &Test, annot: &str) -> io::Result<()> {
        self.test_result(test.name(), annot, Color::Green)
    }

    pub fn test_added(&mut self, test: &Test, no_ref: bool) -> io::Result<()> {
        self.test_result(test.name(), "added", Color::Green)?;

        if no_ref {
            let hint = format!(
                "Test template used, no default reference generated\nrun 'typst-test update --exact\
                {}' to accept test",
                    test.name(),
                );
            self.hint(&hint)?;
        }

        Ok(())
    }

    pub fn test_failure(&mut self, test: &Test, error: TestFailure) -> io::Result<()> {
        self.test_result(test.name(), "failed", Color::Red)?;
        self.indent((ANNOT_PADDING + 1) as isize);
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
                    "Expected {reference} page{}, got {output} page{}",
                    if reference == 1 { "" } else { "s" },
                    if output == 1 { "" } else { "s" },
                )?;
            }
            TestFailure::Comparison(CompareFailure::Page { pages, diff_dir }) => {
                for (p, _) in pages {
                    writeln!(self, "Page {p} did not match")?;
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
}

impl fmt::Write for Reporter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let pad = " ".repeat(0usize.saturating_add_signed(self.indents.iter().sum()));
        let mut buf = s.as_bytes();

        let res = (|| loop {
            if self.need_indent {
                match buf.iter().position(|&b| b != b'\n') {
                    None => break self.writer.write_all(buf),
                    Some(len) => {
                        let (head, tail) = buf.split_at(len);
                        self.writer.write_all(head)?;
                        self.writer.write_all(pad.as_bytes())?;
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
        })();

        match res {
            Ok(_) => Ok(()),
            Err(err) => {
                self.last_io_on_fmt_error = Some(err);
                Err(fmt::Error)
            }
        }
    }

    fn write_char(&mut self, c: char) -> fmt::Result {
        let pad = " ".repeat(0usize.saturating_add_signed(self.indents.iter().sum()));

        if self.need_indent {
            self.need_indent = if c != '\n' {
                match self.writer.write_all(pad.as_bytes()) {
                    Ok(_) => {}
                    Err(err) => {
                        self.last_io_on_fmt_error = Some(err);
                        return Err(fmt::Error);
                    }
                };
                false
            } else {
                true
            }
        }

        fmt::Write::write_char(self, c)
    }
}

impl Write for Reporter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.writer.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.writer.write_all(buf)
    }

    fn write_fmt(&mut self, fmt: fmt::Arguments<'_>) -> io::Result<()> {
        match fmt::Write::write_str(self, &fmt.to_string()) {
            Ok(_) => Ok(()),
            Err(err) => Err(if let Some(io_err) = self.last_io_on_fmt_error.take() {
                io_err
            } else {
                io::Error::other(err)
            }),
        }
    }
}

impl WriteColor for Reporter {
    fn supports_color(&self) -> bool {
        self.writer.supports_color()
    }

    fn set_color(&mut self, spec: &ColorSpec) -> io::Result<()> {
        self.writer.set_color(spec)
    }

    fn reset(&mut self) -> io::Result<()> {
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
