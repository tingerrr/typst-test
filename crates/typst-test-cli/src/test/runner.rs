#![allow(dead_code)]

use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};
use std::time::Instant;

use anyhow::Context;
use ecow::EcoVec;
use typst::eval::Tracer;
use typst::model::Document as TypstDocument;
use typst::syntax::Source;
use typst_test_lib::store::project::{Resolver, TestTarget};
use typst_test_lib::store::test::Test;
use typst_test_lib::store::Document;
use typst_test_lib::{compare, compile, hook, render};

use super::{CompareFailure, CompileFailure, Stage, TestFailure};
use crate::project::Project;
use crate::world::SystemWorld;

#[derive(Debug, Clone, Default)]
pub struct RunnerConfig {
    /// Whether to stop after the first failure.
    no_fail_fast: bool,

    /// Whether to save the temporary documents to disk.
    no_save_temporary: bool,

    /// Whether to compile tests.
    compile: bool,

    /// The strategy to use when rendering documents.
    render_strategy: Option<render::Strategy>,

    /// Whether to update persistent tests.
    update: bool,

    /// The strategy to use when updating persistent tests.
    update_strategy: Option<render::Strategy>,

    /// Whether to compare ephemeral or persistent tests.
    compare: bool,

    /// The strategy to use when comparing documents.
    compare_strategy: Option<compare::Strategy>,

    /// The hook to run once before all tests.
    prepare_hook: Option<PathBuf>,

    /// The hook to run once before all tests.
    cleanup_hook: Option<PathBuf>,

    /// The hook to run once before each tests.
    prepare_each_hook: Option<PathBuf>,

    /// The hook to run once before each tests.
    cleanup_each_hook: Option<PathBuf>,
}

impl RunnerConfig {
    pub fn no_fail_fast(&self) -> bool {
        self.no_fail_fast
    }

    pub fn no_save_temporary(&self) -> bool {
        self.no_save_temporary
    }

    pub fn compile(&self) -> bool {
        self.compile
    }

    pub fn render_strategy(&self) -> Option<render::Strategy> {
        self.render_strategy
    }

    pub fn update(&self) -> bool {
        self.update
    }

    pub fn update_strategy(&self) -> Option<render::Strategy> {
        self.update_strategy
    }

    pub fn compare(&self) -> bool {
        self.compare
    }

    pub fn compare_strategy(&self) -> Option<compare::Strategy> {
        self.compare_strategy
    }

    pub fn prepare_hook(&self) -> Option<&Path> {
        self.prepare_hook.as_deref()
    }

    pub fn cleanup_hook(&self) -> Option<&Path> {
        self.cleanup_hook.as_deref()
    }

    pub fn prepare_each_hook(&self) -> Option<&Path> {
        self.prepare_each_hook.as_deref()
    }

    pub fn cleanup_each_hook(&self) -> Option<&Path> {
        self.cleanup_each_hook.as_deref()
    }

    pub fn with_no_fail_fast(&mut self, yes: bool) -> &mut Self {
        self.no_fail_fast = yes;
        self
    }

    pub fn with_no_save_temporary(&mut self, yes: bool) -> &mut Self {
        self.no_save_temporary = yes;
        self
    }

    pub fn with_compile(&mut self, yes: bool) -> &mut Self {
        self.compile = yes;
        self
    }

    pub fn with_render_strategy(&mut self, strategy: Option<render::Strategy>) -> &mut Self {
        self.render_strategy = strategy;
        self
    }

    pub fn with_update(&mut self, yes: bool) -> &mut Self {
        self.update = yes;
        self
    }

    pub fn with_update_strategy(&mut self, strategy: Option<render::Strategy>) -> &mut Self {
        self.update_strategy = strategy;
        self.with_update(true)
    }

    pub fn with_compare(&mut self, yes: bool) -> &mut Self {
        self.compare = yes;
        self
    }

    pub fn with_compare_strategy(&mut self, strategy: Option<compare::Strategy>) -> &mut Self {
        self.compare_strategy = strategy;
        self.with_compare(true)
    }

    pub fn with_prepare_hook(&mut self, value: Option<PathBuf>) -> &mut Self {
        self.prepare_hook = value;
        self
    }

    pub fn with_cleanup_hook(&mut self, value: Option<PathBuf>) -> &mut Self {
        self.cleanup_hook = value;
        self
    }

    pub fn with_prepare_each_hook(&mut self, value: Option<PathBuf>) -> &mut Self {
        self.prepare_each_hook = value;
        self
    }

