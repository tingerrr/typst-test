use tiny_skia::Pixmap;
use typst::model::Document;
use typst::visualize::Color;

/// Renders a document into a a collection of raster images.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Strategy {
    /// The ammount of pixels to use per pt.
    pub pixel_per_pt: f32,
}

impl Default for Strategy {
    fn default() -> Self {
        Self {
            // NOTE: this doesn't seem to be quite exactly 2, so we use this to
            // ensure we get the same default value as typst-cli
            pixel_per_pt: 144.0 / 72.0,
        }
    }
}

// TODO: add explicit iterators for this

/// Exports a document into the format given by the strategy.
pub fn render_document(
    document: &Document,
    stragety: Strategy,
) -> impl ExactSizeIterator<Item = Pixmap> + '_ {
    document
        .pages
        .iter()
        .map(move |page| typst_render::render(&page.frame, stragety.pixel_per_pt, Color::WHITE))
}
