pub mod compare;
pub mod compile;
pub mod config;
pub mod hook;
pub mod library;
pub mod render;
pub mod store;
pub mod test;
pub mod util;

#[cfg(test)]
pub mod _dev;

#[cfg(test)]
#[cfg(test)]
mod tests {
    use store::project::legacy::ProjectLegacy;
    use test::collector::Collector;
    use typst::eval::Tracer;

    use super::*;
    use crate::_dev::GlobalTestWorld;
    use crate::compile::Metrics;
    use crate::store::page::Png;
    use crate::{compare, compile, library, render};

    #[test]
    fn test_e2e() {
        let world = GlobalTestWorld::new(library::augmented_default_library());
        let project = ProjectLegacy::new("../../", "assets/test-assets/collect");

        let strategy = render::Strategy::default();

        let mut collector = Collector::new(&project);
        collector.collect();

        for test in collector.tests().values() {
            let source = test.load_test_source(&project).unwrap();
            let output = compile::compile(
                source.clone(),
                &world,
                &mut Tracer::new(),
                &mut Metrics::new(),
            )
            .unwrap();

            if test.is_compile_only() {
                continue;
            }

            let output = render::render_document(&output, strategy);

            let reference = if let Some(reference) = test.load_ref_source(&project).unwrap() {
                let reference = compile::compile(
                    reference.clone(),
                    &world,
                    &mut Tracer::new(),
                    &mut Metrics::new(),
                )
                .unwrap();

                render::render_document(&reference, strategy).collect()
            } else if let Some(pages) = test.load_ref_pages::<Png, _>(&project).unwrap() {
                pages
            } else {
                panic!()
            };

            compare::visual::compare_pages(
                output,
                reference.into_iter(),
                compare::visual::Strategy::default(),
                false,
            )
            .unwrap();
        }
    }
}
