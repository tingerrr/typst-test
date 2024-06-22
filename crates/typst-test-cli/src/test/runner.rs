use std::fmt::{Debug, Display};
use std::sync::{mpsc, Arc};
use std::time::Instant;

use anyhow::Context;
use typst::eval::Tracer;
use typst::model::Document as TypstDocument;
use typst::syntax::Source;
use typst::World;
use typst_test_lib::compile::Metrics;
use typst_test_lib::store::project::{Resolver, TestTarget};
use typst_test_lib::store::test::Test;
use typst_test_lib::store::Document;
use typst_test_lib::test::ReferenceKind;
use typst_test_lib::{compare, compile, render};

use super::{CompareFailure, CompileFailure, Stage, TestFailure};
use crate::project::Project;

#[derive(Debug, Clone, Default)]
pub struct RunnerConfig {
    /// Whether to stop after the first failure.
    fail_fast: bool,

    /// Whether to save the temporary documents to disk.
    save_temporary: bool,

    /// The strategy to use when rendering documents.
    render_strategy: Option<render::Strategy>,

    /// Whether to update persistent tests.
    update: bool,

    /// The strategy to use when updating persistent tests.
    update_strategy: Option<render::Strategy>,

    /// Whether to compare ephemeral or persistent tests.
    compare: bool,

    /// The strategy to use when comparing docuemnts.
    compare_strategy: Option<compare::Strategy>,
}

impl RunnerConfig {
    pub fn fail_fast(&self) -> bool {
        self.fail_fast
    }

    pub fn save_temporary(&self) -> bool {
        self.save_temporary
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

    pub fn with_fail_fast(&mut self, yes: bool) -> &mut Self {
        self.fail_fast = yes;
        self
    }

    pub fn with_save_temporary(&mut self, yes: bool) -> &mut Self {
        self.save_temporary = yes;
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

    pub fn build<'p>(self, project: &'p Project, world: &'p (dyn World + Sync)) -> Runner<'p> {
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
    world: &'p (dyn World + Sync),
    config: Arc<RunnerConfig>,
}

pub struct TestRunner<'c, 'p, 't> {
    project_runner: &'c Runner<'p>,
    test: &'t Test,
    config: Arc<RunnerConfig>,

    source: Option<Source>,
    document: Option<TypstDocument>,
    store_document: Option<Document>,

    reference_source: Option<Option<Source>>,
    reference_document: Option<Option<TypstDocument>>,
    reference_store_document: Option<Option<Document>>,
}

impl<'p> Runner<'p> {
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
            source: None,
            document: None,
            store_document: None,
            reference_source: None,
            reference_document: None,
            reference_store_document: None,
        }
    }

    pub fn prepare(&self) -> anyhow::Result<()> {
        Ok(())
    }

    pub fn cleanup(&self) -> anyhow::Result<()> {
        Ok(())
    }
}

macro_rules! bail_inner {
    ($err:expr) => {
        let err: TestFailure = $err.into();
        return Ok(Err(err));
    };
}

