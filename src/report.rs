use std::io;

use termcolor::{Color, ColorSpec, WriteColor};

use crate::project::test::{CompareFailure, TestFailure};

pub const MAX_PADDING: usize = 20;

fn write_bold_colored<W: WriteColor + ?Sized>(
    w: &mut W,
    annot: &str,
    color: Color,
) -> io::Result<()> {
    w.set_color(&ColorSpec::new().set_bold(true).set_fg(Some(color)))?;
    write!(w, "{annot}")?;
    w.reset()?;
    Ok(())
}

fn write_hint<W: WriteColor + ?Sized>(w: &mut W, pad: &str, hint: &str) -> io::Result<()> {
    write_bold_colored(w, &format!("{pad}hint: "), Color::Cyan)?;
    writeln!(w, "{}", hint)?;
    Ok(())
}

fn write_program_buffer<W: WriteColor + ?Sized>(
    w: &mut W,
    pad: &str,
    name: &str,
    buffer: &[u8],
) -> io::Result<()> {
    if buffer.is_empty() {
        return Ok(());
    }

    let mut frame_spec = ColorSpec::new();
    frame_spec.set_bold(true);

    if let Ok(s) = std::str::from_utf8(buffer) {
        w.set_color(&frame_spec)?;
        writeln!(w, "{pad}┏━ {name}")?;
        w.reset()?;
        for line in s.lines() {
            w.set_color(&frame_spec)?;
            write!(w, "{pad}┃")?;
            w.reset()?;
            writeln!(w, "{line}")?;
        }
        w.set_color(&frame_spec)?;
        writeln!(w, "{pad}┗━ {name}")?;
        w.reset()?;
    } else {
        writeln!(w, "{pad}{name} was not valid utf8:")?;
        writeln!(w, "{pad}{buffer:?}")?;
    }

    Ok(())
}

pub fn test<W: WriteColor + ?Sized>(
    w: &mut W,
    max_name_len: usize,
    name: &str,
    annot: (&str, Color),
    details: impl FnOnce(&str, &mut W) -> io::Result<()>,
) -> io::Result<()> {
    let pad = std::cmp::min(max_name_len, MAX_PADDING);
    write!(w, "{name:<pad$} ")?;

    write_bold_colored(w, annot.0, annot.1)?;
    writeln!(w)?;
    details(&" ".repeat(pad + 1), w)?;

    Ok(())
}

pub fn test_added<W: WriteColor + ?Sized>(w: &mut W, name: &str) -> io::Result<()> {
    test(w, 0, name, ("added", Color::Green), |_, _| Ok(()))
}

pub fn test_removed<W: WriteColor + ?Sized>(w: &mut W, name: &str) -> io::Result<()> {
    test(w, 0, name, ("removed", Color::Green), |_, _| Ok(()))
}

pub fn test_success<W: WriteColor + ?Sized>(
    w: &mut W,
    max_name_len: usize,
    name: &str,
) -> io::Result<()> {
    test(w, max_name_len, name, ("ok", Color::Green), |_, _| Ok(()))
}

pub fn test_failure<W: WriteColor + ?Sized>(
    w: &mut W,
    max_name_len: usize,
    name: &str,
    error: TestFailure,
) -> io::Result<()> {
    test(w, max_name_len, name, ("failed", Color::Red), |pad, w| {
        match error {
            TestFailure::Preparation(e) => writeln!(w, "{pad}{e}")?,
            TestFailure::Cleanup(e) => writeln!(w, "{pad}{e}")?,
            TestFailure::Compilation(e) => {
                writeln!(w, "{pad}compilation failed ({})", e.output.status)?;
                write_program_buffer(w, pad, "stdout", &e.output.stdout)?;
                write_program_buffer(w, pad, "stderr", &e.output.stderr)?;
            }
            TestFailure::Comparison(CompareFailure::PageCount { output, reference }) => {
                writeln!(
                    w,
                    "{pad}expected {reference} page{}, got {output} page{}",
                    if reference == 1 { "" } else { "s" },
                    if output == 1 { "" } else { "s" },
                )?;
            }
            TestFailure::Comparison(CompareFailure::Page { pages }) => {
                for (p, f) in pages {
                    writeln!(w, "{pad}page {p}: {f}")?;
                }
            }
            TestFailure::Comparison(CompareFailure::MissingOutput) => {
                writeln!(w, "{pad}no output generated")?;
            }
            TestFailure::Comparison(CompareFailure::MissingReferences) => {
                writeln!(w, "{pad}no references given")?;
                write_hint(
                    w,
                    pad,
                    &format!("use `typst-test update {name}` to accept the test output"),
                )?;
            }
        }

        Ok(())
    })
}
