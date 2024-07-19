use tiny_skia::{BlendMode, FilterQuality, Pixmap, PixmapPaint, PremultipliedColorU8, Transform};
use typst::layout::Page;
use typst::model::Document;
use typst::visualize::Color;

/// Renders a document into a a collection of raster images.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Strategy {
    /// The ammount of pixels to use per pt.
    pub pixel_per_pt: f32,

    /// The background fill.
    pub fill: Color,
}

impl Default for Strategy {
    fn default() -> Self {
        Self {
            // NOTE: this doesn't seem to be quite exactly 2, so we use this to
            // ensure we get the same default value as typst-cli
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

/// Inverts a pixels fully.
pub fn invert_pixel(px: PremultipliedColorU8) -> PremultipliedColorU8 {
    PremultipliedColorU8::from_rgba(
        0u8.wrapping_sub(px.red()),
        0u8.wrapping_sub(px.green()),
        0u8.wrapping_sub(px.blue()),
        0u8.wrapping_sub(px.alpha()),
    )
    .expect("wrapping sub of zero and non zero must not be zero")
}

/// Renders a single page with the given strategy.
pub fn render_page(page: &Page, strategy: Strategy) -> Pixmap {
    typst_render::render(&page.frame, strategy.pixel_per_pt, strategy.fill)
}

// TODO: support rtl by defining which the origin of the images

/// Render the visual diff of two pages.
pub fn render_page_diff(base: &Pixmap, change: &Pixmap) -> Pixmap {
    let mut diff = Pixmap::new(
        Ord::max(base.width(), change.width()),
        Ord::max(base.height(), change.height()),
    )
    .expect("must be larger than zero");

    diff.draw_pixmap(
        0,
        0,
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
        0,
        0,
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
) -> RenderDocumentDiff<'docs> {
    RenderDocumentDiff {
        base: render_document(base, strategy),
        change: render_document(change, strategy),
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
}

impl Iterator for RenderDocumentDiff<'_> {
    type Item = Pixmap;

    fn next(&mut self) -> Option<Self::Item> {
        Some(render_page_diff(&self.base.next()?, &self.change.next()?))
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
    fn test_page_diff() {
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

        assert_eq!(render_page_diff(&base, &change).data(), diff.data());
    }
}
