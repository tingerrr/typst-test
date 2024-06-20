use std::collections::BTreeMap;
use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::sync::Mutex;

use super::{Resolver, TestTarget};
use crate::config::Config;
use crate::test::id::Identifier;

/// The name of the persistent reference store directory or ephemeral test script.
const REF_NAME: &str = "ref";

/// The name of the test script.
const TEST_NAME: &str = "test";

/// The name of the temporary output directory.
const OUT_NAME: &str = "out";

/// The name of the temporary diff directory.
const DIFF_NAME: &str = "diff";

// We simply leak paths which we have already created and store pointers to them
// to avoid invalidating references when adding new entries to leaked. When we
// drop leaked we simply recreate them so they can be dropped.
#[derive(Default)]
struct Paths {
    test_dir: Option<NonNull<Path>>,
    test_script: Option<NonNull<Path>>,

    ref_dir: Option<NonNull<Path>>,
    ref_script: Option<NonNull<Path>>,

    out_dir: Option<NonNull<Path>>,

    diff_dir: Option<NonNull<Path>>,
}

impl Debug for Paths {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fn map(maybe: &Option<NonNull<Path>>) -> Option<&dyn Debug> {
            // SAFETY: we ensure these never dangle
            maybe.as_ref().map(|p| p as _)
        }

        f.debug_struct("Paths")
            .field("test_dir", &map(&self.test_dir))
            .field("test_script", &map(&self.test_script))
            .field("ref_dir", &map(&self.ref_dir))
            .field("ref_script", &map(&self.ref_script))
            .field("out_dir", &map(&self.out_dir))
            .field("diff_dir", &map(&self.diff_dir))
            .finish()
    }
}

/// An interner for commonly accessed paths following the current project
/// strucutre.
#[derive(Debug)]
pub struct ResolverV1 {
    root: PathBuf,
    test_root: PathBuf,
    leaked: Mutex<BTreeMap<Identifier, Paths>>,
}

impl ResolverV1 {
    /// Creates a new project with the given root and test root directory, the
    /// test root must be relative to the project root.
    pub fn new<P: Into<PathBuf>, Q: AsRef<Path>>(root: P, test_root: Q) -> Self {
        let root = root.into();
        let test_root = root.join(test_root);

        Self {
            root,
            test_root,
            leaked: Mutex::new(BTreeMap::new()),
        }
    }

    /// Creates a new project with the given root and config.
    pub fn from_config<P: Into<PathBuf>>(root: P, config: Config) -> Self {
        Self::new(root, &config.tests_root)
    }

    fn leak_and_record(
        &self,
        id: &Identifier,
        select: impl FnOnce(&mut Paths) -> &mut Option<NonNull<Path>>,
        init: impl FnOnce() -> PathBuf,
    ) -> &Path {
        let mut guard = self.leaked.lock().unwrap();
        let target = select(guard.entry(id.clone()).or_default());

        // SAFETY:
        // - the result of Box::leak never dangles
        // - we ensure that init doesn't panic below
        unsafe {
            target
                .get_or_insert_with(|| NonNull::new_unchecked(Box::leak(init().into_boxed_path())))
                .as_ref()
        }
    }
}

// SAFETY: access to internerd stoarge is synchronized and not thread local
unsafe impl Send for ResolverV1 {}

// SAFETY: access to internerd stoarge is synchronized and not thread local
unsafe impl Sync for ResolverV1 {}

impl Drop for ResolverV1 {
    fn drop(&mut self) {
        fn map(p: Option<NonNull<Path>>) {
            _ = p.map(|p| {
                // SAFETY: we ensure these never dangle and are constructed only from leaking boxes
                unsafe { Box::from_raw(p.as_ptr()) }
            });
        }

        _ = std::mem::take(self.leaked.get_mut().unwrap())
            .into_values()
            .map(|p| {
                map(p.test_dir);
                map(p.test_script);
                map(p.ref_dir);
                map(p.ref_script);
                map(p.out_dir);
                map(p.diff_dir);
            });
    }
}

impl Resolver for ResolverV1 {
    const RESERVED: &'static [&'static str] = &[REF_NAME, TEST_NAME, OUT_NAME, DIFF_NAME];

    fn project_root(&self) -> &Path {
        &self.root
    }

    fn test_root(&self) -> &Path {
        &self.test_root
    }

    fn resolve(&self, id: &Identifier, target: TestTarget) -> &Path {
        match target {
            TestTarget::TestDir => self.leak_and_record(
                id,
                |p| &mut p.test_dir,
                || {
                    let mut path = self.test_root.clone();
                    path.extend(id.components());
                    path
                },
            ),
            TestTarget::TestScript => self.leak_and_record(
                id,
                |p| &mut p.test_script,
                || {
                    let mut path = self.test_root.clone();
                    path.extend(id.components());
                    path.push(TEST_NAME);
                    path.set_extension("typ");
                    path
                },
            ),
            TestTarget::RefDir => self.leak_and_record(
                id,
                |p| &mut p.ref_dir,
                || {
                    let mut path = self.test_root.clone();
                    path.extend(id.components());
                    path.push(REF_NAME);
                    path
                },
            ),
            TestTarget::RefScript => self.leak_and_record(
                id,
                |p| &mut p.ref_script,
                || {
                    let mut path = self.test_root.clone();
                    path.extend(id.components());
                    path.push(REF_NAME);
                    path.set_extension("typ");
                    path
                },
            ),
            TestTarget::OutDir => self.leak_and_record(
                id,
                |p| &mut p.out_dir,
                || {
                    let mut path = self.test_root.clone();
                    path.extend(id.components());
                    path.push(OUT_NAME);
                    path
                },
            ),
            TestTarget::DiffDir => self.leak_and_record(
                id,
                |p| &mut p.diff_dir,
                || {
                    let mut path = self.test_root.clone();
                    path.extend(id.components());
                    path.push(DIFF_NAME);
                    path
                },
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_targets() {
        let project = ResolverV1::new("root", "tests");

        let test = Identifier::new("fancy/test").unwrap();
        assert_eq!(
            project.resolve(&test, TestTarget::TestDir),
            Path::new("root/tests/fancy/test"),
        );
        assert_eq!(
            project.resolve(&test, TestTarget::TestScript),
            Path::new("root/tests/fancy/test/test.typ"),
        );
        assert_eq!(
            project.resolve(&test, TestTarget::RefDir),
            Path::new("root/tests/fancy/test/ref"),
        );
        assert_eq!(
            project.resolve(&test, TestTarget::RefScript),
            Path::new("root/tests/fancy/test/ref.typ"),
        );
        assert_eq!(
            project.resolve(&test, TestTarget::OutDir),
            Path::new("root/tests/fancy/test/out"),
        );
        assert_eq!(
            project.resolve(&test, TestTarget::DiffDir),
            Path::new("root/tests/fancy/test/diff"),
        );
    }
}