    pub fn with_cleanup_each_hook(&mut self, value: Option<PathBuf>) -> &mut Self {
        self.cleanup_each_hook = value;
        self
    }

    pub fn build<'p>(self, project: &'p Project, world: &'p SystemWorld) -> Runner<'p> {
        Runner {
            project,
            world,
            config: Arc::new(self),
        }
    }
}

pub struct Event {
    pub test: Test,
    pub instant: Instant,
    pub message: Option<String>,
    pub payload: EventPayload,
}

pub enum EventPayload {
    StartedTest,
    FinishedTest,
    FailedTest(TestFailure),

    StartedStage(Stage),
    FinishedStage(Stage),
    FailedStage(Stage),
}

pub struct Runner<'p> {
    project: &'p Project,
    world: &'p SystemWorld,
    config: Arc<RunnerConfig>,
}

pub struct Cache {
    pub source: Option<Source>,
    pub document: Option<TypstDocument>,
    pub store_document: Option<Document>,
}

impl Cache {
    fn new() -> Self {
        Self {
            source: None,
            document: None,
            store_document: None,
        }
    }
}

impl<'p> Runner<'p> {
    pub fn project(&self) -> &'p Project {
        self.project
    }

    pub fn world(&self) -> &'p SystemWorld {
        self.world
    }

    pub fn config(&self) -> &RunnerConfig {
        &self.config
    }

    pub fn config_mut(&mut self) -> &mut RunnerConfig {
        Arc::make_mut(&mut self.config)
    }

    pub fn test<'c, 't>(&'c self, test: &'t Test) -> TestRunner<'c, 'p, 't> {
        let mut config = Arc::clone(&self.config);

        if test.is_compile_only() {
            Arc::make_mut(&mut config).with_compare(false);
        }

        TestRunner {
            project_runner: self,
            test,
            config,
            test_cache: Cache::new(),
            reference_cache: Cache::new(),
            difference_store_document: None,
            saved_difference_document: false,
        }
    }

    pub fn run_prepare_hook(&self) -> anyhow::Result<()> {
        if let Some(hook) = &self.config.prepare_hook {
            hook::run(hook, None, self.project.resolver())?;
        }

        Ok(())
    }

    pub fn run_cleanup_hook(&self) -> anyhow::Result<()> {
        if let Some(hook) = &self.config.cleanup_hook {
            hook::run(hook, None, self.project.resolver())?;
        }

        Ok(())
    }
}

macro_rules! bail_inner {
    ($err:expr) => {
        let err: TestFailure = $err.into();
        return Ok(Err(err));
    };
}

pub struct TestRunner<'c, 'p, 't> {
    project_runner: &'c Runner<'p>,
    test: &'t Test,
    config: Arc<RunnerConfig>,

    test_cache: Cache,
    reference_cache: Cache,

    difference_store_document: Option<Document>,

    saved_difference_document: bool,
}

impl TestRunner<'_, '_, '_> {
    pub fn config(&self) -> &RunnerConfig {
        &self.config
    }

    pub fn config_mut(&mut self) -> &mut RunnerConfig {
        Arc::make_mut(&mut self.config)
    }

    pub fn test_cache(&self) -> &Cache {
        &self.test_cache
    }

    pub fn reference_cache(&self) -> &Cache {
        &self.reference_cache
    }
}

impl<'t> TestRunner<'_, '_, 't> {
    fn run_stage<R, F: FnOnce(&mut Self) -> anyhow::Result<R>>(
        &mut self,
        progress: &mpsc::Sender<Event>,
        stage: Stage,
        f: F,
    ) -> anyhow::Result<R> {
        let start = Instant::now();

        progress.send(Event {
            test: self.test.clone(),
            instant: start,
            message: None,
            payload: EventPayload::StartedStage(stage),
        })?;

        let res = f(self);
        let end = Instant::now();

        if res.is_ok() {
            progress.send(Event {
                test: self.test.clone(),
                instant: end,
                message: None,
                payload: EventPayload::FinishedStage(stage),
            })?;
        } else {
            progress.send(Event {
                test: self.test.clone(),
                instant: end,
                message: None,
                payload: EventPayload::FailedStage(stage),
            })?;
        }

        res
    }

