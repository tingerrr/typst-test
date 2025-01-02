//! Document pixel buffer rendering and diffing.

use std::cmp::Ordering;

use tiny_skia::{BlendMode, FilterQuality, Pixmap, PixmapPaint, Transform};

/// The origin of a documents page, this is used for comparisons of pages with
/// different dimensions.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Origin {
    /// The origin of pages on the top left corner, this is the default and used
    /// in left-to-right read documents.
    #[default]
    TopLeft,

    /// The origin of pages on the top right corner, tis is usd in left-to-right
    /// read documents.
    TopRight,

    /// The origin of pages on the botoom left corner, this is included for
    /// completeness.
    BottomLeft,

    /// The origin of pages on the botoom right corner, this is included for
    /// completeness.
    BottomRight,
}

impl Origin {
    /// Whether this origin is at the left.
    pub fn is_left(&self) -> bool {
        matches!(self, Self::TopLeft | Self::BottomLeft)
    }

    /// Whether this origin is at the right.
    pub fn is_right(&self) -> bool {
        matches!(self, Self::TopRight | Self::BottomRight)
    }

    /// Whether this origin is at the top.
    pub fn is_top(&self) -> bool {
        matches!(self, Self::TopLeft | Self::TopRight)
    }

    /// Whether this origin is at the bottom.
    pub fn is_bottom(&self) -> bool {
        matches!(self, Self::BottomLeft | Self::BottomRight)
    }
}

/// The factor used to convert pixel per pt to pixel per inch.
pub const PPP_TO_PPI_FACTOR: f32 = 72.0;

// NOTE(tinger): this doesn't seem to be quite exactly 2, so we use this to
// ensure we get the same default value as typst-cli, this avoids spurious
// failures when people migrate between the old and new version

/// The default pixel per pt value used for rendering pages to pixel buffers.
pub const DEFAULT_PIXEL_PER_PT: f32 = 144.0 / PPP_TO_PPI_FACTOR;

/// Converts a pixel-per-pt ratio to a pixel-per-inch ratio.
pub fn ppp_to_ppi(pixel_per_pt: f32) -> f32 {
    pixel_per_pt * PPP_TO_PPI_FACTOR
}

/// Converts a pixel-per-inch ratio to a pixel-per-pt ratio.
pub fn ppi_to_ppp(pixel_per_inch: f32) -> f32 {
    pixel_per_inch / PPP_TO_PPI_FACTOR
}

/// Render the visual diff of two pages. If the pages do not have matching
/// dimensions, then the origin is used to align them, regions without overlap
/// will simply be colored black.
///
/// The difference is created by `change` on top of `base` using a difference
/// filter.
pub fn page_diff(base: &Pixmap, change: &Pixmap, origin: Origin) -> Pixmap {
    fn aligned_offset((a, b): (u32, u32), end: bool) -> (i32, i32) {
        match Ord::cmp(&a, &b) {
            Ordering::Less if end => (u32::abs_diff(a, b) as i32, 0),
            Ordering::Greater if end => (0, u32::abs_diff(a, b) as i32),
            _ => (0, 0),
        }
    }

    let mut diff = Pixmap::new(
        Ord::max(base.width(), change.width()),
        Ord::max(base.height(), change.height()),
    )
    .expect("must be larger than zero");

    let (base_x, change_x) = aligned_offset((base.width(), change.width()), origin.is_right());
    let (base_y, change_y) = aligned_offset((base.height(), change.height()), origin.is_right());

    diff.draw_pixmap(
        base_x,
        base_y,
        base.as_ref(),
        &PixmapPaint {
            opacity: 1.0,
            blend_mode: BlendMode::Source,
            quality: FilterQuality::Nearest,
        },
        Transform::identity(),
        None,
    );

    diff.draw_pixmap(
        change_x,
        change_y,
        change.as_ref(),
        &PixmapPaint {
            opacity: 1.0,
            blend_mode: BlendMode::Difference,
            quality: FilterQuality::Nearest,
        },
        Transform::identity(),
        None,
    );

    diff
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_diff_top_left() {
        let mut base = Pixmap::new(10, 10).unwrap();
        let mut change = Pixmap::new(15, 5).unwrap();
        let mut diff = Pixmap::new(15, 10).unwrap();

        base.fill(tiny_skia::Color::from_rgba8(255, 255, 255, 255));
        change.fill(tiny_skia::Color::from_rgba8(255, 0, 0, 255));

        let is_in = |x, y, pixmap: &Pixmap| x < pixmap.width() && y < pixmap.height();

        for y in 0..10 {
            for x in 0..15 {
                let idx = diff.width().checked_mul(y).unwrap().checked_add(x).unwrap();
                let px = diff.pixels_mut().get_mut(idx as usize).unwrap();

                // NOTE(tinger): Despite some of these being invalid according
                // to PremultipliedColorU8::new, this is indeed what is
                // internally created when inverting.
                //
                // That's not surprising, but not allowing us to create those
                // pixels when they're valid is.
                *px = bytemuck::cast(match (is_in(x, y, &base), is_in(x, y, &change)) {
                    // proper difference where both are in bounds
                    (true, true) => [0u8, 255, 255, 255],
                    // no difference to base where change is out of bounds
                    (true, false) => [255, 255, 255, 255],
                    // no difference to change where base is out of bounds
                    (false, true) => [255, 0, 0, 255],
                    // dead area from size mismatch
                    (false, false) => [0, 0, 0, 0],
                });
            }
        }

        assert_eq!(
            page_diff(&base, &change, Origin::TopLeft).data(),
            diff.data()
        );
    }

    #[test]
    fn test_page_diff_bottom_right() {
        let mut base = Pixmap::new(10, 10).unwrap();
        let mut change = Pixmap::new(15, 5).unwrap();
        let mut diff = Pixmap::new(15, 10).unwrap();

        base.fill(tiny_skia::Color::from_rgba8(255, 255, 255, 255));
        change.fill(tiny_skia::Color::from_rgba8(255, 0, 0, 255));

        // similar as above, but mirrored across both axes
        let is_in =
            |x, y, pixmap: &Pixmap| (15 - x) <= pixmap.width() && (10 - y) <= pixmap.height();

        for y in 0..10 {
            for x in 0..15 {
                let idx = diff.width().checked_mul(y).unwrap().checked_add(x).unwrap();
                let px = diff.pixels_mut().get_mut(idx as usize).unwrap();

                // NOTE(tinger): Despite some of these being invalid according
                // to PremultipliedColorU8::new, this is indeed what is
                // internally created when inverting.
                //
                // That's not surprising, but not allowing us to create those
                // pixels when they're valid is.
                *px = bytemuck::cast(match (is_in(x, y, &base), is_in(x, y, &change)) {
                    // proper difference where both are in bounds
                    (true, true) => [0u8, 255, 255, 255],
                    // no difference to base where change is out of bounds
                    (true, false) => [255, 255, 255, 255],
                    // no difference to change where base is out of bounds
                    (false, true) => [255, 0, 0, 255],
                    // dead area from size mismatch
                    (false, false) => [0, 0, 0, 0],
                });
            }
        }

        assert_eq!(
            page_diff(&base, &change, Origin::BottomRight).data(),
            diff.data()
        );
    }
}
