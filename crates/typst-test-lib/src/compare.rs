use std::fmt::{Debug, Display};

use thiserror::Error;

use super::render;
use crate::util;

pub mod visual;

// TODO: comparison errors should differ depending on the format and strategy
// implement this similar to the store page formats, currently they are to visual centric

/// The comparison strategy for test output, currently only supports rendering
/// and comparing visually.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Strategy {
    /// Use visual comparison of raster images, with the given render strategy
    /// if necessary.
    Visual(render::Strategy, visual::Strategy),
}

impl Default for Strategy {
    fn default() -> Self {
        Self::Visual(render::Strategy::default(), visual::Strategy::default())
    }
}

/// An error describing why a document.
#[derive(Debug, thiserror::Error)]
pub struct Error {
    /// The output page count.
    pub output: usize,

    /// The refernce page count.
    pub reference: usize,

    /// The page failures if there are any.
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
                "{} {} differed {:?}",
                self.pages.len(),
                util::fmt::plural(self.pages.len(), "page"),
                self.pages.iter().map(|(n, _)| n).collect::<Vec<_>>()
            )?;
        }

        Ok(())
    }
}

/// An error describing why a page comparison failed.
#[derive(Debug, Error)]
pub enum PageError {
    /// The dimensions of the pages did not match.
    #[error("dimensions differed: out {output} != ref {reference}")]
    Dimensions { output: Size, reference: Size },

    /// The pages differed according to [`Strategy::Simple`].
    #[error(
        "content differed in at least {} {}",
        deviations,
        util::fmt::plural(*deviations, "pixel")
    )]
    SimpleDeviations { deviations: usize },
}

/// A struct representing page size in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Size {
    /// The width of the page.
    pub width: u32,

    /// Thenheight of the page.
    pub height: u32,
}

impl Display for Size {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}