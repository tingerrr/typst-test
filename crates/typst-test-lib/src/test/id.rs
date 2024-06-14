use std::borrow::{Borrow, Cow};
use std::fmt::Display;
use std::ops::Deref;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;

use ecow::EcoString;

/// An error returned when parsing of an identifier fails.
#[derive(Debug, thiserror::Error)]
pub enum ParseIdentifierError {
    /// An identifier contained an invalid fragment.
    #[error("identifier contained an invalid fragment")]
    InvalidFrament,

    /// An identifier was not valid UTF-8.
    #[error("identifier was not valid UTF-8")]
    NotUtf8,

    /// An identifier contained a reserved fragment.
    #[error("identifier contained a reserved fragment {0:?}")]
    Reserved(&'static str),

    /// An identifier contained empty or no fragments.
    #[error("identifier contained empty or no fragments")]
    Empty,
}

/// A test identifier, this is the full path from the test root directory, down
/// to the folder containing the test script.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Identifier {
    id: EcoString,
}

impl Identifier {
    /// Reserved components, these may not be used for test or module names.
    pub const RESERVED: &'static [&'static str] = &[
        super::REF_NAME,
        super::TEST_NAME,
        super::OUT_NAME,
        super::DIFF_NAME,
    ];

    /// The test component separator.
    pub const SEPARATOR: &'static str = "/";

    /// Turns this string into an identifier.
    ///
    /// All components must start at least one ascii alphabetic letter and
    /// contain only ascii alphanumeric characters, underscores and minuses.
    /// No component can be equal to the values in [`Identifier::RESERVED`].
    ///
    /// # Examples
    /// ```
    /// # use typst_test_lib::test::id::Identifier;
    /// let id = Identifier::from_path("a/b/c")?;
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    /// Returns an error if a component wasn't valid.
    pub fn new<S: Into<EcoString>>(id: S) -> Result<Self, ParseIdentifierError> {
        let id = id.into();

        for fragment in id.split(Self::SEPARATOR) {
            Self::check_component(fragment)?;
        }

        Ok(Self { id })
    }

    /// Turns this path into an identifier.
    ///
    /// All components must start at least one ascii alphabetic letter and
    /// contain only ascii alphanumeric characters, underscores and minuses.
    /// No component can be equal to the values in [`Identifier::RESERVED`].
    ///
    /// # Examples
    /// ```
    /// # use typst_test_lib::test::id::Identifier;
    /// let id = Identifier::from_path("a/b/c")?;
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    /// Returns an error if a component wasn't valid.
    pub fn from_path<S: AsRef<Path>>(id: S) -> Result<Self, ParseIdentifierError> {
        fn inner(path: &Path) -> Result<Identifier, ParseIdentifierError> {
            if path.as_os_str().is_empty() {
                return Err(ParseIdentifierError::Empty);
            }

            let mut id = EcoString::new();

            for component in path.components() {
                match component {
                    Component::RootDir => {}
                    Component::Prefix(_) | Component::CurDir | Component::ParentDir => {
                        return Err(ParseIdentifierError::InvalidFrament)
                    }
                    Component::Normal(component) => {
                        let fragment = component.to_str().ok_or(ParseIdentifierError::NotUtf8)?;
                        Identifier::check_component(fragment)?;
                        if !id.is_empty() {
                            id.push_str(Identifier::SEPARATOR);
                        }
                        id.push_str(fragment);
                    }
                }
            }

            Ok(Identifier { id })
        }

        inner(id.as_ref())
    }

    fn check_component(fragment: &str) -> Result<(), ParseIdentifierError> {
        if fragment.is_empty() {
            return Err(ParseIdentifierError::Empty);
        }

        if let Some(reserved) = Self::RESERVED.iter().find(|&&r| fragment == r) {
            return Err(ParseIdentifierError::Reserved(reserved));
        }

        let mut chars = fragment.chars().peekable();
        if !chars.next().unwrap().is_ascii_alphabetic() {
            return Err(ParseIdentifierError::InvalidFrament);
        }

        if chars.peek().is_some()
            && !chars.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(ParseIdentifierError::InvalidFrament);
        }

        Ok(())
    }

    /// Returns a reference to the full identifier.
    pub fn as_str(&self) -> &str {
        self.id.as_str()
    }

    /// Returns the name of this test, the last component of this identifier.
    /// Is never empty.
    ///
    /// # Examples
    /// ```
    /// # use typst_test_lib::test::id::Identifier;
    /// let id = Identifier::from_path("a/b/c")?;
    /// assert_eq!(id.name(), "c");
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn name(&self) -> &str {
        self.components().next_back().expect("is non-empty")
    }

    /// Returns the module containing the, all but the last component of this
    /// identifier. May be empty.
    ///
    /// # Examples
    /// ```
    /// # use typst_test_lib::test::id::Identifier;
    /// let id = Identifier::from_path("a/b/c")?;
    /// assert_eq!(id.module(), "a/b");
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn module(&self) -> &str {
        let mut c = self.components();
        _ = c.next_back().expect("is non-empty");
        c.rest
    }

    /// The components of this identifier, the corresponds to the path direcotry
    /// path of the test.
    ///
    /// # Examples
    /// ```
    /// # use typst_test_lib::test::id::Identifier;
    /// let id = Identifier::from_path("a/b/c")?;
    /// let mut components = id.components();
    /// assert_eq!(components.next(), Some("a"));
    /// assert_eq!(components.next(), Some("b"));
    /// assert_eq!(components.next(), Some("c"));
    /// assert_eq!(components.next(), None);
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn components(&self) -> Components<'_> {
        Components { rest: &self.id }
    }

    /// Turns this identifier into a path relative to the test directory root.
    pub fn to_path(&self) -> Cow<'_, Path> {
        let s = self.id.as_str();

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
        self.id.as_str()
    }
}

