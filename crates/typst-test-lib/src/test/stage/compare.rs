use thiserror::Error;

use super::render;
use crate::util;

pub mod structural;
pub mod visual;

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

#[derive(Debug, Clone, Error)]
pub enum Failure {
    #[error("page count differed: out ({output}) != ref ({reference})")]
    PageCount { output: usize, reference: usize },

    #[error(
        "{} {} differed {:?}",
        pages.len(),
        util::fmt::plural(pages.len(), "page"),
        pages.iter().map(|(n, _)| n).collect::<Vec<_>>()
    )]
    Page { pages: Vec<(usize, PageFailure)> },

    #[error("document mismatch")]
    Document,

    #[error("missing output")]
    MissingOutput,

    #[error("missing references")]
    MissingReferences,
}

#[derive(Debug, Clone, Error)]
pub enum PageFailure {
    #[error("dimensions differed: out {output:?} != ref {reference:?}")]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}
