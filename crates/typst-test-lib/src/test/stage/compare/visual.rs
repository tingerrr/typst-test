use std::num::{NonZeroU8, NonZeroUsize};

use tiny_skia::Pixmap;

use super::{Failure, PageFailure, Size};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Strategy {
    Simple {
        // minimum delta between a channel of two pixels to be considered different
        min_delta: NonZeroU8,
        // min amount of pixels that must differ more than min_delta
        min_deviation: NonZeroUsize,
    },
}

impl Default for Strategy {
    fn default() -> Self {
        Self::Simple {
            min_delta: NonZeroU8::new(1).unwrap(),
            min_deviation: NonZeroUsize::new(1).unwrap(),
        }
    }
}

pub fn compare_pages(
    outputs: impl ExactSizeIterator<Item = Pixmap>,
    references: impl ExactSizeIterator<Item = Pixmap>,
    strategy: Strategy,
    fail_fast: bool,
) -> Result<(), Failure> {
    if outputs.len() != references.len() {
        return Err(Failure::PageCount {
            output: outputs.len(),
            reference: references.len(),
        });
    }

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

    if page_errors.len() != 0 {
        page_errors.shrink_to_fit();
        return Err(Failure::Page { pages: page_errors });
    }

    Ok(())
}

pub fn compare_page(
    output: Pixmap,
    reference: Pixmap,
    strategy: Strategy,
) -> Result<(), PageFailure> {
    let Strategy::Simple {
        min_delta,
        min_deviation,
    } = strategy;

    if output.width() != reference.width() || output.height() != reference.height() {
        return Err(PageFailure::Dimensions {
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
            u8::abs_diff(a.red(), b.red()) >= min_delta.get()
                || u8::abs_diff(a.green(), b.green()) >= min_delta.get()
                || u8::abs_diff(a.blue(), b.blue()) >= min_delta.get()
                || u8::abs_diff(a.alpha(), b.alpha()) >= min_delta.get()
        })
        .count();

    if deviations >= min_deviation.get() {
        return Err(PageFailure::Content { deviations });
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
    fn test_compare_page_simple_below_min_delta() {
        let [a, b] = images();
        assert!(compare_page(
            a,
            b,
            Strategy::Simple {
                min_delta: NonZeroU8::new(129).unwrap(),
                min_deviation: NonZeroUsize::new(1).unwrap(),
            },
        )
        .is_ok())
    }

    #[test]
    fn test_compare_page_simple_below_min_devitation() {
        let [a, b] = images();
        assert!(compare_page(
            a,
            b,
            Strategy::Simple {
                min_delta: NonZeroU8::new(1).unwrap(),
                min_deviation: NonZeroUsize::new(5).unwrap(),
            },
        )
        .is_ok());
    }

    #[test]
    fn test_compare_page_simple_above_min_devitation() {
        let [a, b] = images();
        assert!(matches!(
            compare_page(
                a,
                b,
                Strategy::Simple {
                    min_delta: NonZeroU8::new(1).unwrap(),
                    min_deviation: NonZeroUsize::new(1).unwrap(),
                },
            ),
            Err(PageFailure::Content { deviations: 4 })
        ))
    }
}
