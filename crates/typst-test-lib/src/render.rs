use tiny_skia::Pixmap;
use typst::model::Document;
use typst::visualize::Color;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Strategy {
    Raster { pixel_per_pt: f32 },
}

impl Default for Strategy {
    fn default() -> Self {
        Self::Raster { pixel_per_pt: 1.0 }
    }
}

pub fn render_document(
    document: &Document,
    stragety: Strategy,
) -> impl ExactSizeIterator<Item = Pixmap> + '_ {
    let Strategy::Raster { pixel_per_pt } = stragety;

    document
        .pages
        .iter()
        .map(move |page| typst_render::render(&page.frame, pixel_per_pt, Color::WHITE))
}
