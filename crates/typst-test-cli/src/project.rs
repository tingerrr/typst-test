use std::collections::BTreeMap;
use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::{fs, io};

use rayon::prelude::*;
use tiny_skia::Pixmap;
use typst_project::manifest::Manifest;
use typst_test_lib::config::Config;
use typst_test_lib::store::project::v1::ResolverV1;
use typst_test_lib::store::project::Resolver;
use typst_test_lib::store::test::collector::Collector;
use typst_test_lib::store::test::{References, Test};
use typst_test_lib::store::vcs::{Git, Vcs};
use typst_test_lib::store::Document;
use typst_test_lib::test::id::Identifier;
use typst_test_lib::test::ReferenceKind;
use typst_test_lib::test_set::TestSet;
use typst_test_lib::util;

const DEFAULT_TEST_INPUT: &str = include_str!("../../../assets/default-test/test.typ");
const DEFAULT_TEST_OUTPUT: &[u8] = include_bytes!("../../../assets/default-test/test.png");

pub fn try_open_manifest(root: &Path) -> anyhow::Result<Option<Manifest>> {
    tracing::debug!(?root, "reading manifest");

    if typst_project::is_project_root(root)? {
        let content = std::fs::read_to_string(root.join(typst_project::heuristics::MANIFEST_FILE))?;
        let manifest = Manifest::from_str(&content)?;
        Ok(Some(manifest))
    } else {
        Ok(None)
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ScaffoldOptions: u32 {
        /// Create a default example test.
        const EXAMPLE = 0;
    }
}

#[derive(Debug)]
pub struct Project {
    config: Config,
    manifest: Option<Manifest>,
    resolver: ResolverV1,
    vcs: Option<Git>,
    tests: BTreeMap<Identifier, Test>,
    filtered: BTreeMap<Identifier, Test>,
    template: Option<String>,
}

impl Project {
    pub fn new(root: PathBuf, config: Config, manifest: Option<Manifest>) -> Self {
        let resolver = ResolverV1::new(root, config.tests_root_fallback());
        Self {
            config,
            manifest,
            resolver,
            // TODO: vcs support
            vcs: None,
            tests: BTreeMap::new(),
            filtered: BTreeMap::new(),
            template: None,
        }
    }

    pub fn name(&self) -> &str {
        self.manifest
            .as_ref()
            .map(|m| &m.package.name[..])
            .unwrap_or("<unknown package>")
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn manifest(&self) -> Option<&Manifest> {
        self.manifest.as_ref()
    }

    pub fn matched(&self) -> &BTreeMap<Identifier, Test> {
        &self.tests
    }

    #[allow(dead_code)]
    pub fn matched_mut(&mut self) -> &mut BTreeMap<Identifier, Test> {
        &mut self.tests
    }

    pub fn filtered(&self) -> &BTreeMap<Identifier, Test> {
        &self.filtered
    }

    #[allow(dead_code)]
    pub fn filtered_mut(&mut self) -> &mut BTreeMap<Identifier, Test> {
        &mut self.filtered
    }

    pub fn template_path(&self) -> Option<PathBuf> {
        self.config
            .template
            .as_ref()
            .map(|t| self.resolver.project_root().join(t))
    }

    pub fn template(&self) -> Option<&str> {
        self.template.as_deref()
    }

    pub fn resolver(&self) -> &ResolverV1 {
        &self.resolver
    }

    pub fn vcs(&self) -> Option<&dyn Vcs> {
        self.vcs.as_ref().map(|vcs| vcs as _)
    }

    pub fn root(&self) -> &Path {
        self.resolver.project_root()
    }

    pub fn tests_root(&self) -> &Path {
        self.resolver.test_root()
    }

    #[allow(dead_code)]
    pub fn root_exists(&self) -> io::Result<bool> {
        self.resolver.project_root().try_exists()
    }

    pub fn test_root_exists(&self) -> io::Result<bool> {
        self.resolver.test_root().try_exists()
    }

    #[allow(dead_code)]
    pub fn unique_test(&self) -> Result<&Test, ()> {
        if self.tests.len() != 1 {
            return Err(());
        }

        let (_, test) = self.tests.first_key_value().ok_or(())?;

        Ok(test)
    }

    pub fn is_init(&self) -> io::Result<bool> {
        self.test_root_exists()
    }

    pub fn init(&mut self, options: ScaffoldOptions) -> anyhow::Result<()> {
        let tests_root_dir = self.tests_root();
        let _span = tracing::debug_span!("initalizing project", root = ?tests_root_dir);

        tracing::debug!(path = ?tests_root_dir, "creating tests root");
        util::fs::create_dir(tests_root_dir, false)?;

        if options.contains(ScaffoldOptions::EXAMPLE) {
            tracing::debug!("adding example test");
            self.create_test(
                Identifier::new("example").unwrap(),
                Some(ReferenceKind::Persistent),
                false,
            )?;
            Ok(())
        } else {
            Ok(())
        }
    }

    pub fn uninit(&self) -> anyhow::Result<()> {
        let tests_root_dir = self.tests_root();
        let _span = tracing::debug_span!("initalizing project", root = ?tests_root_dir);

        tracing::trace!(path = ?tests_root_dir, "removing");
        util::fs::remove_dir(tests_root_dir, true)?;
        Ok(())
    }

    pub fn clean_artifacts(&self) -> anyhow::Result<()> {
        let _span = tracing::debug_span!("cleaning temporary directories");

        self.tests
            .par_iter()
            .try_for_each(|(_, test)| test.delete_temporary_directories(&self.resolver))?;

        Ok(())
    }

    pub fn load_template(&mut self) -> anyhow::Result<()> {
        tracing::debug!("loading template");

        if let Some(template) = self.template_path() {
            match fs::read_to_string(template) {
                Ok(template) => self.template = Some(template),
                Err(err) if err.kind() == io::ErrorKind::NotFound => {}
                Err(err) => anyhow::bail!(err),
            }
        }

        Ok(())
    }

    pub fn create_test(
        &mut self,
        id: Identifier,
        kind: Option<ReferenceKind>,
        use_template: bool,
    ) -> anyhow::Result<()> {
        let _span = tracing::debug_span!("creating test", test = ?id, ?kind, ?use_template);

        if self.tests.contains_key(&id) {
            anyhow::bail!("Test '{id}' alreayd exists");
        }

        let source = match (use_template, &self.template) {
            (true, Some(template)) => template,
            (true, None) => {
                tracing::debug!("no template loaded, falling back to default");
                DEFAULT_TEST_INPUT
            }
            (false, _) => DEFAULT_TEST_INPUT,
        };

        let reference = match kind {
            Some(ReferenceKind::Ephemeral) => Some(References::Ephemeral(source.into())),
            Some(ReferenceKind::Persistent) if use_template && self.template.is_some() => {
                let _span = tracing::debug_span!("compiling non default persistent test");
                todo!("compile")
            }
            Some(ReferenceKind::Persistent) => Some(References::Persistent(Document::new(vec![
                Pixmap::decode_png(DEFAULT_TEST_OUTPUT).unwrap(),
            ]))),
            None => None,
        };

        let test = Test::create(&self.resolver, self.vcs(), id, source, reference)?;
        self.tests.insert(test.id().clone(), test);

        Ok(())
    }

    pub fn delete_tests(&mut self) -> anyhow::Result<()> {
        let _span = tracing::debug_span!("deleting tests");

        self.tests
            .par_iter()
            .try_for_each(|(_, test)| test.delete(&self.resolver))?;

        self.tests.clear();
        Ok(())
    }

    pub fn collect_tests<T: TestSet + 'static>(&mut self, test_set: T) -> anyhow::Result<()> {
        let _span = tracing::debug_span!("collecting tests", ?test_set);

        // TODO: error handling
        let mut collector = Collector::new(&self.resolver);
        collector.with_test_set(test_set);
        collector.collect();
        self.tests = collector.take_tests();
        self.filtered = collector.take_filtered();

        Ok(())
    }
}
