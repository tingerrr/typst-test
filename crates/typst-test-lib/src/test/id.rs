//! Test identifiers.

use std::borrow::{Borrow, Cow};
use std::fmt::Debug;
use std::fmt::Display;
use std::ops::Deref;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;

use ecow::EcoString;
use thiserror::Error;

/// An error returned when parsing of an identifier fails.
#[derive(Debug, Error)]
pub enum ParseIdentifierError {
    /// An identifier contained an invalid fragment.
    #[error("identifier contained an invalid fragment")]
    InvalidFragment,

    /// An identifier contained empty or no fragments.
    #[error("identifier contained empty or no fragments")]
    Empty,
}

/// A test identifier, this is the full path from the test root directory, down
/// to the folder containing the test script.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Identifier(EcoString);

impl Identifier {
    /// The test component separator.
    pub const SEPARATOR: &'static str = "/";

    /// Turns this string into an identifier.
    ///
    /// All components must start at least one ascii alphabetic letter and
    /// contain only ascii alphanumeric characters, underscores and minuses.
    ///
    /// # Examples
    /// ```
    /// # use typst_test_lib::test::id::Identifier;
    /// let id = Identifier::new("a/b/c")?;
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

    /// Turns this path into an identifier, this follows the same rules as
    /// [`Self::new`] with the additional constraint that paths must valid
    /// UTF-8.
    ///
    /// # Examples
    /// ```
    /// # use typst_test_lib::test::id::Identifier;
    /// let id = Identifier::new_from_path("a/b/c")?;
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    /// Returns an error if a component wasn't valid.
    pub fn new_from_path<P: AsRef<Path>>(path: P) -> Result<Self, ParseIdentifierError> {
        fn inner(path: &Path) -> Result<Identifier, ParseIdentifierError> {
            let mut id = String::new();

            for component in path.components() {
                match component {
                    Component::Normal(comp) => {
                        if let Some(comp) = comp.to_str() {
                            Identifier::validate_component(comp)?;

                            if !id.is_empty() {
                                id.push_str(Identifier::SEPARATOR);
                            }

                            id.push_str(comp);
                        } else {
                            return Err(ParseIdentifierError::InvalidFragment);
                        }
                    }
                    _ => return Err(ParseIdentifierError::InvalidFragment),
                }
            }

            Ok(Identifier(id.into()))
        }

        inner(path.as_ref())
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
    /// # Examples
    /// ```
    /// # use typst_test_lib::test::id::Identifier;
    /// assert!( Identifier::is_valid("a/b/c"));
    /// assert!( Identifier::is_valid("a/b"));
    /// assert!( Identifier::is_valid("a"));
    /// assert!(!Identifier::is_valid("a//b")); // empty component
    /// assert!(!Identifier::is_valid("a/"));   // empty component
    /// ```
    pub fn is_valid(string: &str) -> bool {
        Self::validate(string).is_ok()
    }

    fn validate(string: &str) -> Result<(), ParseIdentifierError> {
        for fragment in string.split(Self::SEPARATOR) {
            Self::validate_component(fragment)?;
        }

        Ok(())
    }

    /// Returns whether the given string is a valid identifier component.
    ///
    /// # Examples
    /// ```
    /// # use typst_test_lib::test::id::Identifier;
    /// assert!( Identifier::is_component_valid("a"));
    /// assert!( Identifier::is_component_valid("a1"));
    /// assert!(!Identifier::is_component_valid("1a")); // invalid char
    /// assert!(!Identifier::is_component_valid("a ")); // invalid char
    /// ```
    pub fn is_component_valid(component: &str) -> bool {
        Self::validate_component(component).is_ok()
    }

    fn validate_component(component: &str) -> Result<(), ParseIdentifierError> {
        if component.is_empty() {
            return Err(ParseIdentifierError::Empty);
        }

        let mut chars = component.chars().peekable();
        if !chars.next().unwrap().is_ascii_alphabetic() {
            return Err(ParseIdentifierError::InvalidFragment);
        }

        if chars.peek().is_some()
            && !chars.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(ParseIdentifierError::InvalidFragment);
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

    /// Returns the name of this test, the last component of this identifier.
    /// Is never empty.
    ///
    /// # Examples
    /// ```
    /// # use typst_test_lib::test::id::Identifier;
    /// let id = Identifier::new("a/b/c")?;
    /// assert_eq!(id.name(), "c");
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn name(&self) -> &str {
        self.components()
            .next_back()
            .expect("identifier is always non-empty")
    }

    /// Returns the module containing the, all but the last component of this
    /// identifier. May be empty.
    ///
    /// # Examples
    /// ```
    /// # use typst_test_lib::test::id::Identifier;
    /// let id = Identifier::new("a/b/c")?;
    /// assert_eq!(id.module(), "a/b");
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn module(&self) -> &str {
        let mut c = self.components();
        _ = c.next_back().expect("identifier is always non-empty");
        c.rest
    }

    /// The components of this identifier, this corresponds to the components of
    /// the test's path.
    ///
    /// # Examples
    /// ```
    /// # use typst_test_lib::test::id::Identifier;
    /// let id = Identifier::new("a/b/c")?;
    /// let mut components = id.components();
    /// assert_eq!(components.next(), Some("a"));
    /// assert_eq!(components.next(), Some("b"));
    /// assert_eq!(components.next(), Some("c"));
    /// assert_eq!(components.next(), None);
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn components(&self) -> Components<'_> {
        Components { rest: &self.0 }
    }

    /// Turns this identifier into a path relative to the test directory root.
    pub fn to_path(&self) -> Cow<'_, Path> {
        let s = self.0.as_str();

        if Self::SEPARATOR == std::path::MAIN_SEPARATOR_STR {
            Cow::Borrowed(Path::new(s))
        } else {
            Cow::Owned(PathBuf::from(
                s.replace(Self::SEPARATOR, std::path::MAIN_SEPARATOR_STR),
            ))
        }
    }
}

