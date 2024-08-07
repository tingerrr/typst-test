#![allow(dead_code)]

use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use anyhow::Context;
use ecow::EcoVec;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use typst::model::Document as TypstDocument;
use typst::syntax::Source;
use typst_test_lib::store::project::{Resolver, TestTarget};
use typst_test_lib::store::test::Test;
use typst_test_lib::store::Document;
use typst_test_lib::test::ReferenceKind;
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

    /// Whether to edit test's kind.
    edit_kind: Option<Option<ReferenceKind>>,

    /// The strategy to use when updating persistent tests.
    update_strategy: Option<render::Strategy>,

    /// Whether to compare ephemeral or persistent tests.
    compare: bool,

    /// The strategy to use when comparing documents.
    compare_strategy: Option<compare::Strategy>,

    /// The origin at which to render diff images of different dimensions.
    diff_render_origin: Option<render::Origin>,

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

    pub fn edit_kind(&self) -> Option<Option<ReferenceKind>> {
        self.edit_kind
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

    pub fn diff_render_origin(&self) -> Option<render::Origin> {
        self.diff_render_origin
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

    pub fn with_edit_kind(&mut self, value: Option<Option<ReferenceKind>>) -> &mut Self {
        self.edit_kind = value;
        self
    }

    pub fn with_compare(&mut self, yes: bool) -> &mut Self {
        self.compare = yes;
        self
    }

    pub fn with_compare_strategy(&mut self, strategy: Option<compare::Strategy>) -> &mut Self {
        self.compare_strategy = strategy;
        self.with_compare(true)
    }

    pub fn with_diff_render_origin(&mut self, origin: Option<render::Origin>) -> &mut Self {
        self.diff_render_origin = origin;
        self
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

    pub fn build<'p>(
        self,
        progress: Progress,
        project: &'p Project,
        world: &'p SystemWorld,
    ) -> Runner<'p> {
        Runner {
            project,
            progress,
            world,
            config: Arc::new(self),
        }
    }
}

pub struct Progress {
    tx: mpsc::Sender<Event>,
    failed_compilation: AtomicUsize,
    failed_comparison: AtomicUsize,
    failed_otherwise: AtomicUsize,
    passed: AtomicUsize,
    total: usize,
    filtered: usize,
    start: Instant,
    stop: Instant,
}

impl Progress {
    pub fn new(project: &Project) -> (Self, mpsc::Receiver<Event>) {
        let (tx, rx) = mpsc::channel();

        (
            Self {
                tx,
                failed_compilation: AtomicUsize::new(0),
                failed_comparison: AtomicUsize::new(0),
                failed_otherwise: AtomicUsize::new(0),
                passed: AtomicUsize::new(0),
                total: project.matched().len() + project.filtered().len(),
                filtered: project.filtered().len(),
                start: Instant::now(),
                stop: Instant::now(),
            },
            rx,
        )
    }

    pub fn start(&mut self) {
        self.start = Instant::now();
    }

    pub fn stop(&mut self) {
        self.stop = Instant::now();
    }

    pub fn to_summary(&self) -> Summary {
        Summary {
            total: self.total,
            filtered: self.filtered,
            failed_compilation: self.failed_compilation.load(Ordering::SeqCst),
            failed_comparison: self.failed_comparison.load(Ordering::SeqCst),
            failed_otherwise: self.failed_otherwise.load(Ordering::SeqCst),
            passed: self.passed.load(Ordering::SeqCst),
            time: self.stop.duration_since(self.start),
        }
    }
}

pub struct Summary {
    pub total: usize,
    pub filtered: usize,
    pub failed_compilation: usize,
    pub failed_comparison: usize,
    pub failed_otherwise: usize,
    pub passed: usize,
    pub time: Duration,
}

impl Summary {
    pub fn run(&self) -> usize {
        self.total - self.filtered
    }

    pub fn is_ok(&self) -> bool {
        self.passed == self.run()
    }

