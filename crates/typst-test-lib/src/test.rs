use ecow::EcoString;
use typst::syntax::Source;

pub mod stage;

#[derive(Debug, Clone)]
pub struct Test {
    pub name: EcoString,
    pub source: Source,
    pub reference: Option<Source>,
}

#[cfg(test)]
mod tests {
    use stage::compile::Metrics;
    use stage::{compare, compile, render};
    use typst::eval::Tracer;
    use typst::syntax::{FileId, Source, VirtualPath};

    use super::*;

    #[test]
    fn test_full_ephemeral_pass() {
        let src_path = "../../assets/test-assets/test/ephemeral-src.typ";
        let ref_path = "../../assets/test-assets/test/ephemeral-ref.typ";

        let test = Test {
            name: "main".into(),
            source: Source::new(
                FileId::new(None, VirtualPath::new(src_path)),
                std::fs::read_to_string(src_path).unwrap(),
            ),
            reference: Some(Source::new(
                FileId::new(None, VirtualPath::new(ref_path)),
                std::fs::read_to_string(ref_path).unwrap(),
            )),
        };

        let world = crate::_dev::GlobalTestWorld::default();

        let output = compile::in_memory::compile(
            test.source.clone(),
            &world,
            &mut Tracer::new(),
            &mut Metrics::new(),
        )
        .unwrap();
        let reference = compile::in_memory::compile(
            test.reference.unwrap().clone(),
            &world,
            &mut Tracer::new(),
            &mut Metrics::new(),
        )
        .unwrap();

        let output = render::render_document(&output, render::Strategy::default());
        let reference = render::render_document(&reference, render::Strategy::default());

        compare::visual::compare_pages(
            output,
            reference,
            compare::visual::Strategy::default(),
            false,
        )
        .unwrap();
    }
}
