//! The core library of typst-test.

pub mod compare;
pub mod compile;
pub mod config;
pub mod hook;
pub mod library;
pub mod render;
pub mod store;
pub mod test;
pub mod test_set;

#[doc(hidden)]
pub mod util;

#[cfg(test)]
pub mod _dev;

#[cfg(test)]
mod tests {
    use typst::eval::Tracer;

    use crate::_dev::GlobalTestWorld;
    use crate::store::project::v1::ResolverV1;
    use crate::store::project::Resolver;
    use crate::store::test::collector::Collector;
    use crate::{compare, compile, library, render};

    #[test]
    fn test_e2e() {
        let project = ResolverV1::new("../../", "assets/test-assets/collect");
        let world = GlobalTestWorld::new(
            project.project_root().to_path_buf(),
            library::augmented_default_library(),
        );

        let strategy = render::Strategy::default();

        let mut collector = Collector::new(&project);
        collector.collect();

        for test in collector.tests().values() {
            let source = test.load_source(&project).unwrap();
            let output = compile::compile(source.clone(), &world, &mut Tracer::new()).unwrap();

            if test.is_compile_only() {
                continue;
            }

            let output: Vec<_> = render::render_document(&output, strategy).collect();

            let reference: Vec<_> =
                if let Some(reference) = test.load_reference_source(&project).unwrap() {
                    let reference =
                        compile::compile(reference.clone(), &world, &mut Tracer::new()).unwrap();

                    render::render_document(&reference, strategy).collect()
                } else if let Some(document) = test.load_reference_documents(&project).unwrap() {
                    document.pages().to_owned()
                } else {
                    panic!()
                };

            compare::visual::compare_pages(
                output.iter(),
                reference.iter(),
                compare::visual::Strategy::default(),
                false,
            )
            .unwrap();
        }
    }
}
