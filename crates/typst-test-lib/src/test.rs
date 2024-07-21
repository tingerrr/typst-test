//! In memory test represention.

use std::str::FromStr;

use ecow::EcoString;
use thiserror::Error;

pub mod id;

/// The kind of a [`Test`][crate::store::test::Test]'s reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReferenceKind {
    /// Ephemeral references are references which are compiled on the fly from a script.
    Ephemeral,

    /// Persistent references are pre compiled and fetched for comparison.
    Persistent,
}

/// An error which may occur while parsing an annotation.
#[derive(Debug, Error)]
pub enum ParseAnnotationError {
    /// The delimiter were missing or unclosed.
    #[error("the annotation had only one or no delimiter")]
    MissingDelimiter,

    /// The annotation identifier is unknown, invalid or empty.
    #[error("unknown or invalid annotation identifier: {0:?}")]
    Unknown(EcoString),

    /// The annotation did not expect any arguments but received some.
    #[error("the annotation {id} had unexpected arguments: {args:?}")]
    UnexpectedArguments { id: EcoString, args: EcoString },

    /// The anotation expected arguments but received none.
    #[error("the annotation {0} expected arguments but received none")]
    MissingArguments(EcoString),

    /// The annotation expected arguments, but did not receive the correct
    /// number or kind of arguments.
    #[error("the annotation {id} had invalid arguments: {args:?}")]
    InvalidArguments { id: EcoString, args: EcoString },

    /// The annotation was otherwise malformed.
    #[error("the annotation was malformed")]
    Other,
}

/// A test annotation used to configure test specific behavior.
///
/// Test annotations are placed on doc comments at the top of a test's source
/// file:
/// ```typst
/// /// [ignored]
/// /// [custom: foo]
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
    Ignored,

    /// A more general version of [`Self::Ignored`], which allows using the
    /// `custom()` test set, can be used more than once. Using `[custom: foo]`
    /// makes the test part of the `custom(foo)` test set.
    Custom(EcoString),
}

impl Annotation {
    /// Attempts to parse a whole annotation line.
    ///
    /// # Examples
    /// ```
    /// # use typst_test_lib::test::Annotation;
    /// let annot = Annotation::parse_line("///   [ ignored   ] ")?;
    /// assert_eq!(Annotation::Ignored);
    /// assert!(Annotation::parse_line("// [ignored]").is_err());
    /// assert!(Annotation::parse_line("/// [ignored").is_err());
    /// # Ok::<_, Box<dyn std::error::Error>(())
    /// ```
    pub fn parse_line(line: &str) -> Result<Self, ParseAnnotationError> {
        let Some(rest) = line.strip_prefix("///") else {
            return Err(ParseAnnotationError::Other);
        };

        rest.trim().parse()
    }
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

        let (id, args) = rest
            .trim()
            .split_once(":")
            .map(|(id, args)| (id.trim_end(), Some(args.trim_start())))
            .unwrap_or((rest.trim(), None));

        match id {
            "ignore" => match args {
                Some(args) => Err(ParseAnnotationError::UnexpectedArguments {
                    id: id.into(),
                    args: args.into(),
                }),
                None => Ok(Annotation::Ignored),
            },
            "custom" => match args {
                Some(args) => {
                    if args.chars().all(|c: char| c != ' ') {
                        Ok(Annotation::Custom(args.into()))
                    } else {
                        Err(ParseAnnotationError::InvalidArguments {
                            id: id.into(),
                            args: args.into(),
                        })
                    }
                }
                None => Err(ParseAnnotationError::MissingArguments(id.into())),
            },
            _ => Err(ParseAnnotationError::Unknown(id.into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_annotation_from_str() {
        assert_eq!(
            Annotation::from_str("[ignore]").unwrap(),
            Annotation::Ignored
        );
        assert_eq!(
            Annotation::from_str("[ ignore  ]").unwrap(),
            Annotation::Ignored
        );
        assert_eq!(
            Annotation::from_str("[custom : foo]").unwrap(),
            Annotation::Custom("foo".into())
        );
        assert_eq!(
            Annotation::from_str("[custom:bar]").unwrap(),
            Annotation::Custom("bar".into())
        );

        assert!(Annotation::from_str("[ ignored  ").is_err());
        assert!(Annotation::from_str("[custom : fo o]").is_err());
    }
}