    pub fn run(
        &mut self,
        progress: mpsc::Sender<Event>,
    ) -> anyhow::Result<Result<(), TestFailure>> {
        let test = self.test.clone();

        let diff_src_needs_save = !self.test.is_compile_only() && !self.config.no_save_temporary;
        let diff_src_needs_render = diff_src_needs_save;

        let ref_doc_needs_save = self.test.is_ephemeral() && !self.config.no_save_temporary;
        let ref_doc_needs_render = self.test.is_ephemeral()
            && (diff_src_needs_render || ref_doc_needs_save || self.config.compare);
        let ref_doc_needs_load =
            self.test.is_persistent() && (diff_src_needs_render || self.config.compare);

        let test_doc_needs_save = !self.config.no_save_temporary;
        let test_doc_needs_render =
            diff_src_needs_render || test_doc_needs_save || self.config.compare;

        let ref_src_needs_compile =
            self.test.is_ephemeral() && (ref_doc_needs_render || self.config.compile);
        let ref_src_needs_load = self.test.is_ephemeral() && ref_src_needs_compile;

        let test_src_needs_compile = test_doc_needs_render || self.config.compile;
        let test_src_needs_load = test_src_needs_compile;

        // TODO: parallelize test and ref steps
        let mut inner = || -> anyhow::Result<Result<(), TestFailure>> {
            self.run_stage(&progress, Stage::Preparation, |this| this.prepare())?;

            self.run_stage(&progress, Stage::Hooks, |this| this.run_prepare_each_hook())?;

            if test_src_needs_load {
                self.run_stage(&progress, Stage::Loading, |this| {
                    this.load_source().context("Loading test source")
                })?;
            }

            if ref_src_needs_load {
                self.run_stage(&progress, Stage::Loading, |this| {
                    this.load_reference_source()
                        .context("Loading reference source")
                })?;
            }

            if test_src_needs_compile {
                if let Err(err) = self.run_stage(&progress, Stage::Compilation, |this| {
                    this.compile().context("Compiling test")
                })? {
                    bail_inner!(err);
                }
            }

            if ref_src_needs_compile {
                if let Err(err) = self.run_stage(&progress, Stage::Compilation, |this| {
                    this.compile_reference().context("Compiling reference")
                })? {
                    bail_inner!(err);
                }
            }

            if test_doc_needs_render {
                self.run_stage(&progress, Stage::Rendering, |this| {
                    this.render_document().context("Rendering test")
                })?;
            }

            if ref_doc_needs_render {
                self.run_stage(&progress, Stage::Rendering, |this| {
                    this.render_reference_document()
                        .context("Rendering reference")
                })?;
            }

            if test_doc_needs_save {
                self.run_stage(&progress, Stage::Saving, |this| {
                    this.save_document().context("Saving test document")
                })?;
            }

            if ref_doc_needs_save {
                self.run_stage(&progress, Stage::Saving, |this| {
                    this.save_reference_document()
                        .context("Saving reference document")
                })?;
            }

            if ref_doc_needs_load {
                self.run_stage(&progress, Stage::Loading, |this| {
                    this.load_reference_document()
                        .context("Loading reference document")
                })?;
            }

            if diff_src_needs_render {
                self.run_stage(&progress, Stage::Rendering, |this| {
                    this.render_difference_document()
                        .context("Rendering difference")
                })?;
            }

            if diff_src_needs_save {
                self.run_stage(&progress, Stage::Saving, |this| {
                    this.save_difference_document()
                        .context("Saving difference document")
                })?;
            }

            if self.config.compare {
                if let Err(err) =
                    self.run_stage(&progress, Stage::Comparison, |this| this.compare())?
                {
                    bail_inner!(err);
                }
            }

            if self.config.update {
                self.run_stage(&progress, Stage::Update, |this| {
                    this.update().context("Updating reference document")
                })?;
            }

            self.run_stage(&progress, Stage::Hooks, |this| this.run_cleanup_each_hook())?;

            self.run_stage(&progress, Stage::Cleanup, |this| this.cleanup())?;

            Ok(Ok(()))
        };

        let start = Instant::now();
        progress.send(Event {
            test,
            instant: start,
            message: None,
            payload: EventPayload::StartedTest,
        })?;

        let res = inner();
        let end = Instant::now();

        match res {
            Ok(Ok(_)) => {
                progress.send(Event {
                    test: self.test.clone(),
                    instant: end,
                    message: None,
                    payload: EventPayload::FinishedTest,
                })?;

                Ok(Ok(()))
            }
            Ok(Err(err)) => {
                progress.send(Event {
                    test: self.test.clone(),
                    instant: end,
                    message: None,
                    payload: EventPayload::FailedTest(err.clone()),
                })?;

                Ok(Err(err))
            }
            Err(err) => { Err(err) }?,
        }
    }