impl AsRef<str> for Identifier {
    fn as_ref(&self) -> &str {
        self.id.as_str()
    }
}

impl Borrow<str> for Identifier {
    fn borrow(&self) -> &str {
        self.id.as_str()
    }
}

impl FromStr for Identifier {
    type Err = ParseIdentifierError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl Display for Identifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.id.fmt(f)
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

    macro_rules! assert_components {
        ($ctor:ident, $val:expr, $comps:expr) => {
            assert_eq!(
                Identifier::$ctor($val)
                    .unwrap()
                    .components()
                    .collect::<Vec<_>>(),
                $comps,
            );
        };
    }

    #[test]
    fn test_components() {
        assert_eq!(
            Identifier { id: "a/b/c".into() }
                .components()
                .collect::<Vec<_>>(),
            ["a", "b", "c"]
        );
        assert_eq!(
            Identifier { id: "a/b/c".into() }
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
            assert_eq!(Identifier { id: id.into() }.name(), name);
        }
    }

    #[test]
    fn test_module() {
        let tests = [("a/b/c", "a/b"), ("a/b", "a"), ("a", "")];

        for (id, name) in tests {
            assert_eq!(Identifier { id: id.into() }.module(), name);
        }
    }

    #[test]
    fn test_str_valid() {
        assert_components!(new, "a/b/c", ["a", "b", "c"]);
        assert_components!(new, "a1", ["a1"]);
    }

    #[test]
    fn test_str_invalid() {
        assert!(Identifier::new("out").is_err());
        assert!(Identifier::new("/a").is_err());
        assert!(Identifier::new("a/").is_err());
        assert!(Identifier::new("a//b").is_err());

        assert!(Identifier::new("a ").is_err());
        assert!(Identifier::new("1a").is_err());
        assert!(Identifier::new("").is_err());
    }

    #[test]
    fn test_path_valid() {
        assert_components!(from_path, "a/b/c", ["a", "b", "c"]);
        assert_components!(from_path, "a//c", ["a", "c"]);
        assert_components!(from_path, "/a", ["a"]);
        assert_components!(from_path, "a/", ["a"]);
        assert_components!(from_path, "a1", ["a1"]);
    }

    #[test]
    fn test_path_invalid() {
        assert!(Identifier::from_path("out").is_err());
        assert!(Identifier::from_path("a ").is_err());
        assert!(Identifier::from_path("1a").is_err());
        assert!(Identifier::from_path("").is_err());
    }
}
