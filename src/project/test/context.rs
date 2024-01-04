use std::collections::HashMap;
use std::fmt::{Debug, Display};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard};

use image::{ImageResult, RgbImage};

use super::{
    CleanupFailure, CompareFailure, ComparePageFailure, CompileFailure, PrepareFailure, Stage,
    Test, TestFailure, TestResult,
};
use crate::project::Project;
use crate::util;

#[derive(Debug)]
pub struct Context {
    project: Project,
    typst: PathBuf,
    fail_fast: bool,
    results: Mutex<HashMap<Test, TestResult>>,
}

#[derive(Debug)]
pub struct TestContext<'ctx> {
    project_context: &'ctx Context,
    test: Test,
    typ_file: PathBuf,
    out_dir: PathBuf,
    ref_dir: PathBuf,
    diff_dir: PathBuf,
}

impl Context {
    pub fn new(project: Project, typst: PathBuf, fail_fast: bool) -> Self {
        Self {
            project,
            typst,
            fail_fast,
            results: Mutex::new(HashMap::new()),
        }
    }

    pub fn test<'ctx>(&'ctx self, test: &Test) -> TestContext<'ctx> {
        let dir = self.project.test_dir();
        let typ_dir = dir.join("typ").join(&test.name);
        let out_dir = dir.join("out").join(&test.name);
        let ref_dir = dir.join("ref").join(&test.name);
        let diff_dir = dir.join("diff").join(&test.name);

        let typ_file = if test.folder {
            typ_dir.join("test")
        } else {
            typ_dir
        }
        .with_extension("typ");

        TestContext {
            project_context: self,
            test: test.clone(),
            typ_file,
            out_dir,
            ref_dir,
            diff_dir,
        }
    }

    pub fn results(&self) -> MutexGuard<'_, HashMap<Test, TestResult>> {
        self.results.lock().unwrap()
    }
}

