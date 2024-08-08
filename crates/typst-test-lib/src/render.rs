//! Rendering document, reference and difference images.

use std::cmp::Ordering;

use tiny_skia::{BlendMode, FilterQuality, Pixmap, PixmapPaint, Transform};
use typst::layout::Page;
use typst::model::Document;
use typst::visualize::Color;

/// The origin of a documents page, this is used for comparisons of pages with
/// different dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Origin {
    /// The origin of pages on the top left corner, this is the default and used
    /// in left-to-right read documents.
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

impl Default for Origin {
    fn default() -> Self {
        Self::TopLeft
    }
}

/// Renders a document into a a collection of raster images.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Strategy {
    /// The amount of pixels to use per pt.
    pub pixel_per_pt: f32,

    /// The background fill.
    pub fill: Color,
}

impl Default for Strategy {
    fn default() -> Self {
        Self {
            // NOTE: this doesn't seem to be quite exactly 2, so we use this to
            // ensure we get the same default value as typst-cli, this avoids
            // spurious failures when people migrate between the old and new
            // version
            pixel_per_pt: ppi_to_ppp(144.0),
            fill: Color::WHITE,
        }
    }
}

/// The factor used to convert pixel per pt to pixel per inch.
pub const PPP_TO_PPI_FACTOR: f32 = 72.0;

/// Converts pixel per pt to pixel per inch.
pub fn ppp_to_ppi(pixel_per_pt: f32) -> f32 {
    pixel_per_pt * PPP_TO_PPI_FACTOR
}

/// Converts pixel per inch to pixel per pt.
pub fn ppi_to_ppp(pixel_per_inch: f32) -> f32 {
    pixel_per_inch / PPP_TO_PPI_FACTOR
}

/// Renders a single page with the given strategy.
pub fn render_page(page: &Page, strategy: Strategy) -> Pixmap {
    typst_render::render(&page.frame, strategy.pixel_per_pt, strategy.fill)
}

/// Takes two lengths and returns the origin offsets required to align both lengths.
fn aligned_offset((a, b): (u32, u32), end: bool) -> (i32, i32) {
    match Ord::cmp(&a, &b) {
        Ordering::Less if end => (u32::abs_diff(a, b) as i32, 0),
        Ordering::Greater if end => (0, u32::abs_diff(a, b) as i32),
        _ => (0, 0),
    }
}

/// Render the visual diff of two pages.
pub fn render_page_diff(base: &Pixmap, change: &Pixmap, origin: Origin) -> Pixmap {
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

/// Renders a document into individual pages with the given strategy.
pub fn render_document(document: &Document, strategy: Strategy) -> RenderDocument<'_> {
    RenderDocument {
        iter: document.pages.iter(),
        strategy,
    }
}

/// Render the visual diff of two documents.
pub fn render_document_diff<'docs>(
    base: &'docs Document,
    change: &'docs Document,
    strategy: Strategy,
    origin: Origin,
) -> RenderDocumentDiff<'docs> {
    RenderDocumentDiff {
        base: render_document(base, strategy),
        change: render_document(change, strategy),
        origin,
    }
}

/// An iterator returning rendered pages of a document.
#[derive(Debug)]
pub struct RenderDocument<'doc> {
    iter: std::slice::Iter<'doc, Page>,
    strategy: Strategy,
}

impl Iterator for RenderDocument<'_> {
    type Item = Pixmap;

    fn next(&mut self) -> Option<Self::Item> {
        Some(render_page(self.iter.next()?, self.strategy))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl ExactSizeIterator for RenderDocument<'_> {}

/// An iterator returning the diff of individual pages of two documents.
#[derive(Debug)]
pub struct RenderDocumentDiff<'docs> {
    base: RenderDocument<'docs>,
    change: RenderDocument<'docs>,
    origin: Origin,
}

impl Iterator for RenderDocumentDiff<'_> {
    type Item = Pixmap;

    fn next(&mut self) -> Option<Self::Item> {
        Some(render_page_diff(
            &self.base.next()?,
            &self.change.next()?,
            self.origin,
        ))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (base_min, base_max) = self.base.size_hint();
        let (change_min, change_max) = self.change.size_hint();

        (
            Ord::min(base_min, change_min),
            Option::zip(base_max, change_max).map(|(base, change)| Ord::min(base, change)),
        )
    }
}

impl ExactSizeIterator for RenderDocumentDiff<'_> {}

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

                // NOTE: Despite some of these being invalid according to
                // PremultipliedColorU8::new, this is indeed what is internally
                // created when inverting.
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
            render_page_diff(&base, &change, Origin::TopLeft).data(),
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

        // similar as above, but mirroed across both axes
        let is_in =
            |x, y, pixmap: &Pixmap| (15 - x) <= pixmap.width() && (10 - y) <= pixmap.height();

        for y in 0..10 {
            for x in 0..15 {
                let idx = diff.width().checked_mul(y).unwrap().checked_add(x).unwrap();
                let px = diff.pixels_mut().get_mut(idx as usize).unwrap();

                // NOTE: Despite some of these being invalid according to
                // PremultipliedColorU8::new, this is indeed what is internally
                // created when inverting.
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
            render_page_diff(&base, &change, Origin::BottomRight).data(),
            diff.data()
        );
    }
}