impl TestRunner<'_, '_, '_> {
    pub fn config(&self) -> &RunnerConfig {
        &self.config
    }

    pub fn config_mut(&mut self) -> &mut RunnerConfig {
        Arc::make_mut(&mut self.config)
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

        res.at(stage)
    }

    pub fn run(
        &mut self,
        progress: mpsc::Sender<Event>,
    ) -> anyhow::Result<Result<(), TestFailure>> {
        let test = self.test.clone();

        let mut inner = || -> anyhow::Result<Result<(), TestFailure>> {
            self.run_stage(&progress, Stage::Preparation, |this| this.prepare())?;

            self.run_stage(&progress, Stage::Loading, |this| this.load_source())?;

            if let Err(err) =
                self.run_stage(&progress, Stage::Compilation, |this| this.compile())?
            {
                bail_inner!(err);
            }

            // both update and compare need the rendered document
            if self.config.update || self.config.compare {
                self.run_stage(&progress, Stage::Rendering, |this| this.render_document())?;

                if self.config.save_temporary {
                    self.run_stage(&progress, Stage::Saving, |this| this.save_document())?;
                }
            }

            if self.config.compare {
                if self.test.is_ephemeral() {
                    self.run_stage(&progress, Stage::Loading, |this| {
                        this.load_reference_source()
                    })?;

                    if let Err(err) = self.run_stage(&progress, Stage::Compilation, |this| {
                        this.compile_reference()
                    })? {
                        bail_inner!(err);
                    }

                    self.run_stage(&progress, Stage::Rendering, |this| {
                        this.render_reference_document()
                    })?;

                    if self.config.save_temporary {
                        self.run_stage(&progress, Stage::Saving, |this| {
                            this.save_reference_document()
                        })?;
                    }
                } else {
                    self.run_stage(&progress, Stage::Loading, |this| {
                        this.load_reference_document()
                    })?;
                }

                if let Err(err) =
                    self.run_stage(&progress, Stage::Comparison, |this| this.compare())?
                {
                    bail_inner!(err);
                }
            }

            if self.config.update {
                self.run_stage(&progress, Stage::Update, |this| this.update())?;
            }

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

    pub fn load_source(&mut self) -> anyhow::Result<()> {
        tracing::trace!(test = ?self.test.id(), "loading test source");

        self.source = Some(
            self.test
                .load_source(self.project_runner.project.resolver())?,
        );

        Ok(())
    }

    pub fn load_reference_source(&mut self) -> anyhow::Result<()> {
        tracing::trace!(test = ?self.test.id(), "loading reference source");

        self.reference_source = Some(
            self.test
                .load_reference_source(self.project_runner.project.resolver())?,
        );

        Ok(())
    }

    pub fn load_reference_document(&mut self) -> anyhow::Result<()> {
        tracing::trace!(test = ?self.test.id(), "loading reference document");

        self.reference_store_document = Some(
            self.test
                .load_reference_document(self.project_runner.project.resolver())?,
        );

        Ok(())
    }

    pub fn render_document(&mut self) -> anyhow::Result<()> {
        tracing::trace!(test = ?self.test.id(), "rendering test document");

        let document = self
            .document
            .as_ref()
            .ok_or_else(|| MissingStage(Stage::Compilation))?;

        let strategy = self
            .project_runner
            .config
            .render_strategy
            .unwrap_or_default();

        self.store_document = Some(Document::render(document, strategy));

        Ok(())
    }

    pub fn render_reference_document(&mut self) -> anyhow::Result<()> {
        tracing::trace!(test = ?self.test.id(), "rendering reference document");

        let document = self
            .reference_document
            .as_ref()
            .ok_or_else(|| MissingStage(Stage::Compilation))?
            .as_ref()
            .ok_or_else(|| IncorrectKind(self.test.ref_kind().copied()))
            .context("Only ephemeral tests can have their reference rendered")?;

        let strategy = self
            .project_runner
            .config
            .render_strategy
            .unwrap_or_default();

        self.reference_store_document = Some(Some(Document::render(document, strategy)));

        Ok(())
    }

    pub fn compile_reference(&mut self) -> anyhow::Result<Result<(), CompileFailure>> {
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
            self.reference_source
                .as_ref()
                .ok_or_else(|| MissingStage(Stage::Loading))?
                .as_ref()
                .ok_or_else(|| IncorrectKind(self.test.ref_kind().copied()))
                .context("Only ephemeral tests can have their reference compiled")?
        } else {
            self.source
                .as_ref()
                .ok_or_else(|| MissingStage(Stage::Loading))?
        };

        match compile::compile(
            source.clone(),
            self.project_runner.world,
            &mut Tracer::new(),
            &mut Metrics::new(),
        ) {
            Ok(doc) => {
                if is_reference {
                    self.reference_document = Some(Some(doc));
                } else {
                    self.document = Some(doc);
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

        let document = self
            .reference_store_document
            .as_ref()
            .ok_or_else(|| MissingStage(Stage::Compilation))?
            .as_ref()
            .ok_or_else(|| IncorrectKind(self.test.ref_kind().copied()))
            .context("Only ephemeral tests can save their reference document")?;

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
            .store_document
            .as_ref()
            .ok_or_else(|| MissingStage(Stage::Compilation))?;

        document.save(
            self.project_runner
                .project
                .resolver()
                .resolve(self.test.id(), TestTarget::OutDir),
        )?;

        Ok(())
    }

    pub fn compare(&mut self) -> anyhow::Result<Result<(), CompareFailure>> {
        tracing::trace!(test = ?self.test.id(), "comparing");

        let output = self
            .store_document
            .as_ref()
            .ok_or_else(|| MissingStage(Stage::Loading))?
            .clone();

        let reference = self
            .reference_store_document
            .as_ref()
            .ok_or_else(|| MissingStage(Stage::Loading))?
            .as_ref()
            .ok_or_else(|| IncorrectKind(self.test.ref_kind().copied()))
            .context("Compile only tests cannot be compared")?;

        let compare::Strategy::Visual(strategy) = self
            .project_runner
            .config
            .compare_strategy
            .unwrap_or_default();

        match compare::visual::compare_pages(
            output.pages(),
            reference.pages(),
            strategy,
            self.project_runner.config.fail_fast,
        ) {
            Ok(_) => Ok(Ok(())),
            Err(error) => Ok(Err(CompareFailure::Visual {
                error,
                diff_dir: None,
            })),
        }
    }

    pub fn update(&self) -> anyhow::Result<()> {
        tracing::trace!(test = ?self.test.id(), "updating references");

        let document = self
            .store_document
            .as_ref()
            .ok_or_else(|| MissingStage(Stage::Loading))?;

        self.test
            .create_reference_document(self.project_runner.project.resolver(), document)?;

        Ok(())
    }
}

trait At<T, E> {
    fn at(self, stage: Stage) -> Result<T, anyhow::Error>;
}

impl<T, E, C: Context<T, E>> At<T, E> for C {
    fn at(self, stage: Stage) -> Result<T, anyhow::Error> {
        self.context(format!("Failed at stage {}", stage))
    }
}

#[derive(Debug, thiserror::Error)]
pub struct IncorrectKind(Option<ReferenceKind>);

impl Display for IncorrectKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Operation invalid for {} tests",
            match self.0 {
                Some(ReferenceKind::Ephemeral) => "ephemeral",
                Some(ReferenceKind::Persistent) => "persistent",
                None => "compile-only",
            }
        )
    }
}

#[derive(Debug, thiserror::Error)]
#[error("Required to run stage {0}, but did not")]
pub struct MissingStage(Stage);
