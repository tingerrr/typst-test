use std::collections::btree_map::Entry;
use std::collections::BTreeMap;
use std::fmt::{Debug, Display};
use std::fs::DirEntry;
use std::io;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::time::Duration;

use image::{ImageResult, RgbImage};
use oxipng::{Deflaters, InFile, IndexSet, Options, OutFile, PngResult, StripChunks};
use rayon::prelude::*;
use semver::Version;
use tiny_skia::Pixmap;
use typst::eval::Tracer;
use typst::model::Document;
use typst_test_lib::test::stage::compile::Metrics;
use typst_test_lib::test::stage::store::on_disk::{LoadFailure, SaveFailure};
use typst_test_lib::test::stage::{compare, store};
use typst_test_lib::test::Test;

use super::{
    CleanupFailure, CompareFailure, ComparePageFailure, CompileFailure, PersistenceFailure,
    PrepareFailure, Stage, TestFailure, TestResult, UpdateFailure,
};
use crate::project::Project;
use crate::util;

const CHANNEL_DELTA: u8 = 10;
const COMPARISON_THRESHOLD: f32 = 0.05;

// TODO: render stage, saving render output in context, loading stage, saving loaded images in context

fn no_optimize_options() -> Options {
    Options {
        fix_errors: false,
        force: true,
        filter: IndexSet::new(),
        interlace: None,
        optimize_alpha: false,
        bit_depth_reduction: false,
        color_type_reduction: false,
        palette_reduction: false,
        grayscale_reduction: false,
        idat_recoding: false,
        scale_16: false,
        strip: StripChunks::None,
        deflate: Deflaters::Libdeflater { compression: 0 },
        fast_evaluation: true,
        timeout: Some(Duration::new(0, 0)),
    }
}

#[derive(Debug)]
pub struct ProjectContext<'p> {
    project: &'p Project,
    fail_fast: bool,
    action: ActionInner,
}

#[derive(Debug)]
pub enum Action {
    CompareVisual,
    Update,
    None,
}

#[derive(Debug)]
enum ActionInner {
    Compare {
        // TODO: strategies
    },
    Update {
        optimize_options: Box<Options>,
    },
    None,
}

// #[derive(Debug)]
pub struct TestContext<'c, 'p, 't> {
    project_context: &'c ProjectContext<'p>,
    test: &'t Test,
    tracer: Tracer,
    metrics: Metrics,
}

impl<'p> ProjectContext<'p> {
    pub fn new(project: &'p Project) -> Self {
        Self {
            project,
            fail_fast: false,
            action: ActionInner::None,
        }
    }

    pub fn fail_fast(&self) -> bool {
        self.fail_fast
    }

    pub fn with_fail_fast(&mut self, yes: bool) -> &mut Self {
        self.fail_fast = yes;
        self
    }

    pub fn with_action(&mut self, action: Action) -> &mut Self {
        self.action = match action {
            Action::CompareVisual => ActionInner::Compare {},
            Action::Update => ActionInner::Update {
                optimize_options: Box::new(Options::max_compression()),
            },
            Action::None => ActionInner::None,
        };
        self
    }

    pub fn test<'c, 't>(&'c self, test: &'t Test) -> TestContext<'c, 'p, 't> {
        tracing::trace!(test = ?test.name, "establishing test context");
        TestContext {
            project_context: self,
            test,
            tracer: Tracer::new(),
            metrics: Metrics::new(),
        }
    }

    pub fn prepare(&self) -> Result<(), Error> {
        Ok(())
    }

    pub fn cleanup(&self) -> Result<(), Error> {
        Ok(())
    }
}

macro_rules! bail_inner {
    ($err:expr) => {
        let err: TestFailure = $err.into();
        return Ok(Err(err));
    };
}

impl TestContext<'_, '_, '_> {
    fn tmp_dir(&self) -> PathBuf {
        todo!()
    }

    fn ref_dir(&self) -> PathBuf {
        todo!()
    }

    fn out_dir(&self) -> PathBuf {
        todo!()
    }
}

