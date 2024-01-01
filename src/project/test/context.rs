use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Context as _;

use super::Test;
use crate::project::Project;
use crate::util;

#[derive(Debug, Clone)]
pub struct Context {
    project: Project,
    typst: PathBuf,
}

#[derive(Debug, Clone)]
pub struct TestContext<'a> {
    project_context: &'a Context,
    typ_file: PathBuf,
    out_dir: PathBuf,
    ref_dir: PathBuf,
    diff_dir: PathBuf,
}

impl Context {
    pub fn new(project: Project, typst: PathBuf) -> Self {
        Self { project, typst }
    }

    #[tracing::instrument(skip_all, fields(name = test.name))]
    pub fn test(&self, test: &Test) -> TestContext<'_> {
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
            typ_file,
            out_dir,
            ref_dir,
            diff_dir,
        }
    }
}

impl TestContext<'_> {
    #[tracing::instrument(skip_all)]
    pub fn prepare(&self) -> anyhow::Result<()> {
        fs::create_dir_all(&self.out_dir)
            .with_context(|| format!("creating out dir: {:?}", self.out_dir))?;
        fs::create_dir_all(&self.ref_dir)
            .with_context(|| format!("creating ref dir: {:?}", &self.ref_dir))?;
        fs::create_dir_all(&self.diff_dir)
            .with_context(|| format!("creating diff dir: {:?}", self.diff_dir))?;
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    pub fn cleanup(&self) -> anyhow::Result<()> {
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    pub fn compile(&self) -> anyhow::Result<()> {
        let mut typst = Command::new(&self.project_context.typst);
        typst.args(["compile", "--root"]);
        typst.arg(self.project_context.project.root());
        typst.arg(&self.typ_file);
        typst.arg(self.out_dir.join("{n}").with_extension("png"));

        let res = typst.output()?;
        if !res.status.success() {
            todo!("compile");
        }

        Ok(())
    }

    #[tracing::instrument(skip_all)]
    pub fn compare(&self) -> anyhow::Result<()> {
        let mut out_entries = util::fs::collect_dir_entries(&self.out_dir)
            .with_context(|| format!("reading out directory: {:?}", &self.out_dir))?;

        let mut ref_entries = util::fs::collect_dir_entries(&self.ref_dir)
            .with_context(|| format!("reading ref directory: {:?}", &self.ref_dir))?;

        out_entries.sort_by_key(|t| t.file_name());
        ref_entries.sort_by_key(|t| t.file_name());

        if out_entries.len() != ref_entries.len() {
            todo!("lengths");
        }

        for (out_entry, ref_entry) in out_entries.into_iter().zip(ref_entries) {
            self.compare_page(&out_entry.path(), &ref_entry.path())?;
        }

        Ok(())
    }

    #[tracing::instrument(skip_all)]
    pub fn compare_page(&self, out_file: &Path, ref_file: &Path) -> anyhow::Result<()> {
        let out_data = fs::read(out_file)?;
        let ref_data = fs::read(ref_file)?;

        if out_data != ref_data {
            todo!("data");
        }

        Ok(())
    }
}