    pub fn prepare(&mut self) -> anyhow::Result<()> {
        tracing::trace!(test = ?self.test.id(), "clearing temporary directories");

        self.test
            .delete_temporary_directories(self.project_runner.project.resolver())?;

        self.test
            .create_temporary_directories(self.project_runner.project.resolver())?;

        Ok(())
    }

    pub fn cleanup(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    pub fn run_prepare_each_hook(&mut self) -> anyhow::Result<()> {
        tracing::trace!(test = ?self.test.id(), "running prepare-each hook");

        if let Some(hook) = &self.config.prepare_each_hook {
            hook::run(hook, None, self.project_runner.project.resolver())?;
        }

        Ok(())
    }

    pub fn run_cleanup_each_hook(&mut self) -> anyhow::Result<()> {
        tracing::trace!(test = ?self.test.id(), "running cleanup-each hook");

        if let Some(hook) = &self.config.cleanup_each_hook {
            hook::run(hook, None, self.project_runner.project.resolver())?;
        }

        Ok(())
    }

    pub fn load_source(&mut self) -> anyhow::Result<()> {
        tracing::trace!(test = ?self.test.id(), "loading test source");

        self.test_cache.source = Some(
            self.test
                .load_source(self.project_runner.project.resolver())?,
        );

        Ok(())
    }

    pub fn load_reference_source(&mut self) -> anyhow::Result<()> {
        tracing::trace!(test = ?self.test.id(), "loading reference source");

        #[cfg(debug_assertions)]
        if !self.test.is_ephemeral() {
            anyhow::bail!("attempted to load reference source for non-ephemeral test");
        }

        self.reference_cache.source = Some(
            self.test
                .load_reference_source(self.project_runner.project.resolver())?
                .with_context(|| {
                    format!("couldn't find reference source for test {}", self.test.id())
                })?,
        );

        Ok(())
    }

    pub fn load_reference_document(&mut self) -> anyhow::Result<()> {
        tracing::trace!(test = ?self.test.id(), "loading reference document");

        #[cfg(debug_assertions)]
        if !self.test.is_persistent() {
            anyhow::bail!("attempted to load reference source for non-persistent test");
        }

        self.reference_cache.store_document = Some(
            self.test
                .load_reference_documents(self.project_runner.project.resolver())?
                .with_context(|| {
                    format!(
                        "couldn't find reference document for test {}",
                        self.test.id()
                    )
                })?,
        );

        Ok(())
    }

    pub fn render_document(&mut self) -> anyhow::Result<()> {
        tracing::trace!(test = ?self.test.id(), "rendering test document");

        let document = self
            .test_cache
            .document
            .as_ref()
            .context("Output document not compiled")?;

        let strategy = self
            .project_runner
            .config
            .render_strategy
            .unwrap_or_default();

        self.test_cache.store_document = Some(Document::render(document, strategy));

        Ok(())
    }

    pub fn render_reference_document(&mut self) -> anyhow::Result<()> {
        tracing::trace!(test = ?self.test.id(), "rendering reference document");

        #[cfg(debug_assertions)]
        if !self.test.is_ephemeral() {
            anyhow::bail!("attempted to render reference document for non-ephemeral test");
        }

        let document = self
            .reference_cache
            .document
            .as_ref()
            .context("Reference document not compiled")?;

        let strategy = self
            .project_runner
            .config
            .render_strategy
            .unwrap_or_default();

        self.reference_cache.store_document = Some(Document::render(document, strategy));

        Ok(())
    }

    pub fn render_difference_document(&mut self) -> anyhow::Result<()> {
        tracing::trace!(test = ?self.test.id(), "rendering difference document");

        #[cfg(debug_assertions)]
        if self.test.is_compile_only() {
            anyhow::bail!("attempted to render difference document for compile-only test");
        }

        let output = self
            .test_cache
            .store_document
            .as_ref()
            .context("Output document not rendered")?;

        let reference = self
            .reference_cache
            .store_document
            .as_ref()
            .context("Reference document not rendered or loaded")?;

        self.difference_store_document = Some(Document::new(
            Iterator::zip(reference.pages().iter(), output.pages())
                .map(|(base, change)| render::render_page_diff(base, change))
                .collect::<EcoVec<_>>(),
        ));

        Ok(())
    }

    pub fn compile_reference(&mut self) -> anyhow::Result<Result<(), CompileFailure>> {
        #[cfg(debug_assertions)]
        if self.test.is_compile_only() {
            anyhow::bail!("attempted to compile reference for compile-only test");
        }

        self.compile_inner(true)
    }

    pub fn compile(&mut self) -> anyhow::Result<Result<(), CompileFailure>> {
        self.compile_inner(false)
    }

    fn compile_inner(&mut self, is_reference: bool) -> anyhow::Result<Result<(), CompileFailure>> {
        tracing::trace!(
            test = ?self.test.id(),
            "compiling {}document",
            if is_reference { "reference " } else { "" },
        );

        let source = if is_reference {
            self.reference_cache
                .source
                .as_ref()
                .context("Reference source not loaded")?
        } else {
            self.test_cache
                .source
                .as_ref()
                .context("Test source not loaded")?
        };

        match compile::compile(
            source.clone(),
            self.project_runner.world,
            &mut Tracer::new(),
        ) {
            Ok(doc) => {
                if is_reference {
                    self.reference_cache.document = Some(doc);
                } else {
                    self.test_cache.document = Some(doc);
                }
            }
            Err(error) => {
                return Ok(Err(CompileFailure {
                    is_ref: is_reference,
                    error,
                }))
            }
        }

        Ok(Ok(()))
    }

    pub fn save_reference_document(&mut self) -> anyhow::Result<()> {
        tracing::trace!(test = ?self.test.id(), "saving reference document");

        #[cfg(debug_assertions)]
        if !self.test.is_ephemeral() {
            anyhow::bail!("attempted to save reference document for non-ephemeral test");
        }

        let document = self
            .reference_cache
            .store_document
            .as_ref()
            .context("Reference document not rendered")?;

        document.save(
            self.project_runner
                .project
                .resolver()
                .resolve(self.test.id(), TestTarget::RefDir),
        )?;

        Ok(())
    }

    pub fn save_document(&mut self) -> anyhow::Result<()> {
        tracing::trace!(test = ?self.test.id(), "saving test document");

        let document = self
            .test_cache
            .store_document
            .as_ref()
            .context("Output document not rendered")?;

        document.save(
            self.project_runner
                .project
                .resolver()
                .resolve(self.test.id(), TestTarget::OutDir),
        )?;

        Ok(())
    }

    pub fn save_difference_document(&mut self) -> anyhow::Result<()> {
        tracing::trace!(test = ?self.test.id(), "saving difference document");

        #[cfg(debug_assertions)]
        if self.test.is_compile_only() {
            anyhow::bail!("attempted to save difference document for compile-only test");
        }

        let document = self
            .difference_store_document
            .as_ref()
            .context("Difference document not rendered")?;

        document.save(
            self.project_runner
                .project
                .resolver()
                .resolve(self.test.id(), TestTarget::DiffDir),
        )?;

        self.saved_difference_document = true;

        Ok(())
    }

    pub fn compare(&mut self) -> anyhow::Result<Result<(), CompareFailure>> {
        tracing::trace!(test = ?self.test.id(), "comparing");

        #[cfg(debug_assertions)]
        if self.test.is_compile_only() {
            anyhow::bail!("attempted to compare compile-only test");
        }

        let output = self
            .test_cache
            .store_document
            .as_ref()
            .context("Output document not rendered")?;

        let reference = self
            .reference_cache
            .store_document
            .as_ref()
            .context("Reference document not rendered")?;

        let compare::Strategy::Visual(strategy) = self
            .project_runner
            .config
            .compare_strategy
            .unwrap_or_default();

        match compare::visual::compare_pages(
            output.pages(),
            reference.pages(),
            strategy,
            !self.project_runner.config.no_fail_fast,
        ) {
            Ok(_) => Ok(Ok(())),
            Err(error) => Ok(Err(CompareFailure::Visual {
                error,
                diff_dir: self.saved_difference_document.then(|| {
                    self.project_runner
                        .project
                        .resolver()
                        .resolve(self.test.id(), TestTarget::DiffDir)
                        .to_path_buf()
                }),
            })),
        }
    }

    pub fn update(&self) -> anyhow::Result<()> {
        tracing::trace!(test = ?self.test.id(), "updating references");

        let document = self
            .test_cache
            .store_document
            .as_ref()
            .context("Output document not rendered")?;

        self.test
            .create_reference_documents(self.project_runner.project.resolver(), document)?;

        Ok(())
    }
}
