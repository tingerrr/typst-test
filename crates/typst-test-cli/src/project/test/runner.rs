use std::fmt::{Debug, Display};

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
use typst_test_lib::{compare, compile};

use super::{CompareFailure, CompileFailure, Stage, TestFailure};
use crate::project::Project;

pub struct Runner<'p> {
    project: &'p Project,
    world: &'p (dyn World + Sync),
    fail_fast: bool,
    save_temporary: bool,
    update: bool,
    compare_strategy: Option<compare::Strategy>,
}

pub struct TestRunner<'c, 'p, 't> {
    project_runner: &'c Runner<'p>,
    test: &'t Test,

    source: Option<Source>,
    document: Option<TypstDocument>,
    store_document: Option<Document>,

    reference_source: Option<Option<Source>>,
    reference_document: Option<Option<TypstDocument>>,
    reference_store_document: Option<Option<Document>>,
}

impl<'p> Runner<'p> {
    pub fn new(project: &'p Project, world: &'p (dyn World + Sync)) -> Self {
        Self {
            project,
            world,
            fail_fast: false,
            save_temporary: true,
            update: false,
            compare_strategy: None,
        }
    }

    pub fn fail_fast(&self) -> bool {
        self.fail_fast
    }

    pub fn with_fail_fast(&mut self, yes: bool) -> &mut Self {
        self.fail_fast = yes;
        self
    }

    pub fn with_save_temporary(&mut self, yes: bool) -> &mut Self {
        self.save_temporary = yes;
        self
    }

    pub fn with_compare(&mut self, strategy: Option<compare::Strategy>) -> &mut Self {
        self.compare_strategy = strategy;
        self
    }

    pub fn with_update(&mut self, yes: bool) -> &mut Self {
        self.update = yes;
        self
    }

    pub fn test<'c, 't>(&'c self, test: &'t Test) -> TestRunner<'c, 'p, 't> {
        TestRunner {
            project_runner: self,
            test,
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
    pub fn run(&mut self) -> anyhow::Result<Result<(), TestFailure>> {
        self.prepare().at(Stage::Preparation)?;

        self.load_source().at(Stage::Loading)?;

        if let Err(err) = self.compile().at(Stage::Compilation)? {
            bail_inner!(err);
        }

        // both update and compare need the rendered document
        if self.project_runner.update || self.project_runner.compare_strategy.is_some() {
            self.render_document().at(Stage::Rendering)?;

            if self.project_runner.save_temporary {
                self.save_document().at(Stage::Saving)?;
            }
        }

        if self.project_runner.compare_strategy.is_some() {
            if self.test.is_ephemeral() {
                self.load_reference_source().at(Stage::Loading)?;

                if let Err(err) = self.compile_reference().at(Stage::Compilation)? {
                    bail_inner!(err);
                }

                self.render_reference_document().at(Stage::Rendering)?;

                if self.project_runner.save_temporary {
                    self.save_reference_document().at(Stage::Saving)?;
                }
            } else {
                self.load_reference_document().at(Stage::Loading)?;
            }

            if let Err(err) = self.compare()? {
                bail_inner!(err);
            }
        }

        if self.project_runner.update {
            self.update().at(Stage::Update)?;
        }

        self.cleanup().at(Stage::Cleanup)?;
        Ok(Ok(()))
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

        let compare::Strategy::Visual(strategy, _) =
            self.project_runner.compare_strategy.unwrap_or_default();
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

        let compare::Strategy::Visual(strategy, _) =
            self.project_runner.compare_strategy.unwrap_or_default();
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
                .resovler
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
                .resovler
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

        let compare::Strategy::Visual(_, strategy) =
            self.project_runner.compare_strategy.unwrap_or_default();

        match compare::visual::compare_pages(
            output.pages(),
            reference.pages(),
            strategy,
            self.project_runner.fail_fast,
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
