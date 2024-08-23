//! Helper functions and types for formatting.

use std::cell::RefCell;
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

/// Displays a sequence of elements as comma separated list with a final
/// separator.
///
/// # Examples
/// ```
/// # use typst_test_stdx::fmt::Separators;
/// assert_eq!(
///    Separators::new(", ", " or ").with(["a", "b", "c"]).to_string(),
///    "a, b or c",
/// );
/// assert_eq!(
///    Separators::comma_or().with(["a", "b", "c"]).to_string(),
///    "a, b or c",
/// );
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Separators {
    separator: &'static str,
    terminal_separator: Option<&'static str>,
}

impl Separators {
    /// Creates a new sequence to display.
    ///
    /// # Examples
    /// ```
    /// # use typst_test_stdx::fmt::Separators;
    /// assert_eq!(
    ///     Separators::new("-", None).with(["a", "b", "c"]).to_string(),
    ///     "a-b-c",
    /// );
    /// assert_eq!(
    ///     Separators::new("-", "/").with(["a", "b", "c"]).to_string(),
    ///     "a-b/c",
    /// );
    /// ```
    pub fn new(
        separator: &'static str,
        terminal_separator: impl Into<Option<&'static str>>,
    ) -> Self {
        Self {
            separator,
            terminal_separator: terminal_separator.into(),
        }
    }

    /// Creates a new sequence to display using only `, ` as separtor.
    ///
    /// # Examples
    /// ```
    /// # use typst_test_stdx::fmt::Separators;
    /// assert_eq!(
    ///    Separators::comma().with(["a", "b"]).to_string(),
    ///    "a, b",
    /// );
    /// assert_eq!(
    ///    Separators::comma().with(["a", "b", "c"]).to_string(),
    ///    "a, b, c",
    /// );
    /// ```
    pub fn comma() -> Self {
        Self::new(", ", None)
    }

    /// Creates a new sequence to display using `, ` and ` or ` as the separtors.
    ///
    /// # Examples
    /// ```
    /// # use typst_test_stdx::fmt::Separators;
    /// assert_eq!(
    ///     Separators::comma_or().with(["a", "b"]).to_string(),
    ///     "a or b",
    /// );
    /// assert_eq!(
    ///     Separators::comma_or().with(["a", "b", "c"]).to_string(),
    ///     "a, b or c",
    /// );
    /// ```
    pub fn comma_or() -> Self {
        Self::new(", ", " or ")
    }

    /// Creates a new sequence to display using `, ` and ` and ` as the separtors.
    ///
    /// # Examples
    /// ```
    /// # use typst_test_stdx::fmt::Separators;
    /// assert_eq!(
    ///    Separators::comma_and().with(["a", "b"]).to_string(),
    ///    "a and b",
    /// );
    /// assert_eq!(
    ///    Separators::comma_and().with(["a", "b", "c"]).to_string(),
    ///    "a, b and c",
    /// );
    /// ```
    pub fn comma_and() -> Self {
        Self::new(", ", " and ")
    }

    /// Formats th given items with this sequence's separators.
    ///
    /// # Examples
    /// ```
    /// # use typst_test_stdx::fmt::Separators;
    /// assert_eq!(
    ///    Separators::new(", ", " or ").with(["a", "b", "c"]).to_string(),
    ///    "a, b or c",
    /// );
    /// assert_eq!(
    ///    Separators::comma_or().with(["a", "b", "c"]).to_string(),
    ///    "a, b or c",
    /// );
    /// ```
    pub fn with<S>(self, items: S) -> impl Display
    where
        S: IntoIterator,
        S::IntoIter: ExactSizeIterator,
        S::Item: Display,
    {
        SequenceDisplay {
            seq: self,
            items: RefCell::new(items.into_iter()),
        }
    }
}

struct SequenceDisplay<I> {
    seq: Separators,
    items: RefCell<I>,
}

impl<I> Display for SequenceDisplay<I>
where
    I: ExactSizeIterator,
    I::Item: Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut items = self.items.try_borrow_mut().expect("is not Sync");
        let mut items = items.by_ref().enumerate();
        let len = items.len();

        if let Some((_, item)) = items.next() {
            write!(f, "{item}")?;
        } else {
            return Ok(());
        }

        for (idx, item) in items {
            let sep = if idx == len - 1 {
                self.seq.terminal_separator.unwrap_or(self.seq.separator)
            } else {
                self.seq.separator
            };

            write!(f, "{sep}{item}")?;
        }

        Ok(())
    }
}
