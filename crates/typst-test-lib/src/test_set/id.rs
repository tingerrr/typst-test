//! Test set ids.

use std::borrow::Borrow;
use std::fmt::{self, Debug, Display};
use std::str::FromStr;

use ecow::EcoString;
use thiserror::Error;

/// An id for test sets.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Id(EcoString);

impl Id {
    /// Turns this string into an id.
    ///
    /// All ids must start at least one ascii alphabetic letter and contain only
    /// ascii alpha-numeric characters, underscores and minuses.
    ///
    /// # Examples
    /// ```
    /// # use typst_test_lib::test_set::Id;
    /// let id = Id::new("abc")?;
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    /// Returns an error if a component wasn't valid.
    pub fn new<S: Into<EcoString>>(string: S) -> Result<Self, ParseIdError> {
        let id = string.into();
        Self::validate(&id)?;

        Ok(Self(id))
    }

    /// Turns this string into an id without validating it.
    ///
    /// # Safety
    /// The caller must ensure that the given string is a valid id.
    pub unsafe fn new_unchecked(string: EcoString) -> Self {
        debug_assert!(Self::is_valid(&string));
        Self(string)
    }

    /// Whether the given string is a valid id.
    ///
    /// # Examples
    /// ```
    /// # use typst_test_lib::test_set::Id;
    /// assert!( Id::is_valid("abc"));
    /// assert!( Id::is_valid("ab"));
    /// assert!( Id::is_valid("a1"));
    /// assert!(!Id::is_valid("a+b")); // invalid character
    /// assert!(!Id::is_valid("1a"));  // invalid character
    /// ```
    pub fn is_valid(string: &str) -> bool {
        Self::validate(string).is_ok()
    }

    fn validate(string: &str) -> Result<(), ParseIdError> {
        if string.is_empty() {
            return Err(ParseIdError::Empty);
        }

        let mut chars = string.chars().peekable();
        if !chars.next().unwrap().is_ascii_alphabetic() {
            return Err(ParseIdError::InvalidChraracter);
        }

        if chars.peek().is_some()
            && !chars.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(ParseIdError::InvalidChraracter);
        }

        Ok(())
    }

    /// The full id as a `str`, this string is never empty.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Clones the inner [`EcoString`].
    pub fn to_inner(&self) -> EcoString {
        self.0.clone()
    }
}

impl AsRef<str> for Id {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for Id {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl FromStr for Id {
    type Err = ParseIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl Debug for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Debug::fmt(self.as_str(), f)
    }
}

impl Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(self.as_str(), f)
    }
}

impl PartialEq<str> for Id {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<Id> for str {
    fn eq(&self, other: &Id) -> bool {
        self == other.as_str()
    }
}

impl PartialEq<String> for Id {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<Id> for String {
    fn eq(&self, other: &Id) -> bool {
        self == other.as_str()
    }
}

impl PartialEq<EcoString> for Id {
    fn eq(&self, other: &EcoString) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<Id> for EcoString {
    fn eq(&self, other: &Id) -> bool {
        self == other.as_str()
    }
}

/// An error returned when parsing of an id fails.
#[derive(Debug, Error)]
pub enum ParseIdError {
    /// An id contained an invalid character.
    #[error("id contained an invalid character")]
    InvalidChraracter,

    /// An id contained empty or no fragments.
    #[error("id contained empty or no fragments")]
    Empty,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid() {
        assert!(Id::new("/a").is_err());
        assert!(Id::new("a/").is_err());
        assert!(Id::new("a+b").is_err());
        assert!(Id::new("a ").is_err());
        assert!(Id::new("1a").is_err());
        assert!(Id::new("").is_err());
    }
}