impl TestContext<'_> {
    #[tracing::instrument(skip(self))]
    pub fn register_result(&self, result: TestResult) {
        let mut results = self.project_context.results.lock().unwrap();
        results.insert(self.test.clone(), result);
    }

    #[tracing::instrument(skip_all)]
    pub fn run(&self, compare: bool) -> ContextResult<TestFailure> {
        macro_rules! bail_if_fail_fast {
            ($err:expr) => {
                let err: TestFailure = $err.into();
                self.register_result(Err(err.clone()));
                if self.project_context.fail_fast {
                    return Ok(Err(err));
                } else {
                    return Ok(Ok(()));
                }
            };
        }

        if let Err(err) = self.prepare()? {
            bail_if_fail_fast!(err);
        }

        if let Err(err) = self.compile()? {
            bail_if_fail_fast!(err);
        }

        if compare {
            if let Err(err) = self.compare()? {
                bail_if_fail_fast!(err);
            }
        }

        if let Err(err) = self.cleanup()? {
            bail_if_fail_fast!(err);
        }

        self.register_result(Ok(()));
        Ok(Ok(()))
    }

    #[tracing::instrument(skip_all)]
    pub fn prepare(&self) -> ContextResult<PrepareFailure> {
        let err_fn = |t, d| format!("creating {}, dir: {:?}", t, d);
        let dirs = [("out", &self.out_dir), ("diff", &self.diff_dir)];

        for (name, path) in dirs {
            util::fs::ensure_empty_dir(path, false)
                .map_err(|e| Error::io(Stage::Preparation, e).context(err_fn(name, path)))?;
        }

        util::fs::ensure_dir(&self.ref_dir, false).map_err(|e| Error::io(Stage::Preparation, e))?;

        Ok(Ok(()))
    }

    #[tracing::instrument(skip_all)]
    pub fn cleanup(&self) -> ContextResult<CleanupFailure> {
        Ok(Ok(()))
    }

    #[tracing::instrument(skip_all)]
    pub fn compile(&self) -> ContextResult<CompileFailure> {
        let mut typst = Command::new(&self.project_context.typst);
        typst.args(["compile", "--root"]);
        typst.arg(self.project_context.project.root());
        typst.arg(&self.typ_file);
        typst.arg(self.out_dir.join("{n}").with_extension("png"));

        let output = typst
            .output()
            .map_err(|e| Error::io(Stage::Compilation, e).context("executing typst"))?;

        if !output.status.success() {
            return Ok(Err(CompileFailure {
                args: typst.get_args().map(ToOwned::to_owned).collect(),
                output,
            }));
        }

        Ok(Ok(()))
    }

    #[tracing::instrument(skip_all)]
    pub fn compare(&self) -> ContextResult<CompareFailure> {
        let mut out_entries = util::fs::collect_dir_entries(&self.out_dir).map_err(|e| {
            Error::io(Stage::Comparison, e)
                .context(format!("reading out directory: {:?}", &self.out_dir))
        })?;

        let mut ref_entries = util::fs::collect_dir_entries(&self.ref_dir).map_err(|e| {
            Error::io(Stage::Comparison, e)
                .context(format!("reading ref directory: {:?}", &self.ref_dir))
        })?;

        out_entries.sort_by_key(|t| t.file_name());
        ref_entries.sort_by_key(|t| t.file_name());

        if out_entries.len() != ref_entries.len() {
            return Ok(Err(CompareFailure::PageCount {
                output: out_entries.len(),
                reference: ref_entries.len(),
            }));
        }

        let mut pages = vec![];

        for (idx, (out_entry, ref_entry)) in out_entries.into_iter().zip(ref_entries).enumerate() {
            if let Err(err) = self.compare_page(idx + 1, &out_entry.path(), &ref_entry.path())? {
                pages.push((idx, err));
                if self.project_context.fail_fast {
                    return Ok(Err(CompareFailure::Page { pages }));
                }
            }
        }

        if !pages.is_empty() {
            return Ok(Err(CompareFailure::Page { pages }));
        }

        Ok(Ok(()))
    }

    #[tracing::instrument(skip_all, fields(page = ?page_number))]
    pub fn compare_page(
        &self,
        page_number: usize,
        out_file: &Path,
        ref_file: &Path,
    ) -> ContextResult<ComparePageFailure> {
        let out_image = image::open(out_file)
            .map_err(|e| {
                Error::image(Stage::Comparison, e).context(format!("reading image: {:?}", out_file))
            })?
            .into_rgb8();

        let ref_image = image::open(ref_file)
            .map_err(|e| {
                Error::image(Stage::Comparison, e).context(format!("reading image: {:?}", ref_file))
            })?
            .into_rgb8();

        if out_image.dimensions() != ref_image.dimensions() {
            return Ok(Err(ComparePageFailure::Dimensions {
                output: out_image.dimensions(),
                reference: ref_image.dimensions(),
            }));
        }

        for (out_px, ref_px) in out_image.pixels().zip(ref_image.pixels()) {
            if out_px != ref_px {
                self.save_diff_page(page_number, &out_image, &ref_image)
                    .map_err(|e| Error::image(Stage::Comparison, e))?;
                return Ok(Err(ComparePageFailure::Content));
            }
        }

        Ok(Ok(()))
    }

    #[tracing::instrument(skip_all, fields(page = ?page_number))]
    pub fn save_diff_page(
        &self,
        page_number: usize,
        out_image: &RgbImage,
        ref_image: &RgbImage,
    ) -> ImageResult<()> {
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

        tracing::debug!(?path, "saving diff image");
        diff_image.save(path)?;

        Ok(())
    }
}

pub type ContextResult<E = TestFailure> = Result<TestResult<E>, Error>;

#[derive(Debug)]
enum ErrorImpl {
    Io(io::Error),
    Image(image::ImageError),
}

pub struct Error {
    inner: ErrorImpl,
    context: Option<String>,
    stage: Stage,
}

impl Error {
    fn io(stage: Stage, error: io::Error) -> Self {
        Self {
            inner: ErrorImpl::Io(error),
            context: None,
            stage,
        }
    }

    fn image(stage: Stage, error: image::ImageError) -> Self {
        Self {
            inner: ErrorImpl::Image(error),
            context: None,
            stage,
        }
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
        write!(f, "{} stage failed", self.stage)?;
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
        })
    }
}
