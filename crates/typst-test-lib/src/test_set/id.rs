//! Test set identifiers.

use std::borrow::Borrow;
use std::fmt::{Debug, Display};

use ecow::EcoString;
use thiserror::Error;

/// An error returned when parsing of an identifier fails.
#[derive(Debug, Error)]
pub enum ParseIdentifierError {
    /// An identifier contained an invalid character.
    #[error("identifier contained an invalid character")]
    InvalidChraracter,

    /// An identifier contained empty or no fragments.
    #[error("identifier contained empty or no fragments")]
    Empty,
}

/// An identifier for test sets.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Identifier(EcoString);

impl Identifier {
    /// Turns this string into an identifier.
    ///
    /// All identifiers must start at least one ascii alphabetic letter and
    /// contain only ascii alphanumeric characters, underscores and minuses.
    ///
    /// # Examples
    /// ```
    /// # use typst_test_lib::test_set::id::Identifier;
    /// let id = Identifier::new("abc")?;
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    /// Returns an error if a component wasn't valid.
    pub fn new<S: Into<EcoString>>(string: S) -> Result<Self, ParseIdentifierError> {
        let id = string.into();
        Self::validate(&id)?;

        Ok(Self(id))
    }

    /// Turns this string into an identifier without validating it.
    ///
    /// # Safety
    /// The caller must ensure that the given string is a valid identifier.
    pub unsafe fn new_unchecked(string: EcoString) -> Self {
        debug_assert!(Self::is_valid(&string));
        Self(string)
    }

    /// Returns whether the given string is a valid identifier.
    ///
    /// # Exmaples
    /// ```
    /// # use typst_test_lib::test_set::id::Identifier;
    /// assert!( Identifier::is_valid("abc"));
    /// assert!( Identifier::is_valid("ab"));
    /// assert!( Identifier::is_valid("a1"));
    /// assert!(!Identifier::is_valid("a+b")); // invalid character
    /// assert!(!Identifier::is_valid("1a"));  // invalid character
    /// ```
    pub fn is_valid(string: &str) -> bool {
        Self::validate(string).is_ok()
    }

    fn validate(string: &str) -> Result<(), ParseIdentifierError> {
        if string.is_empty() {
            return Err(ParseIdentifierError::Empty);
        }

        let mut chars = string.chars().peekable();
        if !chars.next().unwrap().is_ascii_alphabetic() {
            return Err(ParseIdentifierError::InvalidChraracter);
        }

        if chars.peek().is_some()
            && !chars.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(ParseIdentifierError::InvalidChraracter);
        }

        Ok(())
    }

    /// Returns a reference to the full identifier.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Returns the inner [`EcoString`], performing a cheap clone.
    pub fn to_inner(&self) -> EcoString {
        self.0.clone()
    }
}

impl AsRef<str> for Identifier {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for Identifier {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl Debug for Identifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self.as_str(), f)
    }
}

impl Display for Identifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self.as_str(), f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid() {
        assert!(Identifier::new("/a").is_err());
        assert!(Identifier::new("a/").is_err());
        assert!(Identifier::new("a+b").is_err());
        assert!(Identifier::new("a ").is_err());
        assert!(Identifier::new("1a").is_err());
        assert!(Identifier::new("").is_err());
    }
}
