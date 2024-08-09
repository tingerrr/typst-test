//! This crate is internal to typst-test and should not be considered stable at
//! any moment. The various modules mirror the standard library where needed,
//! providing extensions or helper types/functions.

pub mod fmt;
pub mod fs;
pub mod result;

/// Common traits, types, functions and macros defined in this crate.
pub mod prelude {
    pub use crate::result::ResultEx;
}
