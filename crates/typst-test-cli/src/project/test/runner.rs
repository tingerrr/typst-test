use std::fmt::{Debug, Display};

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
            compare_strategy: Some(compare::Strategy::default()),
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

impl TestRunner<'_, '_, '_> {
    pub fn run(&mut self) -> Result<Result<(), TestFailure>, Error> {
        self.prepare()?;

        self.load_source()?;

        if self.test.is_ephemeral() {
            self.load_reference_source()?;
        }

        if let Err(err) = self.compile()? {
            bail_inner!(err);
        }

        if self.project_runner.compare_strategy.is_some() {
            self.render()?;

            if self.project_runner.save_temporary {
                self.save_document()?;
            }

            if self.test.is_ephemeral() {
                self.render_reference()?;

                if self.project_runner.save_temporary {
                    self.save_reference_document()?;
                }
            } else {
                self.load_reference_document()?;
            }

            if let Err(err) = self.compare()? {
                bail_inner!(err);
            }
        }

        if self.project_runner.update {
            self.update()?;
        }

        self.cleanup()?;
        Ok(Ok(()))
    }

    pub fn prepare(&mut self) -> Result<(), Error> {
        tracing::trace!(test = ?self.test.id(), "clearing temporary directories");

        self.test
            .delete_temporary_directories(self.project_runner.project.resolver())
            .map_err(|err| Error::other(err).at(Stage::Preparation))?;

        self.test
            .create_temporary_directories(self.project_runner.project.resolver())
            .map_err(|err| Error::other(err).at(Stage::Preparation))?;

        Ok(())
    }

    pub fn cleanup(&mut self) -> Result<(), Error> {
        Ok(())
    }

    pub fn load_source(&mut self) -> Result<(), Error> {
        tracing::trace!(test = ?self.test.id(), "loading test source");

        self.source = Some(
            self.test
                .load_source(self.project_runner.project.resolver())
                .map_err(|err| Error::other(err).at(Stage::Loading))?,
        );

        Ok(())
    }

    pub fn load_reference_source(&mut self) -> Result<(), Error> {
        tracing::trace!(test = ?self.test.id(), "loading reference source");

        self.reference_source = Some(
            self.test
                .load_reference_source(self.project_runner.project.resolver())
                .map_err(|err| Error::other(err).at(Stage::Preparation))?,
        );

        Ok(())
    }

    pub fn load_reference_document(&mut self) -> Result<(), Error> {
        tracing::trace!(test = ?self.test.id(), "loading reference document");

        self.reference_store_document = Some(
            self.test
                .load_reference_document(self.project_runner.project.resolver())
                .map_err(|err| Error::other(err).at(Stage::Loading))?,
        );

        Ok(())
    }

    pub fn render(&mut self) -> Result<(), Error> {
        tracing::trace!(test = ?self.test.id(), "rendering test document");

        let document = self
            .document
            .as_ref()
            .ok_or_else(|| Error::missing_stage(Stage::Compilation).at(Stage::Rendering))?;

        let compare::Strategy::Visual(strategy, _) =
            self.project_runner.compare_strategy.unwrap_or_default();
        self.store_document = Some(Document::render(document, strategy));

        Ok(())
    }

    pub fn render_reference(&mut self) -> Result<(), Error> {
        tracing::trace!(test = ?self.test.id(), "rendering reference document");

        let document = self
            .reference_document
            .as_ref()
            .ok_or_else(|| Error::missing_stage(Stage::Compilation).at(Stage::Rendering))?
            .as_ref()
            .ok_or_else(|| {
                Error::incorrect_kind(self.test.ref_kind().copied())
                    .context("Only ephemeral tests can have their reference rendered")
                    .at(Stage::Comparison)
            })?;

        let compare::Strategy::Visual(strategy, _) =
            self.project_runner.compare_strategy.unwrap_or_default();
        self.reference_store_document = Some(Some(Document::render(document, strategy)));

        Ok(())
    }

    pub fn compile_reference(&mut self) -> Result<Result<(), CompileFailure>, Error> {
        self.compile_inner(true)
    }

    pub fn compile(&mut self) -> Result<Result<(), CompileFailure>, Error> {
        self.compile_inner(false)
    }

    fn compile_inner(&mut self, is_reference: bool) -> Result<Result<(), CompileFailure>, Error> {
        tracing::trace!(
            test = ?self.test.id(),
            "compiling {}document",
            if is_reference { "reference " } else { "" },
        );

        let source = if is_reference {
            self.reference_source
                .as_ref()
                .ok_or_else(|| Error::missing_stage(Stage::Loading).at(Stage::Compilation))?
                .as_ref()
                .ok_or_else(|| {
                    Error::incorrect_kind(self.test.ref_kind().copied())
                        .context("Only ephemeral tests can have their reference compiled")
                        .at(Stage::Comparison)
                })?
        } else {
            self.source
                .as_ref()
                .ok_or_else(|| Error::missing_stage(Stage::Loading).at(Stage::Compilation))?
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

    pub fn save_reference_document(&mut self) -> Result<(), Error> {
        tracing::trace!(test = ?self.test.id(), "saving reference document");

        let document = self
            .reference_store_document
            .as_ref()
            .ok_or_else(|| Error::missing_stage(Stage::Compilation).at(Stage::Saving))?
            .as_ref()
            .ok_or_else(|| {
                Error::incorrect_kind(self.test.ref_kind().copied())
                    .context("Only ephemeral tests can save their reference document")
                    .at(Stage::Comparison)
            })?;

        document
            .save(
                self.project_runner
                    .project
                    .resovler
                    .resolve(self.test.id(), TestTarget::RefDir),
            )
            .map_err(|err| Error::other(err).at(Stage::Saving))?;

        Ok(())
    }

    pub fn save_document(&mut self) -> Result<(), Error> {
        tracing::trace!(test = ?self.test.id(), "saving test document");

        let document = self
            .store_document
            .as_ref()
            .ok_or_else(|| Error::missing_stage(Stage::Compilation).at(Stage::Saving))?;

        document
            .save(
                self.project_runner
                    .project
                    .resovler
                    .resolve(self.test.id(), TestTarget::OutDir),
            )
            .map_err(|err| Error::other(err).at(Stage::Saving))?;

        Ok(())
    }

    pub fn compare(&mut self) -> Result<Result<(), CompareFailure>, Error> {
        tracing::trace!(test = ?self.test.id(), "comparing");

        let output = self
            .store_document
            .as_ref()
            .ok_or_else(|| Error::missing_stage(Stage::Loading).at(Stage::Comparison))?
            .clone();

        let reference = self
            .reference_store_document
            .as_ref()
            .ok_or_else(|| Error::missing_stage(Stage::Loading).at(Stage::Comparison))?
            .as_ref()
            .ok_or_else(|| {
                Error::incorrect_kind(self.test.ref_kind().copied())
                    .context("Compile only tests cannot be compared")
                    .at(Stage::Comparison)
            })?;

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

    pub fn update(&self) -> Result<(), Error> {
        tracing::trace!(test = ?self.test.id(), "updating references");

        let document = self
            .store_document
            .as_ref()
            .ok_or_else(|| Error::missing_stage(Stage::Loading).at(Stage::Comparison))?;

        self.test
            .create_reference_document(self.project_runner.project.resolver(), document)
            .map_err(|err| Error::other(err).at(Stage::Update))?;

        Ok(())
    }
}

#[derive(Debug)]
enum ErrorImpl {
    IncorrectKind(Option<ReferenceKind>),
    MissingStage(Stage),
    Other(anyhow::Error),
}

pub struct Error {
    inner: ErrorImpl,
    context: Option<String>,
    stage: Option<Stage>,
}

impl Error {
    fn incorrect_kind(kind: Option<ReferenceKind>) -> Self {
        Self {
            inner: ErrorImpl::IncorrectKind(kind),
            context: None,
            stage: None,
        }
    }

    fn missing_stage(stage: Stage) -> Self {
        Self {
            inner: ErrorImpl::MissingStage(stage),
            context: None,
            stage: None,
        }
    }

    fn other(error: impl Into<anyhow::Error>) -> Self {
        Self {
            inner: ErrorImpl::Other(error.into()),
            context: None,
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
            write!(f, "failed: ")?;
        }

        if let Some(ctx) = &self.context {
            write!(f, "{ctx}")?;
        }

        Ok(())
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}
