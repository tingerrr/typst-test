//! Comparison on rendered pages.

use std::fmt::{Debug, Display};

use thiserror::Error;
use tiny_skia::Pixmap;

use crate::stdx;
use crate::stdx::fmt::Term;

/// A struct representing page size in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Size {
    /// The width of the page.
    pub width: u32,

    /// The height of the page.
    pub height: u32,
}

impl Display for Size {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}

/// The strategy to use for visual comparison.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Strategy {
    /// Use a simple pixel channel difference comparison, setting both fields
    /// to `0` makes an exact comparison.
    Simple {
        /// The maximum allowed difference between a channel of two pixels
        /// before the pixel is considered different. A single channel mismatch
        /// is enough to mark a pixel as a deviation.
        max_delta: u8,

        /// The maximum allowed amount of pixels that can differ per page in
        /// accordance to `max_delta` before two pages are considered different.
        max_deviation: usize,
    },
}

impl Default for Strategy {
    fn default() -> Self {
        Self::Simple {
            max_delta: 0,
            max_deviation: 0,
        }
    }
}

/// Compares two pages individually using the given strategy.
pub fn page(output: &Pixmap, reference: &Pixmap, strategy: Strategy) -> Result<(), PageError> {
    match strategy {
        Strategy::Simple {
            max_delta,
            max_deviation,
        } => page_simple(output, reference, max_delta, max_deviation),
    }
}

/// Compares two pages individually using [`Strategy::Simple`].
fn page_simple(
    output: &Pixmap,
    reference: &Pixmap,
    max_delta: u8,
    max_deviation: usize,
) -> Result<(), PageError> {
    if output.width() != reference.width() || output.height() != reference.height() {
        return Err(PageError::Dimensions {
            output: Size {
                width: output.width(),
                height: output.height(),
            },
            reference: Size {
                width: reference.width(),
                height: reference.height(),
            },
        });
    }

    let deviations = Iterator::zip(output.pixels().iter(), reference.pixels().iter())
        .filter(|(a, b)| {
            u8::abs_diff(a.red(), b.red()) > max_delta
                || u8::abs_diff(a.green(), b.green()) > max_delta
                || u8::abs_diff(a.blue(), b.blue()) > max_delta
                || u8::abs_diff(a.alpha(), b.alpha()) > max_delta
        })
        .count();

    if deviations > max_deviation {
        return Err(PageError::SimpleDeviations { deviations });
    }

    Ok(())
}

/// An error describing why a document comparison failed.
#[derive(Debug, Clone, Error)]
pub struct Error {
    /// The output page count.
    pub output: usize,

    /// The reference page count.
    pub reference: usize,

    /// The page failures if there are any with their indices.
    pub pages: Vec<(usize, PageError)>,
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.output != self.reference {
            write!(
                f,
                "page count differed (out {} != ref {})",
                self.output, self.reference,
            )?;
        }

        if self.output != self.reference && self.pages.is_empty() {
            write!(f, " and ")?;
        }

        if self.pages.is_empty() {
            write!(
                f,
                "{} {} differed at indices: {:?}",
                self.pages.len(),
                stdx::fmt::Term::simple("page").with(self.pages.len()),
                self.pages.iter().map(|(n, _)| n).collect::<Vec<_>>()
            )?;
        }

        Ok(())
    }
}

/// An error describing why a page comparison failed.
#[derive(Debug, Clone, Error)]
pub enum PageError {
    /// The dimensions of the pages did not match.
    #[error("dimensions differed: out {output} != ref {reference}")]
    Dimensions {
        /// The size of the output page.
        output: Size,

        /// The size of the reference page.
        reference: Size,
    },

    /// The pages differed according to [`Strategy::Simple`].
    #[error(
        "content differed in at least {} {}",
        deviations,
        Term::simple("pixel").with(*deviations)
    )]
    SimpleDeviations {
        /// The amount of visual deviations, i.e. the amount of pixels which did
        /// not match according to the visual strategy.
        deviations: usize,
    },
}

#[cfg(test)]
mod tests {
    use tiny_skia::PremultipliedColorU8;

    use super::*;

    fn images() -> [Pixmap; 2] {
        let a = Pixmap::new(10, 1).unwrap();
        let mut b = Pixmap::new(10, 1).unwrap();

        let red = PremultipliedColorU8::from_rgba(128, 0, 0, 128).unwrap();
        b.pixels_mut()[0] = red;
        b.pixels_mut()[1] = red;
        b.pixels_mut()[2] = red;
        b.pixels_mut()[3] = red;

        [a, b]
    }

    #[test]
    fn test_page_simple_below_max_delta() {
        let [a, b] = images();
        assert!(page(
            &a,
            &b,
            Strategy::Simple {
                max_delta: 128,
                max_deviation: 0,
            },
        )
        .is_ok())
    }

    #[test]
    fn test_page_simple_below_max_devitation() {
        let [a, b] = images();
        assert!(page(
            &a,
            &b,
            Strategy::Simple {
                max_delta: 0,
                max_deviation: 5,
            },
        )
        .is_ok());
    }

    #[test]
    fn test_page_simple_above_max_devitation() {
        let [a, b] = images();
        assert!(matches!(
            page(
                &a,
                &b,
                Strategy::Simple {
                    max_delta: 0,
                    max_deviation: 0,
                },
            ),
            Err(PageError::SimpleDeviations { deviations: 4 })
        ))
    }
}
