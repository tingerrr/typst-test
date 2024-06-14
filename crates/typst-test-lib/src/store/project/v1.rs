use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::sync::Mutex;

use super::{Project, TestTarget};
use crate::test::id::Identifier;

/// The name of the persistent reference store directory or ephemeral test script.
const REF_NAME: &str = "ref";

/// The name of the test script.
const TEST_NAME: &str = "test";

/// The name of the ephemeral output directory.
const OUT_NAME: &str = "out";

/// The name of the ephemeral diff directory.
const DIFF_NAME: &str = "diff";

#[derive(Default)]
struct Paths {
    test_dir: Option<NonNull<Path>>,
    test_script: Option<NonNull<Path>>,

    ref_dir: Option<NonNull<Path>>,
    ref_script: Option<NonNull<Path>>,

    out_dir: Option<NonNull<Path>>,

    diff_dir: Option<NonNull<Path>>,
}

pub struct ProjectV1 {
    test_root: PathBuf,
    leaked: Mutex<BTreeMap<Identifier, Paths>>,
}

impl ProjectV1 {
    pub fn new<P: Into<PathBuf>>(test_root: P) -> Self {
        Self {
            test_root: test_root.into(),
            leaked: Mutex::new(BTreeMap::new()),
        }
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
        // - Box::leak contains a non-null ptr
        // - we know the value was created from a valid initalized ptr and can therefore create a ref
        // - we ensure that init doesn't panic below
        unsafe {
            target
                .get_or_insert_with(|| NonNull::new_unchecked(Box::leak(init().into_boxed_path())))
                .as_ref()
        }
    }
}

impl Drop for ProjectV1 {
    fn drop(&mut self) {
        _ = std::mem::take(self.leaked.get_mut().unwrap())
            .into_values()
            .map(|p| {
                // SAFETY: we know these were constructed from leaking and are thus valid
                p.test_dir.map(|p| unsafe { Box::from_raw(p.as_ptr()) });
                p.test_script.map(|p| unsafe { Box::from_raw(p.as_ptr()) });
                p.ref_dir.map(|p| unsafe { Box::from_raw(p.as_ptr()) });
                p.ref_script.map(|p| unsafe { Box::from_raw(p.as_ptr()) });
                p.out_dir.map(|p| unsafe { Box::from_raw(p.as_ptr()) });
                p.diff_dir.map(|p| unsafe { Box::from_raw(p.as_ptr()) });
            });
    }
}

impl Project for ProjectV1 {
    const RESERVED: &'static [&'static str] = &[REF_NAME, TEST_NAME, OUT_NAME, DIFF_NAME];

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
                    path.push(TEST_NAME);
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
