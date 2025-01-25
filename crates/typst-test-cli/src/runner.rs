use std::fmt::Debug;
use std::sync::atomic::{AtomicBool, Ordering};

use color_eyre::eyre::{self, ContextCompat};
use lib::doc::compare::Strategy;
use lib::doc::render::Origin;
use lib::doc::{compare, compile, Document};
use lib::project::Project;
use lib::test::{Kind, Suite, SuiteResult, Test, TestResult, TestResultKind};
use typst::diag::{Severity, Warned};
use typst::model::Document as TypstDocument;
use typst::syntax::Source;

use crate::cli::TestFailure;
use crate::report::Reporter;
use crate::world::SystemWorld;
use crate::DEFAULT_OPTIMIZE_OPTIONS;

#[derive(Debug, Clone)]
pub enum Action {
    /// Compile and optionally compare tests.
    Run {
        /// The strategy to use when comparing documents.
        strategy: Option<Strategy>,

        /// Whether to export temporaries.
        export: bool,

        /// The origin at which to render diff images of different dimensions.
        origin: Origin,
    },

    /// Compile and update test references.
    Update {
        /// Whether to export temporaries.
        export: bool,

        /// The origin at which to render diff images of different dimensions.
        origin: Origin,
    },
}

#[derive(Debug, Clone)]
pub struct RunnerConfig<'c> {
    /// Whether to promote warnings to errors.
    pub promote_warnings: bool,

    /// Whether to optimize reference documents.
    pub optimize: bool,

    /// Whether to stop after the first failure.
    pub fail_fast: bool,

    /// The pixel-per-pt to use when rendering documents.
    pub pixel_per_pt: f32,

    /// The action to take for the test.
    pub action: Action,

    /// A cancellation flag used to abort a test run.
    pub cancellation: &'c AtomicBool,
}

pub struct Runner<'c, 'p> {
    pub project: &'p Project,
    pub suite: &'p Suite,
    pub world: &'p SystemWorld,

    pub result: SuiteResult,
    pub config: RunnerConfig<'c>,
}

impl<'c, 'p> Runner<'c, 'p> {
    pub fn new(
        project: &'p Project,
        suite: &'p Suite,
        world: &'p SystemWorld,
        config: RunnerConfig<'c>,
    ) -> Self {
        Self {
            project,
            result: SuiteResult::new(suite),
            suite,
            world,
            config,
        }
    }

    pub fn test<'s>(&'s self, test: &'p Test) -> TestRunner<'c, 's, 'p> {
        TestRunner {
            project_runner: self,
            test,
            result: TestResult::new(),
        }
    }

    pub fn run_inner(&mut self, reporter: &Reporter) -> eyre::Result<()> {
        reporter.report_status(&self.result)?;

        for (id, test) in self.suite.matched() {
            if self.config.cancellation.load(Ordering::SeqCst) {
                return Ok(());
            }

            let result = self.test(test).run()?;

            reporter.clear_status()?;
            match result.kind() {
                Some(
                    TestResultKind::FailedCompilation { .. } | TestResultKind::FailedComparison(..),
                ) => {
                    // TODO(tinger): retrieve export var from action
                    reporter.report_test_fail(test, &result, true)?;
                }
                Some(TestResultKind::PassedCompilation | TestResultKind::PassedComparison) => {
                    reporter.report_test_pass(test, result.duration(), result.warnings())?;
                }
                _ => unreachable!(),
            }
            reporter.report_status(&self.result)?;

            self.result.set_test_result(id.clone(), result);
        }

        reporter.clear_status()?;

        Ok(())
    }

    pub fn run(mut self, reporter: &Reporter) -> eyre::Result<SuiteResult> {
        self.result.start();
        reporter.report_start(&self.result)?;
        let res = self.run_inner(reporter);
        self.result.end();
        reporter.report_end(&self.result)?;

        res?;

        Ok(self.result)
    }
}

pub struct TestRunner<'c, 's, 'p> {
    project_runner: &'s Runner<'c, 'p>,
    test: &'p Test,
    result: TestResult,
}

