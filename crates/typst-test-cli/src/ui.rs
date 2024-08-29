#![allow(dead_code)]

use std::fmt::{Debug, Display};
use std::io::{BufRead, IsTerminal, Stdin, StdinLock, Write};
use std::{fmt, io};

/// The maximum needed padding to align all standard annotations. The longest of
/// which is currently `warning:` at 8 bytes.
///
/// This is used in all annotated messages of [`Ui`].
pub const ANNOTATION_MAX_PADDING: usize = 8;

use termcolor::{
    Color, ColorChoice, ColorSpec, HyperlinkSpec, StandardStream, StandardStreamLock, WriteColor,
};

#[derive(Debug)]
pub struct Ui {
    stdin: Stdin,
    stdout: StandardStream,
    stderr: StandardStream,
}

fn check_terminal<T: IsTerminal>(t: T, choice: ColorChoice) -> ColorChoice {
    match choice {
        // NOTE: when we use auto and the stream is not a terminal, we disable
        // it since termcolor does not check for this, in any other case we let
        // termcolor figure out what to do
        ColorChoice::Auto if !t.is_terminal() => ColorChoice::Never,
        other => other,
    }
}

impl Ui {
    /// Creates a new `Ui`.
    pub fn new(out: ColorChoice, err: ColorChoice) -> Self {
        Self {
            stdin: io::stdin(),
            stdout: StandardStream::stdout(check_terminal(io::stdout(), out)),
            stderr: StandardStream::stderr(check_terminal(io::stderr(), err)),
        }
    }

