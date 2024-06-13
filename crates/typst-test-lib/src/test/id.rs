use std::fmt::Display;
use std::ops::Deref;
use std::path::{Component, Path};
use std::str::FromStr;

use ecow::EcoString;

#[derive(Debug, thiserror::Error)]
pub enum IdentifierError {
    #[error("identifier contained an invalid fragment")]
    InvalidFrament,

    #[error("identifier was not valid UTF-8")]
    NotUtf8,

    #[error("identifier contained a reserved fragment {0:?}")]
    Reserved(&'static str),

    #[error("identifier contained empty or no fragments")]
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Identifier {
    id: EcoString,
}

impl Identifier {
    pub const RESERVED: &'static [&'static str] = &[
        super::REF_NAME,
        super::TEST_NAME,
        super::OUT_NAME,
        super::DIFF_NAME,
    ];

    pub const SEPARATOR: &'static str = "/";

    pub fn new<S: Into<EcoString>>(id: S) -> Result<Self, IdentifierError> {
        let id = id.into();

        for fragment in id.split(Self::SEPARATOR) {
            Self::check_fragment(fragment)?;
        }

        Ok(Self { id })
    }

    pub fn from_path<S: AsRef<Path>>(id: S) -> Result<Self, IdentifierError> {
        fn inner(path: &Path) -> Result<Identifier, IdentifierError> {
            if path.as_os_str().is_empty() {
                return Err(IdentifierError::Empty);
            }

            let mut id = EcoString::new();

            for component in path.components() {
                match component {
                    Component::RootDir => {}
                    Component::Prefix(_) | Component::CurDir | Component::ParentDir => {
                        return Err(IdentifierError::InvalidFrament)
                    }
                    Component::Normal(fragment) => {
                        let fragment = fragment.to_str().ok_or(IdentifierError::NotUtf8)?;
                        Identifier::check_fragment(fragment)?;
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

    fn check_fragment(fragment: &str) -> Result<(), IdentifierError> {
        if fragment.is_empty() {
            return Err(IdentifierError::Empty);
        }

        if let Some(reserved) = Self::RESERVED.iter().find(|&&r| fragment == r) {
            return Err(IdentifierError::Reserved(reserved));
        }

        let mut chars = fragment.chars().peekable();
        if !chars.next().unwrap().is_ascii_alphabetic() {
            return Err(IdentifierError::InvalidFrament);
        }

        if chars.peek().is_some()
            && !chars.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(IdentifierError::InvalidFrament);
        }

        Ok(())
    }

    pub fn as_str(&self) -> &str {
        self.id.as_str()
    }

    pub fn components(&self) -> Components<'_> {
        Components { rest: &self.id }
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

impl FromStr for Identifier {
    type Err = IdentifierError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl Display for Identifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.id.fmt(f)
    }
}

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