impl TestContext<'_, '_, '_> {
    pub fn run(&self) -> StageResult<TestFailure> {
        if let Err(err) = self.prepare()? {
            bail_inner!(err);
        }

        // TODO: possibly parallelize those two compilations, needs disjunct storage for their output
        if let Err(err) = self.compile_test()? {
            bail_inner!(err);
        }

        if self.test.reference.is_some() {
            if let Err(err) = self.compile_refs()? {
                bail_inner!(err);
            }
        }

        match &self.project_context.action {
            ActionInner::Compare {} => {
                if let Err(err) = self.compare()? {
                    bail_inner!(err);
                }
            }
            ActionInner::Update { optimize_options } => {
                if let Err(err) = self.update()? {
                    bail_inner!(err);
                }
            }
            ActionInner::None => {}
        }

        if let Err(err) = self.cleanup()? {
            bail_inner!(err);
        }

        Ok(Ok(()))
    }

    pub fn prepare(&self) -> StageResult<PrepareFailure> {
        let path = self.tmp_dir();
        tracing::trace!(test = ?self.test.name, ?path, "clearing tmp dir");
        util::fs::create_empty_dir(&path, false).map_err(|e| {
            Error::io(e)
                .at(Stage::Preparation)
                .context(format!("clearing tmp dir: {:?}", path))
        })?;

        Ok(Ok(()))
    }

    pub fn cleanup(&self) -> StageResult<CleanupFailure> {
        Ok(Ok(()))
    }

    pub fn compile_refs(&self) -> StageResult<CompileFailure> {
        let Some(reference) = self.test.reference.clone() else {
            // TODO: this is an outer failure, not an inner one
            return Err(Error::missing_references().at(Stage::Compilation));
        };

        match typst_test_lib::test::stage::compile::in_memory::compile(
            reference,
            self.project_context.project,
            &mut self.tracer,
            &mut self.metrics,
        ) {
            Ok(output) => {
                self.reference_output = Some(output.document);
                Ok(Ok(()))
            }
            Err(errors) => Ok(Err(CompileFailure { errors })),
        }
    }

    pub fn compile_test(&self) -> StageResult<CompileFailure> {
        match typst_test_lib::test::stage::compile::in_memory::compile(
            self.test.source.clone(),
            self.project_context.project,
            &mut self.tracer,
            &mut self.metrics,
        ) {
            Ok(output) => {
                self.test_output = Some(output.document);
                Ok(Ok(()))
            }
            Err(errors) => Ok(Err(CompileFailure { errors })),
        }
    }

    pub fn load_reference_render(&self) -> Result<Vec<Pixmap>, LoadFailure> {
        store::on_disk::load_pages(&self.ref_dir(), store::Format::Png)
    }

    pub fn save_reference_render(&self) -> Result<(), SaveFailure> {
        store::on_disk::save_pages(&self.ref_dir(), store::Resource::Png(todo!()))
    }

    pub fn save_test_render(&mut self) -> Result<(), SaveFailure> {
        store::on_disk::save_pages(&self.out_dir(), store::Resource::Png(todo!()))
    }

    pub fn compare(&self) -> Result<Result<(), compare::Failure>, Error> {
        let Some(output) = self.test_render.map(|r| r.iter()) else {
            return Error::missing_renders("test").at(Stage::Comparison);
        };

        let Some(reference) = self.reference_render.map(|r| r.iter()) else {
            return Error::missing_renders("reference").at(Stage::Comparison);
        };

        if let Err(err) = compare::visual::compare_pages(
            outout,
            reference,
            // TODO: make configurable
            compare::visual::Strategy::Simple {
                min_delta: NonZeroU8::new(1).unwrap(),
                min_deviation: NonZeroUsize::new(1).unwrap(),
            },
            self.project_context.fail_fast,
        ) {
            return Ok(Err(err));
        }

        Ok(Ok(()))
    }

    pub fn diff(&self) -> Result<Result<(), compare::Failure>, Error> {
        let Some(output) = self.test_render.map(|r| r.iter()) else {
            return Error::missing_renders("test").at(Stage::Comparison);
        };

        let Some(reference) = self.reference_render.map(|r| r.iter()) else {
            return Error::missing_renders("reference").at(Stage::Comparison);
        };

        // TODO: make parallel for each page
        let mut diff_image = out_image.clone();

        for (out_px, ref_px) in diff_image.pixels_mut().zip(ref_image.pixels()) {
            out_px.0[0] = u8::abs_diff(out_px.0[0], ref_px.0[0]);
            out_px.0[1] = u8::abs_diff(out_px.0[1], ref_px.0[1]);
            out_px.0[2] = u8::abs_diff(out_px.0[2], ref_px.0[2]);
        }

        let path = self
            .diff_dir
            .join(page_number.to_string())
            .with_extension("png");

        tracing::trace!(?path, "saving diff image");
        diff_image.save(path)?;

        Ok(Ok(()))
    }

    pub fn update(&self) -> StageResult<UpdateFailure> {
        tracing::trace!(path = ?self.out_dir, "collecting new refs from out dir");
        let entries = util::fs::collect_dir_entries(&self.out_dir).map_err(Error::io)?;

        if let Err(err) = entries.par_iter().try_for_each(|entry| {
            let name = entry.file_name();
            self.update_optimize_ref(&entry.path(), &self.ref_dir.join(name))
        }) {
            return Ok(Err(UpdateFailure::Optimize { error: err }));
        }

        Ok(Ok(()))
    }
}

pub type ContextResult<V> = Result<V, Error>;
pub type StageResult<E = TestFailure> = ContextResult<TestResult<E>>;

#[derive(Debug)]
enum ErrorImpl {
    Io(io::Error),
    Image(image::ImageError),
    MissingReferences,
}

pub struct Error {
    inner: ErrorImpl,
    context: Option<String>,
    stage: Option<Stage>,
}

impl Error {
    fn io(error: io::Error) -> Self {
        Self {
            inner: ErrorImpl::Io(error),
            context: None,
            stage: None,
        }
    }

    fn image(error: image::ImageError) -> Self {
        Self {
            inner: ErrorImpl::Image(error),
            context: None,
            stage: None,
        }
    }

    fn missing_references() -> Self {
        Self {
            inner: ErrorImpl::MissingReferences,
            context: None,
            stage: None,
        }
    }

    fn missing_renders(kind: &str) -> Self {
        Self {
            inner: ErrorImpl::MissingRenders,
            context: Some(format!("couldn't find {kind} renders")),
            stage: None,
        }
    }

    fn at(mut self, stage: Stage) -> Self {
        self.stage = Some(stage);
        self
    }

    fn context<S: Into<String>>(mut self, context: S) -> Self {
        self.context = Some(context.into());
        self
    }
}

impl Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.inner, f)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(stage) = &self.stage {
            write!(f, "{} stage failed", stage)?;
        } else {
            write!(f, "failed")?;
        }

        if let Some(ctx) = &self.context {
            write!(f, " while {ctx}")?;
        }

        Ok(())
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match &self.inner {
            ErrorImpl::Io(e) => e,
            ErrorImpl::Image(e) => e,
            ErrorImpl::MissingReferences => return None,
        })
    }
}
