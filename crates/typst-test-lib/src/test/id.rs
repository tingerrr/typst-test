//! Test ids.

use std::borrow::{Borrow, Cow};
use std::fmt::{self, Debug, Display};
use std::ops::Deref;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;

use ecow::EcoString;
use thiserror::Error;

/// A test id, this is the relative path from the test root directory, down to
/// the folder containing the test script.
///
/// Each part of the path must be a simple id containing only ASCII
/// alpha-numeric characters, dashes `-` or underscores `_` and start with an
/// alphabetic character. This restriction may be lifted in the future.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
pub struct Id(EcoString);

impl Id {
    /// The test component separator.
    pub const SEPARATOR: &'static str = "/";
}

impl Id {
    /// Turns this string into an id.
    ///
    /// All components must start at least one ascii alphabetic letter and
    /// contain only ascii alphanumeric characters, underscores and minuses.
    ///
    /// # Examples
    /// ```
    /// # use typst_test_lib::test::Id;
    /// let id = Id::new("a/b/c")?;
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

    /// Turns this path into an id, this follows the same rules as
    /// [`Id::new`] with the additional constraint that paths must valid
    /// UTF-8.
    ///
    /// # Examples
    /// ```
    /// # use typst_test_lib::test::Id;
    /// let id = Id::new_from_path("a/b/c")?;
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    /// Returns an error if a component wasn't valid.
    pub fn new_from_path<P: AsRef<Path>>(path: P) -> Result<Self, ParseIdError> {
        fn inner(path: &Path) -> Result<Id, ParseIdError> {
            let mut id = String::new();

            for component in path.components() {
                match component {
                    Component::Normal(comp) => {
                        if let Some(comp) = comp.to_str() {
                            Id::validate_component(comp)?;

                            if !id.is_empty() {
                                id.push_str(Id::SEPARATOR);
                            }

                            id.push_str(comp);
                        } else {
                            return Err(ParseIdError::InvalidFragment);
                        }
                    }
                    _ => return Err(ParseIdError::InvalidFragment),
                }
            }

            Ok(Id(id.into()))
        }

        inner(path.as_ref())
    }

    /// Turns this string into an id without validating it.
    ///
    /// # Safety
    /// The caller must ensure that the given string is a valid id.
    pub unsafe fn new_unchecked(string: EcoString) -> Self {
        debug_assert!(Self::is_valid(&string));
        Self(string)
    }
}

impl Id {
    /// Whether the given string is a valid id.
    ///
    /// # Examples
    /// ```
    /// # use typst_test_lib::test::Id;
    /// assert!( Id::is_valid("a/b/c"));
    /// assert!( Id::is_valid("a/b"));
    /// assert!( Id::is_valid("a"));
    /// assert!(!Id::is_valid("a//b"));  // empty component
    /// assert!(!Id::is_valid("a/"));    // empty component
    /// ```
    pub fn is_valid<S: AsRef<str>>(string: S) -> bool {
        Self::validate(string).is_ok()
    }

    fn validate<S: AsRef<str>>(string: S) -> Result<(), ParseIdError> {
        for fragment in string.as_ref().split(Self::SEPARATOR) {
            Self::validate_component(fragment)?;
        }

        Ok(())
    }

    /// Whether the given string is a valid id component.
    ///
    /// # Examples
    /// ```
    /// # use typst_test_lib::test::Id;
    /// assert!( Id::is_component_valid("a"));
    /// assert!( Id::is_component_valid("a1"));
    /// assert!(!Id::is_component_valid("1a"));  // invalid char
    /// assert!(!Id::is_component_valid("a "));  // invalid char
    /// ```
    pub fn is_component_valid<S: AsRef<str>>(component: S) -> bool {
        Self::validate_component(component).is_ok()
    }

    // TODO(tinger): this seems to be the culprit of the 100% doc tests
    fn validate_component<S: AsRef<str>>(component: S) -> Result<(), ParseIdError> {
        let component = component.as_ref();

        if component.is_empty() {
            return Err(ParseIdError::Empty);
        }

        let mut chars = component.chars().peekable();
        if !chars.next().unwrap().is_ascii_alphabetic() {
            return Err(ParseIdError::InvalidFragment);
        }

        if chars.peek().is_some()
            && !chars.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(ParseIdError::InvalidFragment);
        }

        Ok(())
    }
}

impl Id {
    /// The full id as a `str`, this string is never empty.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Clones the inner [`EcoString`].
    pub fn to_inner(&self) -> EcoString {
        self.0.clone()
    }

    /// The name of this test, the last component of this id. This string is
    /// never empty.
    ///
    /// # Examples
    /// ```
    /// # use typst_test_lib::test::Id;
    /// let id = Id::new("a/b/c")?;
    /// assert_eq!(id.name(), "c");
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn name(&self) -> &str {
        self.components()
            .next_back()
            .expect("id is always non-empty")
    }