    /// Returns an exclusive lock to stdin.
    pub fn stdin(&self) -> StdinLock<'_> {
        self.stdin.lock()
    }

    /// Returns an exclusive lock to stdout.
    pub fn stdout(&self) -> StandardStreamLock<'_> {
        self.stdout.lock()
    }

    /// Returns an exclusive lock to stderr.
    pub fn stderr(&self) -> StandardStreamLock<'_> {
        self.stderr.lock()
    }

    /// Writes the given closure with an error annotation header.
    pub fn error_with(
        &self,
        f: impl FnOnce(&mut Indented<&mut StandardStreamLock<'_>>) -> io::Result<()>,
    ) -> io::Result<()> {
        write_error_with(&mut self.stderr(), ANNOTATION_MAX_PADDING, f)
    }

    /// Writes the given closure with a warning annotation header.
    pub fn warning_with(
        &self,
        f: impl FnOnce(&mut Indented<&mut StandardStreamLock<'_>>) -> io::Result<()>,
    ) -> io::Result<()> {
        write_warning_with(&mut self.stderr(), ANNOTATION_MAX_PADDING, f)
    }

    /// Writes the given closure with a hint annotation header.
    pub fn hint_with(
        &self,
        f: impl FnOnce(&mut Indented<&mut StandardStreamLock<'_>>) -> io::Result<()>,
    ) -> io::Result<()> {
        write_hint_with(&mut self.stderr(), ANNOTATION_MAX_PADDING, f)
    }

    /// Writes the given closure with an error annotation header.
    pub fn error_hinted_with(
        &self,
        f: impl FnOnce(&mut Indented<&mut StandardStreamLock<'_>>) -> io::Result<()>,
        h: impl FnOnce(&mut Indented<&mut StandardStreamLock<'_>>) -> io::Result<()>,
    ) -> io::Result<()> {
        write_error_with(&mut self.stderr(), ANNOTATION_MAX_PADDING, f)?;
        write_hint_with(&mut self.stderr(), ANNOTATION_MAX_PADDING, h)
    }

    /// Writes the given closure with a warning annotation header.
    pub fn warning_hinted_with(
        &self,
        f: impl FnOnce(&mut Indented<&mut StandardStreamLock<'_>>) -> io::Result<()>,
        h: impl FnOnce(&mut Indented<&mut StandardStreamLock<'_>>) -> io::Result<()>,
    ) -> io::Result<()> {
        write_warning_with(&mut self.stderr(), ANNOTATION_MAX_PADDING, f)?;
        write_hint_with(&mut self.stderr(), ANNOTATION_MAX_PADDING, h)
    }

    /// A shorthand for [`Self::error_with`].
    pub fn error(&self, message: impl Display) -> io::Result<()> {
        self.error_with(|w| writeln!(w, "{message}"))
    }

    /// A shorthand for [`Self::warning_with`].
    pub fn warning(&self, message: impl Display) -> io::Result<()> {
        self.warning_with(|w| writeln!(w, "{message}"))
    }

    /// A shorthand for [`Self::hint_with`].
    pub fn hint(&self, message: impl Display) -> io::Result<()> {
        self.hint_with(|w| writeln!(w, "{message}"))
    }

    /// Writes a hinted error to stderr.
    pub fn error_hinted(&self, message: impl Display, hint: impl Display) -> io::Result<()> {
        self.error_hinted_with(|w| writeln!(w, "{message}"), |w| writeln!(w, "{hint}"))
    }

    /// Writes a hinted warning to stderr.
    pub fn warning_hinted(&self, message: impl Display, hint: impl Display) -> io::Result<()> {
        self.warning_hinted_with(|w| writeln!(w, "{message}"), |w| writeln!(w, "{hint}"))
    }

    /// Returns whether or not a prompt can be displayed.
    pub fn can_prompt(&self) -> bool {
        io::stderr().is_terminal()
    }

    /// Prompts the user for input with the given prompt on stderr.
    pub fn prompt_with(
        &self,
        prompt: impl FnOnce(&mut dyn WriteColor) -> io::Result<()>,
    ) -> anyhow::Result<String> {
        if !self.can_prompt() {
            anyhow::bail!(io::Error::new(
                io::ErrorKind::Unsupported,
                "Cannot prompt for input since the output is not connected to a terminal",
            ));
        }

        let mut stderr = self.stderr();
        let mut stdin = self.stdin();

        prompt(&mut stderr)?;
        stderr.flush()?;

        let mut buffer = String::new();
        stdin.read_line(&mut buffer)?;

        if buffer.strip_suffix('\n').is_some() {
            buffer.pop();
        } else if buffer.is_empty() {
            anyhow::bail!(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Prompt cancelled by EOF",
            ));
        }

        Ok(buffer)
    }

    /// A shorthand for [`Self::prompt_with`] for confirmations.
    pub fn prompt_yes_no(
        &self,
        prompt: impl Display,
        default: impl Into<Option<bool>>,
    ) -> anyhow::Result<bool> {
        let default = default.into();
        let def = match default {
            Some(true) => "Y/n",
            Some(false) => "y/N",
            None => "y/n",
        };

        let res = self.prompt_with(|err| write!(err, "{prompt} [{def}]: "))?;

        Ok(match &res[..] {
            "" => default.ok_or_else(|| anyhow::anyhow!("expected [y]es or [n]o, got nothing"))?,
            "y" | "Y" => true,
            "n" | "N" => false,
            _ => {
                if res.eq_ignore_ascii_case("yes") {
                    true
                } else if res.eq_ignore_ascii_case("no") {
                    false
                } else {
                    anyhow::bail!("expected [y]es or [n]o, got: {res:?}");
                }
            }
        })
    }

    /// Flushes and resets both output streams.
    pub fn flush(&self) -> io::Result<()> {
        let mut out = self.stdout();
        let mut err = self.stderr();

        out.reset()?;
        write!(out, "")?;

        err.reset()?;
        write!(err, "")?;

        Ok(())
    }
}

/// Executes the given closure with custom set and reset style closures.
pub fn write_with<W: WriteColor + ?Sized>(
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

/// A shorthand for [`write_with`] which writes bold.
pub fn write_bold<W: WriteColor + ?Sized>(
    w: &mut W,
    f: impl FnOnce(&mut W) -> io::Result<()>,
) -> io::Result<()> {
    write_with(w, |c| c.set_bold(true), |c| c.set_bold(false), f)
}

/// A shorthand for [`write_with`] which writes with the given color.
pub fn write_colored<W: WriteColor + ?Sized>(
    w: &mut W,
    color: Color,
    f: impl FnOnce(&mut W) -> io::Result<()>,
) -> io::Result<()> {
    write_with(w, |c| c.set_fg(Some(color)), |c| c.set_fg(None), f)
}

/// A shorthand for [`write_with`] which writes bold and with the given color.
pub fn write_bold_colored<W: WriteColor + ?Sized>(
    w: &mut W,
    color: Color,
    f: impl FnOnce(&mut W) -> io::Result<()>,
) -> io::Result<()> {
    write_with(
        w,
        |c| c.set_bold(true).set_fg(Some(color)),
        |c| c.set_bold(false).set_fg(None),
        f,
    )
}

/// A shorthand for [`write_bold_colored`] with cyan as the color.
pub fn write_ident<W: WriteColor + ?Sized>(
    w: &mut W,
    f: impl FnOnce(&mut W) -> io::Result<()>,
) -> io::Result<()> {
    write_with(
        w,
        |c| c.set_bold(true).set_fg(Some(Color::Cyan)),
        |c| c.set_bold(false).set_fg(None),
        f,
    )
}

/// Writes the given closure as an annotation, that is, it is written with a
/// header after which each line is indented by the header length.
///
/// The maximum hanging indent can be set.
pub fn write_annotated<W: WriteColor + ?Sized>(
    w: &mut W,
    header: &str,
    color: Color,
    max_align: impl Into<Option<usize>>,
    f: impl FnOnce(&mut Indented<&mut W>) -> io::Result<()>,
) -> io::Result<()> {
    let align = max_align.into().unwrap_or(header.len());
    write_bold_colored(w, color, |w| write!(w, "{header:>align$} "))?;

    // NOTE: when taking the indent from the header length, we need to account
    // for the additonal space
    f(&mut Indented::continued(w, align + 1))?;
    Ok(())
}

/// Writes the given closure with an error annotation header.
pub fn write_error_with<W: WriteColor + ?Sized>(
    w: &mut W,
    pad: impl Into<Option<usize>>,
    f: impl FnOnce(&mut Indented<&mut W>) -> io::Result<()>,
) -> io::Result<()> {
    write_annotated(w, "error:", Color::Red, pad, f)
}

/// Writes the given closure with a warning annotation header.
pub fn write_warning_with<W: WriteColor + ?Sized>(
    w: &mut W,
    pad: impl Into<Option<usize>>,
    f: impl FnOnce(&mut Indented<&mut W>) -> io::Result<()>,
) -> io::Result<()> {
    write_annotated(w, "warning:", Color::Yellow, pad, f)
}

/// Writes the given closure with a hint annotation header.
pub fn write_hint_with<W: WriteColor + ?Sized>(
    w: &mut W,
    pad: impl Into<Option<usize>>,
    f: impl FnOnce(&mut Indented<&mut W>) -> io::Result<()>,
) -> io::Result<()> {
    write_annotated(w, "hint:", Color::Cyan, pad, f)
}

/// A shorthand for [`write_error_with`].
pub fn write_error<W: WriteColor + ?Sized, M: Display>(
    w: &mut W,
    pad: impl Into<Option<usize>>,
    message: M,
) -> io::Result<()> {
    write_error_with(w, pad, |w| writeln!(w, "{message}"))
}

/// A shorthand for [`write_warning_with`].
pub fn write_warning<W: WriteColor + ?Sized, M: Display>(
    w: &mut W,
    pad: impl Into<Option<usize>>,
    message: M,
) -> io::Result<()> {
    write_warning_with(w, pad, |w| writeln!(w, "{message}"))
}

/// A shorthand for [`write_hint_with`].
pub fn write_hint<W: WriteColor + ?Sized, M: Display>(
    w: &mut W,
    pad: impl Into<Option<usize>>,
    message: M,
) -> io::Result<()> {
    write_hint_with(w, pad, |w| writeln!(w, "{message}"))
}

/// Writes the ANSI escape codes to clear the given number of last lines.
pub fn clear_last_lines<W: Write + ?Sized>(w: &mut W, lines: usize) -> io::Result<()> {
    if lines != 0 {
        write!(w, "\x1B[{}F\x1B[0J", lines)?;
    }

    Ok(())
}

#[derive(Debug)]
pub struct Counted<W> {
    /// The writer to write to.
    writer: W,

    /// The currently counted lines.
    lines: usize,
}

impl<W> Counted<W> {
    /// Creates a new writer which counts the number of lines printed.
    pub fn new(writer: W) -> Self {
        Self { writer, lines: 0 }
    }

    /// Returns a mutable reference to the inner writer.
    pub fn inner(&mut self) -> &mut W {
        &mut self.writer
    }

    /// Returns the number of lines since the last reset.
    pub fn lines(&self) -> usize {
        self.lines
    }

    /// Resets the line counter to `0`.
    pub fn reset_lines(&mut self) {
        self.lines = 0;
    }

    /// Returns the inner writer.
    pub fn into_inner(self) -> W {
        self.writer
    }
}

impl<W: WriteColor> fmt::Write for Counted<W> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_all(s.as_bytes()).map_err(|_| fmt::Error)
    }
}

impl<W: Write> Write for Counted<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.writer.write(buf).inspect(|&len| {
            self.lines += buf[..len].iter().filter(|&&b| b == b'\n').count();
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.writer.write_all(buf)?;
        self.lines += buf.iter().filter(|&&b| b == b'\n').count();
        Ok(())
    }
}

impl<W: WriteColor> WriteColor for Counted<W> {
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

#[derive(Debug)]
pub struct Indented<W> {
    /// The writer to write to.
    writer: W,

    /// The current indent.
    indent: usize,

    /// Whether an indent is required at the next newline.
    need_indent: bool,

    /// The color spec to reactivate after the next indent.
    spec: Option<ColorSpec>,
}

impl<W> Indented<W> {
    /// Creates a new writer which indents every non-empty line.
    pub fn new(writer: W, indent: usize) -> Self {
        Self {
            writer,
            indent,
            need_indent: true,
            spec: None,
        }
    }

    /// Creates a new writer which indents every non-empty line after the first
    /// one. This is useful for writers which start on a non-empty line.
    pub fn continued(writer: W, indent: usize) -> Self {
        Self {
            writer,
            indent,
            need_indent: false,
            spec: None,
        }
    }

    /// Returns a mutable reference to the inner writer.
    pub fn inner(&mut self) -> &mut W {
        &mut self.writer
    }

    /// Executes the given closure with an additional indent which is later reset.
    pub fn write_with<R>(&mut self, indent: usize, f: impl FnOnce(&mut Self) -> R) -> R {
        self.indent += indent;
        let res = f(self);
        self.indent -= indent;
        res
    }

    /// Returns the inner writer.
    pub fn into_inner(self) -> W {
        self.writer
    }
}

impl<W: WriteColor> fmt::Write for Indented<W> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_all(s.as_bytes()).map_err(|_| fmt::Error)
    }
}

impl<W: WriteColor> Write for Indented<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.write_all(buf).map(|_| buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }

    fn write_all(&mut self, mut buf: &[u8]) -> io::Result<()> {
        let pad = " ".repeat(self.indent);

        loop {
            if self.need_indent {
                match buf.iter().position(|&b| b != b'\n') {
                    None => break self.writer.write_all(buf),
                    Some(len) => {
                        let (head, tail) = buf.split_at(len);
                        self.writer.write_all(head)?;
                        if self.spec.is_some() {
                            self.writer.reset()?;
                        }
                        self.writer.write_all(pad.as_bytes())?;
                        if let Some(spec) = &self.spec {
                            self.writer.set_color(spec)?;
                        }
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

impl<W: WriteColor> WriteColor for Indented<W> {
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

#[derive(Debug)]
pub struct Heading<W> {
    /// The writer to write to.
    writer: Indented<W>,

    /// The heading to write once.
    heading: String,

    /// Whether the heading has been written.
    written: bool,

    /// The spec to write after the heading.
    spec: Option<ColorSpec>,
}

impl<W> Heading<W> {
    /// Creates a new writer which writes a heading on the first write and
    /// indents all subsequent lines.
    pub fn new<S: Display>(writer: W, heading: S) -> Self {
        Self {
            writer: Indented::new(writer, 2),
            heading: heading.to_string(),
            written: false,
            spec: None,
        }
    }

    /// Returns a mutable reference to the inner writer.
    pub fn inner(&mut self) -> &mut Indented<W> {
        &mut self.writer
    }

    /// Returns the inner writer.
    pub fn into_inner(self) -> Indented<W> {
        self.writer
    }
}

impl<W: WriteColor> fmt::Write for Heading<W> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_all(s.as_bytes()).map_err(|_| fmt::Error)
    }
}

impl<W: WriteColor> Write for Heading<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if !self.written {
            self.writer.set_color(ColorSpec::new().set_bold(true))?;
            self.writer.inner().write_all(self.heading.as_bytes())?;
            self.writer.reset()?;
            writeln!(self.writer.inner())?;
            if let Some(spec) = &self.spec {
                self.writer.set_color(spec)?;
            }
            self.written = true;
        }
        self.writer.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        if !self.written {
            self.writer.set_color(ColorSpec::new().set_bold(true))?;
            self.writer.inner().write_all(self.heading.as_bytes())?;
            self.writer.reset()?;
            writeln!(self.writer.inner())?;
            if let Some(spec) = &self.spec {
                self.writer.set_color(spec)?;
            }
            self.written = true;
        }
        self.writer.write_all(buf)
    }
}

impl<W: WriteColor> WriteColor for Heading<W> {
    fn supports_color(&self) -> bool {
        self.writer.supports_color()
    }

    fn set_color(&mut self, spec: &ColorSpec) -> io::Result<()> {
        self.spec = Some(spec.clone());
        if self.written {
            self.writer.set_color(spec)?;
        }

        Ok(())
    }

    fn reset(&mut self) -> io::Result<()> {
        self.spec = None;
        if self.written {
            self.writer.reset()?;
        }

        Ok(())
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

#[derive(Debug)]
pub struct Live<W> {
    /// The writer to write to.
    writer: W,

    /// The lines to clear before each live reporting.
    to_clear: usize,
}

impl<W> Live<W> {
    /// Creates a new writer, which will clear previous lines for each live write.
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            to_clear: 0,
        }
    }

    /// Returns a mutable reference to the inner writer.
    pub fn inner(&mut self) -> &mut W {
        &mut self.writer
    }

    /// Returns the inner writer.
    pub fn into_inner(self) -> W {
        self.writer
    }
}

impl<W: WriteColor> Live<W> {
    /// Clears the previously written lines and writes new ones using the given closure.
    pub fn live<F>(&mut self, f: F) -> io::Result<()>
    where
        F: FnOnce(&mut Counted<&mut W>) -> io::Result<()>,
    {
        clear_last_lines(&mut self.writer, self.to_clear)?;
        let mut w = Counted::new(&mut self.writer);
        f(&mut w)?;
        self.to_clear = w.lines();

        Ok(())
    }
}

#[allow(dead_code)]
fn assert_traits() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    assert_send::<Ui>();
    assert_sync::<Ui>();
}

#[cfg(test)]
mod tests {
    use insta::assert_snapshot;
    use termcolor::Ansi;

    use super::*;

    #[test]
    fn test_counted() {
        let mut w = Counted::new(vec![]);

        write!(w, "Hello\n\nWorld\n").unwrap();

        assert_eq!(w.lines(), 3);

        let w = w.into_inner();
        let str = std::str::from_utf8(&w).unwrap();
        assert_snapshot!(str);
    }

    #[test]
    fn test_indented() {
        let mut w = Indented::new(Ansi::new(vec![]), 2);

        write!(w, "Hello\n\nWorld\n").unwrap();

        let w = w.into_inner().into_inner();
        let str = std::str::from_utf8(&w).unwrap();
        assert_snapshot!(str);
    }

    #[test]
    fn test_indented_continued() {
        let mut w = Indented::continued(Ansi::new(vec![]), 2);

        write!(w, "Hello\n\nWorld\n").unwrap();

        let w = w.into_inner().into_inner();
        let str = std::str::from_utf8(&w).unwrap();
        assert_snapshot!(str);
    }

    #[test]
    fn test_indented_nested() {
        let mut w = Indented::new(Indented::new(Ansi::new(vec![]), 2), 2);

        write!(w, "Hello\n\nWorld\n").unwrap();

        let w = w.into_inner().into_inner().into_inner();
        let str = std::str::from_utf8(&w).unwrap();
        assert_snapshot!(str);
    }

    #[test]
    fn test_indented_set_color() {
        let mut w = Indented::new(Ansi::new(vec![]), 2);

        w.set_color(ColorSpec::new().set_bold(true)).unwrap();
        write!(w, "Hello\n\nWorld\n").unwrap();

        let w = w.into_inner().into_inner();
        let str = std::str::from_utf8(&w).unwrap();
        assert_snapshot!(str);
    }

    #[test]
    fn test_heading() {
        let mut w = Heading::new(Ansi::new(vec![]), "Heading");

        write!(w, "Hello\n\nWorld\n").unwrap();

        let w = w.into_inner().into_inner().into_inner();
        let str = std::str::from_utf8(&w).unwrap();
        assert_snapshot!(str);
    }

    #[test]
    fn test_heading_set_color() {
        let mut w = Heading::new(Ansi::new(vec![]), "Heading");

        w.set_color(ColorSpec::new().set_bold(true)).unwrap();
        write!(w, "Hello World").unwrap();

        let w = w.into_inner().into_inner().into_inner();
        let str = std::str::from_utf8(&w).unwrap();
        assert_snapshot!(str);
    }

    // TODO: test live
}