impl TestRunner<'_, '_, '_> {
    fn run_inner(&mut self) -> eyre::Result<()> {
        // TODO(tinger): don't exit early if there are still exports possible

        let paths = self.project_runner.project.paths();
        let vcs = self.project_runner.project.vcs();

        match self.project_runner.config.action {
            Action::Run {
                strategy,
                export,
                origin,
            } => {
                let output = self.load_out_src()?;
                let output = self.compile_out_doc(output)?;
                let output = self.render_out_doc(output)?;

                if export {
                    self.export_out_doc(&output)?;
                }

                match self.test.kind() {
                    Kind::Ephemeral => {
                        let reference = self.load_ref_src()?;
                        let reference = self.compile_ref_doc(reference)?;
                        let reference = self.render_ref_doc(reference)?;

                        if export {
                            self.export_ref_doc(&reference)?;

                            let diff = self.render_diff_doc(&output, &reference, origin)?;
                            self.export_diff_doc(&diff)?;
                        }

                        if let Some(strategy) = strategy {
                            if let Err(err) = self.compare(&output, &reference, strategy) {
                                eyre::bail!(err);
                            }
                        }
                    }
                    Kind::Persistent => {
                        let reference = self.load_ref_doc()?;

                        // TODO(tinger): don't unconditionally export this
                        // perhaps? on the other hand without comparison we
                        // don't know whether this is meaningful or not
                        if export {
                            let diff = self.render_diff_doc(&output, &reference, origin)?;
                            self.export_diff_doc(&diff)?;
                        }

                        if let Some(strategy) = strategy {
                            if let Err(err) = self.compare(&output, &reference, strategy) {
                                eyre::bail!(err);
                            }
                        }
                    }
                    Kind::CompileOnly => {}
                }
            }
            Action::Update { export, origin } => match self.test.kind() {
                Kind::Ephemeral => {
                    let output = self.load_out_src()?;
                    let output = self.compile_out_doc(output)?;
                    let output = self.render_out_doc(output)?;

                    if export {
                        self.export_out_doc(&output)?;
                    }
                }
                Kind::Persistent => {
                    let output = self.load_out_src()?;
                    let output = self.compile_out_doc(output)?;
                    let output = self.render_out_doc(output)?;

                    self.test.create_reference_documents(
                        paths,
                        vcs,
                        &output,
                        self.project_runner
                            .config
                            .optimize
                            .then_some(&*DEFAULT_OPTIMIZE_OPTIONS),
                    )?;

                    if export {
                        let reference = self.load_ref_doc()?;
                        self.export_out_doc(&reference)?;

                        let diff = self.render_diff_doc(&output, &reference, origin)?;
                        self.export_diff_doc(&diff)?;
                    }
                }
                Kind::CompileOnly => eyre::bail!("attempted to update compile-only test"),
            },
        }

        Ok(())
    }

    pub fn run(mut self) -> eyre::Result<TestResult> {
        self.result.start();
        self.prepare()?;
        let res = self.run_inner();
        self.cleanup()?;
        self.result.end();

        if let Err(err) = res {
            if !err.chain().any(|s| s.is::<TestFailure>()) {
                eyre::bail!(err);
            }
        }

        Ok(self.result)
    }

    pub fn prepare(&mut self) -> eyre::Result<()> {
        tracing::trace!(test = ?self.test.id(), "clearing temporary directories");

        self.test.create_temporary_directories(
            self.project_runner.project.paths(),
            self.project_runner.project.vcs(),
        )?;

        Ok(())
    }

    pub fn cleanup(&mut self) -> eyre::Result<()> {
        Ok(())
    }

    pub fn load_out_src(&mut self) -> eyre::Result<Source> {
        tracing::trace!(test = ?self.test.id(), "loading output source");
        Ok(self.test.load_source(self.project_runner.project.paths())?)
    }

    pub fn load_ref_src(&mut self) -> eyre::Result<Source> {
        tracing::trace!(test = ?self.test.id(), "loading reference source");

        if !self.test.kind().is_ephemeral() {
            eyre::bail!("attempted to load reference source for non-ephemeral test");
        }

        self.test
            .load_reference_source(self.project_runner.project.paths())?
            .wrap_err_with(|| format!("couldn't load reference source for test {}", self.test.id()))
    }

    pub fn load_ref_doc(&mut self) -> eyre::Result<Document> {
        tracing::trace!(test = ?self.test.id(), "loading reference document");

        if !self.test.kind().is_persistent() {
            eyre::bail!("attempted to load reference source for non-persistent test");
        }

        self.test
            .load_reference_documents(self.project_runner.project.paths())?
            .wrap_err_with(|| {
                format!(
                    "couldn't load reference document for test {}",
                    self.test.id()
                )
            })
    }

    pub fn render_out_doc(&mut self, doc: TypstDocument) -> eyre::Result<Document> {
        tracing::trace!(test = ?self.test.id(), "rendering output document");

        Ok(Document::render(
            doc,
            self.project_runner.config.pixel_per_pt,
        ))
    }

    pub fn render_ref_doc(&mut self, doc: TypstDocument) -> eyre::Result<Document> {
        tracing::trace!(test = ?self.test.id(), "rendering reference document");

        if !self.test.kind().is_ephemeral() {
            eyre::bail!("attempted to render reference for non-ephemeral test");
        }

        Ok(Document::render(
            doc,
            self.project_runner.config.pixel_per_pt,
        ))
    }

    pub fn render_diff_doc(
        &mut self,
        output: &Document,
        reference: &Document,
        origin: Origin,
    ) -> eyre::Result<Document> {
        tracing::trace!(test = ?self.test.id(), "rendering difference document");

        if self.test.kind().is_compile_only() {
            eyre::bail!("attempted to render difference document for compile-only test");
        }

        Ok(Document::render_diff(reference, output, origin))
    }

    pub fn compile_out_doc(&mut self, output: Source) -> eyre::Result<TypstDocument> {
        tracing::trace!(test = ?self.test.id(), "compiling output document");

        self.compile_inner(output)
    }

    pub fn compile_ref_doc(&mut self, reference: Source) -> eyre::Result<TypstDocument> {
        tracing::trace!(test = ?self.test.id(), "compiling reference document");

        if self.test.kind().is_compile_only() {
            eyre::bail!("attempted to compile reference for compile-only test");
        }

        self.compile_inner(reference)
    }

    fn compile_inner(&mut self, source: Source) -> eyre::Result<TypstDocument> {
        let Warned {
            output,
            mut warnings,
        } = compile::compile(source, self.project_runner.world);

        if self.project_runner.config.promote_warnings {
            warnings = warnings
                .into_iter()
                .map(|mut warning| {
                    warning.severity = Severity::Error;
                    warning.with_hint("this warning was promoted to an error")
                })
                .collect();
        }

        let doc = match output {
            Ok(doc) => {
                self.result.set_passed_compilation();
                if self.project_runner.config.promote_warnings {
                    self.result
                        .set_failed_reference_compilation(compile::Error(warnings));
                    eyre::bail!(TestFailure);
                } else {
                    self.result.set_warnings(warnings);
                }
                doc
            }
            Err(mut err) => {
                if self.project_runner.config.promote_warnings {
                    err.0.extend(warnings);
                } else {
                    self.result.set_warnings(warnings);
                }
                self.result.set_failed_reference_compilation(err);
                eyre::bail!(TestFailure);
            }
        };

        Ok(doc)
    }

    pub fn export_ref_doc(&mut self, reference: &Document) -> eyre::Result<()> {
        tracing::trace!(test = ?self.test.id(), "saving reference document");

        if !self.test.kind().is_ephemeral() {
            eyre::bail!("attempted to save reference document for non-ephemeral test");
        }

        reference.save(
            self.project_runner
                .project
                .paths()
                .test_ref_dir(self.test.id()),
            None,
        )?;

        Ok(())
    }

    pub fn export_out_doc(&mut self, output: &Document) -> eyre::Result<()> {
        tracing::trace!(test = ?self.test.id(), "saving output document");

        output.save(
            self.project_runner
                .project
                .paths()
                .test_out_dir(self.test.id()),
            None,
        )?;

        Ok(())
    }

    pub fn export_diff_doc(&mut self, doc: &Document) -> eyre::Result<()> {
        tracing::trace!(test = ?self.test.id(), "saving difference document");

        if self.test.kind().is_compile_only() {
            eyre::bail!("attempted to save difference document for compile-only test");
        }

        doc.save(
            self.project_runner
                .project
                .paths()
                .test_diff_dir(self.test.id()),
            None,
        )?;

        Ok(())
    }

    pub fn compare(
        &mut self,
        output: &Document,
        reference: &Document,
        strategy: Strategy,
    ) -> eyre::Result<()> {
        tracing::trace!(test = ?self.test.id(), "comparing");

        if self.test.kind().is_compile_only() {
            eyre::bail!("attempted to compare compile-only test");
        }

        let mut pages =
            Vec::with_capacity(Ord::min(output.buffers().len(), reference.buffers().len()));

        for (idx, (output, reference)) in
            output.buffers().iter().zip(reference.buffers()).enumerate()
        {
            match compare::page(output, reference, strategy) {
                Ok(_) => {}
                Err(err) if self.project_runner.config.fail_fast => {
                    pages.push((idx, err));
                    break;
                }
                Err(err) => pages.push((idx, err)),
            }
        }

        if !pages.is_empty() || output.buffers().len() != reference.buffers().len() {
            self.result.set_failed_comparison(compare::Error {
                output: output.buffers().len(),
                reference: reference.buffers().len(),
                pages,
            });

            eyre::bail!(TestFailure);
        }

        self.result.set_passed_comparison();

        Ok(())
    }
}
