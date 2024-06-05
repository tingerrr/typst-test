use ecow::EcoVec;
use tiny_skia::Pixmap;

pub mod on_disk;

#[derive(Debug, Clone)]
pub enum Resource {
    Png(EcoVec<Pixmap>),
}

impl Resource {
    fn format(&self) -> Format {
        match self {
            Resource::Png(_) => Format::Png,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Format {
    // TODO: if we can, allow deserializing the document from disk
    Native,
    Pdf,
    Png,
    Svg,
}