impl Deref for Identifier {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0.as_str()
    }
}

impl AsRef<str> for Identifier {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl Borrow<str> for Identifier {
    fn borrow(&self) -> &str {
        self.0.as_str()
    }
}

impl FromStr for Identifier {
    type Err = ParseIdentifierError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
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

/// An iterator over components of an identifier, returned by
/// [`Identifier::components`].
#[derive(Debug)]
pub struct Components<'id> {
    rest: &'id str,
}

impl<'id> Iterator for Components<'id> {
    type Item = &'id str;

    fn next(&mut self) -> Option<Self::Item> {
        if self.rest.is_empty() {
            return None;
        }

        let (c, rest) = self
            .rest
            .split_once(Identifier::SEPARATOR)
            .unwrap_or((self.rest, ""));
        self.rest = rest;

        Some(c)
    }
}

impl<'id> DoubleEndedIterator for Components<'id> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.rest.is_empty() {
            return None;
        }

        let (rest, c) = self
            .rest
            .rsplit_once(Identifier::SEPARATOR)
            .unwrap_or(("", self.rest));
        self.rest = rest;

        Some(c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_components() {
        assert_eq!(
            Identifier::new("a/b/c")
                .unwrap()
                .components()
                .collect::<Vec<_>>(),
            ["a", "b", "c"]
        );
        assert_eq!(
            Identifier::new("a/b/c")
                .unwrap()
                .components()
                .rev()
                .collect::<Vec<_>>(),
            ["c", "b", "a"]
        );
    }

    #[test]
    fn test_name() {
        let tests = [("a/b/c", "c"), ("a/b", "b"), ("a", "a")];

        for (id, name) in tests {
            assert_eq!(Identifier(id.into()).name(), name);
        }
    }

    #[test]
    fn test_module() {
        let tests = [("a/b/c", "a/b"), ("a/b", "a"), ("a", "")];

        for (id, name) in tests {
            assert_eq!(Identifier(id.into()).module(), name);
        }
    }

    #[test]
    fn test_str_invalid() {
        assert!(Identifier::new("/a").is_err());
        assert!(Identifier::new("a/").is_err());
        assert!(Identifier::new("a//b").is_err());

        assert!(Identifier::new("a ").is_err());
        assert!(Identifier::new("1a").is_err());
        assert!(Identifier::new("").is_err());
    }
}
