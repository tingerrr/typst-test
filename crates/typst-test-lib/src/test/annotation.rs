//! Inline annotations of tests.

use std::str::FromStr;

use ecow::EcoString;
use thiserror::Error;

/// An error which may occur while parsing an annotation.
#[derive(Debug, Error)]
pub enum ParseAnnotationError {
    /// The delimiter were missing or unclosed.
    #[error("the annotation had only one or no delimiter")]
    MissingDelimiter,

    /// The annotation identifier is unknown, invalid or empty.
    #[error("unknown or invalid annotation identifier: {0:?}")]
    Unknown(EcoString),

    /// The annotation was otherwise malformed.
    #[error("the annotation was malformed")]
    Other,
}

/// A test annotation used to configure test specific behavior.
///
/// Test annotations are placed on doc comments at the top of a test's source
/// file:
/// ```typst
/// /// [skip]
///
/// #set page("a4")
/// ...
/// ```
///
/// Each annotation is on it's own line and may have optional arguments.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Annotation {
    /// The ignored annotation, this can be used to exclude a test by virtue of
    /// the `ignored` test set.
    Skip,
}

impl FromStr for Annotation {
    type Err = ParseAnnotationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let Some(rest) = s.strip_prefix('[') else {
            return Err(ParseAnnotationError::MissingDelimiter);
        };

        let Some(rest) = rest.strip_suffix(']') else {
            return Err(ParseAnnotationError::MissingDelimiter);
        };

        let id = rest.trim();

        match id {
            "skip" => Ok(Annotation::Skip),
            _ => Err(ParseAnnotationError::Unknown(id.into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_annotation_from_str() {
        assert_eq!(Annotation::from_str("[skip]").unwrap(), Annotation::Skip);
        assert_eq!(Annotation::from_str("[ skip  ]").unwrap(), Annotation::Skip);

        assert!(Annotation::from_str("[ skip  ").is_err());
        assert!(Annotation::from_str("[unknown]").is_err());
    }
}
