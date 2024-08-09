//! Helper functions and types for formatting.

use std::fmt::Display;

/// Types which affect the plurality of a word. Mostly numbers.
pub trait Plural: Copy {
    /// Returns whether a word representing this value is plural.
    fn is_plural(self) -> bool;
}

macro_rules! impl_plural_num {
    ($t:ty, $id:expr) => {
        impl Plural for $t {
            fn is_plural(self) -> bool {
                self != $id
            }
        }
    };
}

impl_plural_num!(u8, 1);
impl_plural_num!(u16, 1);
impl_plural_num!(u32, 1);
impl_plural_num!(u64, 1);
impl_plural_num!(u128, 1);
impl_plural_num!(usize, 1);

impl_plural_num!(i8, 1);
impl_plural_num!(i16, 1);
impl_plural_num!(i32, 1);
impl_plural_num!(i64, 1);
impl_plural_num!(i128, 1);
impl_plural_num!(isize, 1);

impl_plural_num!(f32, 1.0);
impl_plural_num!(f64, 1.0);

/// A struct which formats the given value in either singular (1) or plural
/// (2+).
///
/// # Examples
/// ```
/// # use typst_test_stdx::fmt::Term;
/// assert_eq!(Term::simple("word").with(1).to_string(), "word");
/// assert_eq!(Term::simple("word").with(2).to_string(), "words");
/// assert_eq!(Term::new("index", "indices").with(1).to_string(), "index");
/// assert_eq!(Term::new("index", "indices").with(2).to_string(), "indices");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Term<'a> {
    /// Construct the plurla term by appending an `s`.
    Simple {
        /// The singular term which can be turned into plural by appending an
        /// `s`.
        singular: &'a str,
    },

    /// Explicitly use the give singular and plural term.
    Explicit {
        /// The singular term.
        singular: &'a str,

        /// The plural term.
        plural: &'a str,
    },
}

impl<'a> Term<'a> {
    /// Creates a new simple term whose plural term is created by appending an
    /// `s`.
    pub const fn simple(singular: &'a str) -> Self {
        Self::Simple { singular }
    }

    /// Creates a term from the explicit singular and plural form.
    pub const fn new(singular: &'a str, plural: &'a str) -> Self {
        Self::Explicit { singular, plural }
    }

    /// Formats this term with the given value.
    ///
    /// # Examples
    /// ```
    /// # use typst_test_stdx::fmt::Term;
    /// assert_eq!(Term::simple("word").with(1).to_string(), "word");
    /// assert_eq!(Term::simple("word").with(2).to_string(), "words");
    /// assert_eq!(Term::new("index", "indices").with(1).to_string(), "index");
    /// assert_eq!(Term::new("index", "indices").with(2).to_string(), "indices");
    /// ```
    pub fn with(self, plural: impl Plural) -> impl Display + 'a {
        PluralDisplay {
            terms: self,
            is_plural: plural.is_plural(),
        }
    }
}

struct PluralDisplay<'a> {
    terms: Term<'a>,
    is_plural: bool,
}

impl Display for PluralDisplay<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (&self.terms, self.is_plural) {
            (Term::Simple { singular }, true) => write!(f, "{singular}s"),
            (Term::Explicit { plural, .. }, true) => write!(f, "{plural}"),
            (Term::Simple { singular }, false) => write!(f, "{singular}"),
            (Term::Explicit { singular, .. }, false) => write!(f, "{singular}"),
        }
    }
}
