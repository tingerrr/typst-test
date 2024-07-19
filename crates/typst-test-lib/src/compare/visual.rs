use tiny_skia::Pixmap;

use super::{Error, PageError, Size};

/// The strategy to use for comparison.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Strategy {
    /// Use a simple pixel channel difference comparison, setting both fields
    /// to `0` makes an exact comparison.
    Simple {
        /// The maximum allowed difference between a channel of two pixels
        /// before they are be considered different.
        max_delta: u8,

        /// The maximum allowed amount of pixels that can differ per page in
        /// accordance to `max_delta` before two pages are considered different.
        max_deviation: usize,
    },
}

impl Default for Strategy {
    fn default() -> Self {
        Self::Simple {
            max_delta: 0,
            max_deviation: 0,
        }
    }
}

/// Compares the rendered outputs of two documents with the given strategy, if
/// `fail_fast` is `true`, then the first page failure aborts further
/// comparisons.
pub fn compare_pages<'p, O, R>(
    outputs: O,
    references: R,
    strategy: Strategy,
    fail_fast: bool,
) -> Result<(), Error>
where
    O: IntoIterator,
    R: IntoIterator,
    O::IntoIter: ExactSizeIterator<Item = &'p Pixmap>,
    R::IntoIter: ExactSizeIterator<Item = &'p Pixmap>,
{
    let outputs = outputs.into_iter();
    let references = references.into_iter();

    let output_len = outputs.len();
    let reference_len = references.len();

    let mut page_errors = if fail_fast {
        vec![]
    } else {
        Vec::with_capacity(outputs.len())
    };

    for (idx, (a, b)) in Iterator::zip(outputs, references).enumerate() {
        if let Err(err) = compare_page(a, b, strategy) {
            page_errors.push((idx, err));

            if fail_fast {
                break;
            }
        }
    }

    if !page_errors.is_empty() || output_len != reference_len {
        page_errors.shrink_to_fit();
        return Err(Error {
            output: output_len,
            reference: reference_len,
            pages: page_errors,
        });
    }

    Ok(())
}

/// Compares two pages individually using the given strategy.
pub fn compare_page(
    output: &Pixmap,
    reference: &Pixmap,
    strategy: Strategy,
) -> Result<(), PageError> {
    let Strategy::Simple {
        max_delta,
        max_deviation,
    } = strategy;

    if output.width() != reference.width() || output.height() != reference.height() {
        return Err(PageError::Dimensions {
            output: Size {
                width: output.width(),
                height: output.height(),
            },
            reference: Size {
                width: reference.width(),
                height: reference.height(),
            },
        });
    }

    let deviations = Iterator::zip(output.pixels().iter(), reference.pixels().iter())
        .filter(|(a, b)| {
            u8::abs_diff(a.red(), b.red()) > max_delta
                || u8::abs_diff(a.green(), b.green()) > max_delta
                || u8::abs_diff(a.blue(), b.blue()) > max_delta
                || u8::abs_diff(a.alpha(), b.alpha()) > max_delta
        })
        .count();

    if deviations > max_deviation {
        return Err(PageError::SimpleDeviations { deviations });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use tiny_skia::PremultipliedColorU8;

    use super::*;

    fn images() -> [Pixmap; 2] {
        let a = Pixmap::new(10, 1).unwrap();
        let mut b = Pixmap::new(10, 1).unwrap();

        let red = PremultipliedColorU8::from_rgba(128, 0, 0, 128).unwrap();
        b.pixels_mut()[0] = red;
        b.pixels_mut()[1] = red;
        b.pixels_mut()[2] = red;
        b.pixels_mut()[3] = red;

        [a, b]
    }

    #[test]
    fn test_compare_page_simple_below_max_delta() {
        let [a, b] = images();
        assert!(compare_page(
            &a,
            &b,
            Strategy::Simple {
                max_delta: 128,
                max_deviation: 0,
            },
        )
        .is_ok())
    }

    #[test]
    fn test_compare_page_simple_below_max_devitation() {
        let [a, b] = images();
        assert!(compare_page(
            &a,
            &b,
            Strategy::Simple {
                max_delta: 0,
                max_deviation: 5,
            },
        )
        .is_ok());
    }

    #[test]
    fn test_compare_page_simple_above_max_devitation() {
        let [a, b] = images();
        assert!(matches!(
            compare_page(
                &a,
                &b,
                Strategy::Simple {
                    max_delta: 0,
                    max_deviation: 0,
                },
            ),
            Err(PageError::SimpleDeviations { deviations: 4 })
        ))
    }
}
