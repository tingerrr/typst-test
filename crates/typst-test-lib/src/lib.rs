//! The core library of typst-test.

pub mod config;
pub mod doc;
pub mod library;
pub mod project;
pub mod stdx;
pub mod test;
pub mod test_set;

/// The tool name, this is used in various places like config file directories,
/// manifest tool sections, , and more.
pub const TOOL_NAME: &str = "typst-test";

#[cfg(test)]
pub mod _dev;