    /// The module containing the, all but the last component of this id. This
    /// string may be empty.
    ///
    /// # Examples
    /// ```
    /// # use typst_test_lib::test::Id;
    /// let id = Id::new("a/b/c")?;
    /// assert_eq!(id.module(), "a/b");
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn module(&self) -> &str {
        let mut c = self.components();
        _ = c.next_back().expect("id is always non-empty");
        c.rest
    }

    /// The ancestors of this id, this corresponds to the ancestors of the
    /// test's path.
    ///
    /// # Examples
    /// ```
    /// # use typst_test_lib::test::Id;
    /// let id = Id::new("a/b/c")?;
    /// let mut ancestors = id.ancestors();
    /// assert_eq!(ancestors.next(), Some("a/b/c"));
    /// assert_eq!(ancestors.next(), Some("a/b"));
    /// assert_eq!(ancestors.next(), Some("a"));
    /// assert_eq!(ancestors.next(), None);
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn ancestors(&self) -> Ancestors<'_> {
        Ancestors { rest: &self.0 }
    }

    /// The components of this id, this corresponds to the components of the
    /// test's path.
    ///
    /// # Examples
    /// ```
    /// # use typst_test_lib::test::Id;
    /// let id = Id::new("a/b/c")?;
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

    /// Turns this id into a path relative to the test directory root.
    pub fn to_path(&self) -> Cow<'_, Path> {
        let s = self.as_str();

        if Self::SEPARATOR == std::path::MAIN_SEPARATOR_STR {
            Cow::Borrowed(Path::new(s))
        } else {
            Cow::Owned(PathBuf::from(
                s.replace(Self::SEPARATOR, std::path::MAIN_SEPARATOR_STR),
            ))
        }
    }
}

impl Id {
    /// Adds the given component to this Id without checking if it is valid.
    ///
    /// # Safety
    /// The caller must ensure that the given component is valid.
    pub unsafe fn push_component_unchecked<S: AsRef<str>>(&mut self, component: S) {
        let comp = component.as_ref();
        self.0.push_str(Self::SEPARATOR);
        self.0.push_str(comp);
    }

    /// Tries to add the given component to this id.
    pub fn push_component<S: AsRef<str>>(&mut self, component: S) -> Result<(), ParseIdError> {
        let comp = component.as_ref();
        Self::validate_component(comp)?;

        // SAFETY: we validated above
        unsafe {
            self.push_component_unchecked(component);
        }

        Ok(())
    }

    /// Tries to add the given component to this id.
    pub fn push_path_component<P: AsRef<Path>>(
        &mut self,
        component: P,
    ) -> Result<(), ParseIdError> {
        self.push_component(
            component
                .as_ref()
                .to_str()
                .ok_or(ParseIdError::InvalidFragment)?,
        )
    }
}

impl Deref for Id {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0.as_str()
    }
}

impl AsRef<str> for Id {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl Borrow<str> for Id {
    fn borrow(&self) -> &str {
        self.0.as_str()
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

/// Returned by [`Id::ancestors`].
#[derive(Debug)]
pub struct Ancestors<'id> {
    rest: &'id str,
}

impl<'id> Iterator for Ancestors<'id> {
    type Item = &'id str;

    fn next(&mut self) -> Option<Self::Item> {
        let ret = self.rest;
        self.rest = self
            .rest
            .rsplit_once(Id::SEPARATOR)
            .map(|(rest, _)| rest)
            .unwrap_or("");

        if ret.is_empty() {
            return None;
        }

        Some(ret)
    }
}

/// Returned by [`Id::components`].
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
            .split_once(Id::SEPARATOR)
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
            .rsplit_once(Id::SEPARATOR)
            .unwrap_or(("", self.rest));
        self.rest = rest;

        Some(c)
    }
}

/// Returned by [`Id::new`][new] and [`Id::new_from_path`][new_from_path].
///
/// [new]: super::Id::new
/// [new_from_path]: super::Id::new_from_path
#[derive(Debug, Error)]
pub enum ParseIdError {
    /// An id contained an invalid fragment.
    #[error("id contained an invalid fragment")]
    InvalidFragment,

    /// An id contained empty or no fragments.
    #[error("id contained empty or no fragments")]
    Empty,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ancestors() {
        assert_eq!(
            Id::new("a/b/c").unwrap().ancestors().collect::<Vec<_>>(),
            ["a/b/c", "a/b", "a"]
        );
    }

    #[test]
    fn test_components() {
        assert_eq!(
            Id::new("a/b/c").unwrap().components().collect::<Vec<_>>(),
            ["a", "b", "c"]
        );
        assert_eq!(
            Id::new("a/b/c")
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
            assert_eq!(Id(id.into()).name(), name);
        }
    }

    #[test]
    fn test_module() {
        let tests = [("a/b/c", "a/b"), ("a/b", "a"), ("a", "")];

        for (id, name) in tests {
            assert_eq!(Id(id.into()).module(), name);
        }
    }

    #[test]
    fn test_str_invalid() {
        assert!(Id::new("/a").is_err());
        assert!(Id::new("a/").is_err());
        assert!(Id::new("a//b").is_err());

        assert!(Id::new("a ").is_err());
        assert!(Id::new("1a").is_err());
        assert!(Id::new("").is_err());
    }
}
