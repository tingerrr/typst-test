use std::fmt::Display;

use thiserror::Error;

use super::render;
use crate::util;

pub mod structural;
pub mod visual;

// TODO: comparison erros should differ depending on the format and strategy
// implement this similar to the store page formats, currently they are to visual centric

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Strategy {
    Structural,
    Visual(render::Strategy, visual::Strategy),
}

impl Default for Strategy {
    fn default() -> Self {
        Self::Visual(render::Strategy::default(), visual::Strategy::default())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("page count differed: out ({output}) != ref ({reference})")]
    PageCount { output: usize, reference: usize },

    #[error(
        "{} {} differed {:?}",
        pages.len(),
        util::fmt::plural(pages.len(), "page"),
        pages.iter().map(|(n, _)| n).collect::<Vec<_>>()
    )]
    Page { pages: Vec<(usize, PageError)> },

    #[error("document mismatch")]
    Document,

    #[error("missing output")]
    MissingOutput,

    #[error("missing references")]
    MissingReferences,
}

#[derive(Debug, Error)]
pub enum PageError {
    #[error("dimensions differed: out {output} != ref {reference}")]
    Dimensions { output: Size, reference: Size },

    #[error(
        "content differed at {} {}",
        deviations,
        util::fmt::plural(*deviations, "pixel")
    )]
    Content { deviations: usize },

    #[error("structural mismatch")]
    Structure,
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