    pub fn is_total_fail(&self) -> bool {
        self.passed == 0
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
    progress: Progress,
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

    pub fn progress(&self) -> &Progress {
        &self.progress
    }

    pub fn progress_mut(&mut self) -> &mut Progress {
        &mut self.progress
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

    pub fn test<'c>(&'c self, test: Test) -> TestRunner<'c, 'p> {
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

    pub fn run(mut self) -> anyhow::Result<Summary> {
        self.progress_mut().start();
        self.run_prepare_hook()?;

        let res = self.project.matched().par_iter().try_for_each(
            |(_, test)| -> Result<(), Option<anyhow::Error>> {
                match self.test(test.clone()).run() {
                    Ok(Ok(_)) => Ok(()),
                    Ok(Err(_)) => {
                        if self.config().no_fail_fast() {
                            Ok(())
                        } else {
                            Err(None)
                        }
                    }
                    Err(err) => Err(Some(
                        err.context(format!("Fatal error when running test {}", test.id())),
                    )),
                }
            },
        );

        if let Err(Some(err)) = res {
            return Err(err);
        }

        self.run_cleanup_hook()?;
        self.progress_mut().stop();

        Ok(self.progress().to_summary())
    }
}

macro_rules! bail_inner {
    ($err:expr) => {
        let err: TestFailure = $err.into();
        return Ok(Err(err));
    };
}

pub struct TestRunner<'c, 'p> {
    project_runner: &'c Runner<'p>,
    test: Test,
    config: Arc<RunnerConfig>,

    test_cache: Cache,
    reference_cache: Cache,

    difference_store_document: Option<Document>,

    saved_difference_document: bool,
}

impl TestRunner<'_, '_> {
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

impl TestRunner<'_, '_> {
    fn run_stage<R, F: FnOnce(&mut Self) -> anyhow::Result<R>>(
        &mut self,
        progress: &Progress,
        stage: Stage,
        f: F,
    ) -> anyhow::Result<R> {
        let start = Instant::now();

        let _ = progress.tx.send(Event {
            test: self.test.clone(),
            instant: start,
            message: None,
            payload: EventPayload::StartedStage(stage),
        });

        let res = f(self);
        let end = Instant::now();

        if res.is_ok() {
            let _ = progress.tx.send(Event {
                test: self.test.clone(),
                instant: end,
                message: None,
                payload: EventPayload::FinishedStage(stage),
            });
        } else {
            let _ = progress.tx.send(Event {
                test: self.test.clone(),
                instant: end,
                message: None,
                payload: EventPayload::FailedStage(stage),
            });
        }

        res
    }

    pub fn run(&mut self) -> anyhow::Result<Result<(), TestFailure>> {
        let test = self.test.clone();

        let test_kind_needs_edit = self
            .config
            .edit_kind
            .is_some_and(|kind| kind != self.test.ref_kind());

        let diff_src_needs_save =
            !test_kind_needs_edit && !self.test.is_compile_only() && !self.config.no_save_temporary;
        let diff_src_needs_render = !test_kind_needs_edit && diff_src_needs_save;

        let ref_doc_needs_save = self.test.is_ephemeral() && !self.config.no_save_temporary;
        let ref_doc_needs_render = self.test.is_ephemeral()
            && !test_kind_needs_edit
            && (diff_src_needs_render || ref_doc_needs_save || self.config.compare);
        let ref_doc_needs_load = self.test.is_persistent()
            && !test_kind_needs_edit
            && (diff_src_needs_render || self.config.compare);

        let test_doc_needs_save = !self.config.no_save_temporary;
        let test_doc_needs_render = diff_src_needs_render
            || test_doc_needs_save
            || test_kind_needs_edit
            || self.config.compare;

        let ref_src_needs_compile = self.test.is_ephemeral()
            && !test_kind_needs_edit
            && (ref_doc_needs_render || self.config.compile);
        let ref_src_needs_load =
            self.test.is_ephemeral() && !test_kind_needs_edit && ref_src_needs_compile;

        let test_src_needs_compile = test_doc_needs_render || self.config.compile;
        let test_src_needs_load = test_src_needs_compile;

        let progress = &self.project_runner.progress;

        // TODO: parallelize test and ref steps
        let mut inner = || -> anyhow::Result<Result<(), TestFailure>> {
            self.run_stage(progress, Stage::Preparation, |this| this.prepare())?;

            self.run_stage(progress, Stage::Hooks, |this| this.run_prepare_each_hook())?;

            if test_src_needs_load {
                self.run_stage(progress, Stage::Loading, |this| {
                    this.load_source().context("Loading test source")
                })?;
            }

            if ref_src_needs_load {
                self.run_stage(progress, Stage::Loading, |this| {
                    this.load_reference_source()
                        .context("Loading reference source")
                })?;
            }

            if test_src_needs_compile {
                if let Err(err) = self.run_stage(progress, Stage::Compilation, |this| {
                    this.compile().context("Compiling test")
                })? {
                    bail_inner!(err);
                }
            }

            if ref_src_needs_compile {
                if let Err(err) = self.run_stage(progress, Stage::Compilation, |this| {
                    this.compile_reference().context("Compiling reference")
                })? {
                    bail_inner!(err);
                }
            }

            if test_doc_needs_render {
                self.run_stage(progress, Stage::Rendering, |this| {
                    this.render_document().context("Rendering test")
                })?;
            }

            if ref_doc_needs_render {
                self.run_stage(progress, Stage::Rendering, |this| {
                    this.render_reference_document()
                        .context("Rendering reference")
                })?;
            }

            if test_kind_needs_edit {
                self.run_stage(progress, Stage::Update, |this| {
                    this.update_test_kind().context("Updating test kind")
                })?;
            }

            if test_doc_needs_save {
                self.run_stage(progress, Stage::Saving, |this| {
                    this.save_document().context("Saving test document")
                })?;
            }

            if ref_doc_needs_save {
                self.run_stage(progress, Stage::Saving, |this| {
                    this.save_reference_document()
                        .context("Saving reference document")
                })?;
            }

            if ref_doc_needs_load {
                self.run_stage(progress, Stage::Loading, |this| {
                    this.load_reference_document()
                        .context("Loading reference document")
                })?;
            }

            if diff_src_needs_render {
                self.run_stage(progress, Stage::Rendering, |this| {
                    this.render_difference_document()
                        .context("Rendering difference")
                })?;
            }

            if diff_src_needs_save {
                self.run_stage(progress, Stage::Saving, |this| {
                    this.save_difference_document()
                        .context("Saving difference document")
                })?;
            }

            if self.config.compare {
                if let Err(err) =
                    self.run_stage(progress, Stage::Comparison, |this| this.compare())?
                {
                    bail_inner!(err);
                }
            }

            if self.config.update {
                self.run_stage(progress, Stage::Update, |this| {
                    this.update().context("Updating reference document")
                })?;
            }

            self.run_stage(progress, Stage::Hooks, |this| this.run_cleanup_each_hook())?;

            self.run_stage(progress, Stage::Cleanup, |this| this.cleanup())?;

            Ok(Ok(()))
        };

        let start = Instant::now();
        let _ = progress.tx.send(Event {
            test,
            instant: start,
            message: None,
            payload: EventPayload::StartedTest,
        });

        let res = inner();
        let end = Instant::now();

        match res {
            Ok(Ok(_)) => {
                progress.passed.fetch_add(1, Ordering::SeqCst);
                let _ = progress.tx.send(Event {
                    test: self.test.clone(),
                    instant: end,
                    message: None,
                    payload: EventPayload::FinishedTest,
                });

                Ok(Ok(()))
            }
            Ok(Err(err)) => {
                let counter = match err.stage() {
                    Stage::Compilation => &progress.failed_compilation,
                    Stage::Comparison => &progress.failed_comparison,
                    _ => &progress.failed_otherwise,
                };

                counter.fetch_add(1, Ordering::SeqCst);
                let _ = progress.tx.send(Event {
                    test: self.test.clone(),
                    instant: end,
                    message: None,
                    payload: EventPayload::FailedTest(err.clone()),
                });

                Ok(Err(err))
            }
            Err(err) => {
                progress.failed_otherwise.fetch_add(1, Ordering::SeqCst);
                Err(err)
            }?,
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

        let origin = self.config.diff_render_origin.unwrap_or_default();

        self.difference_store_document = Some(Document::new(
            Iterator::zip(reference.pages().iter(), output.pages())
                .map(|(base, change)| render::render_page_diff(base, change, origin))
                .collect::<EcoVec<_>>(),
        ));

        Ok(())
    }

    pub fn update_test_kind(&mut self) -> anyhow::Result<()> {
        tracing::trace!(test = ?self.test.id(), "updating test kind");

        let new_kind = self.config.edit_kind.context("No reference kind given")?;

        let resolver = self.project_runner.project.resolver();
        let vcs = self.project_runner.project.vcs();

        match new_kind {
            Some(ReferenceKind::Ephemeral) => self.test.make_ephemeral(resolver, vcs)?,
            Some(ReferenceKind::Persistent) => {
                let output = self
                    .test_cache
                    .store_document
                    .as_ref()
                    .context("Output document not rendered")?;

                self.test.make_persistent(resolver, vcs, output)?
            }
            None => self.test.make_compile_only(resolver, vcs)?,
        };

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

        // TODO: handle warnings
        match compile::compile(source.clone(), self.project_runner.world).output {
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
