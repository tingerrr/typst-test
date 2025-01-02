//! Test results.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use ecow::{eco_vec, EcoVec};
use typst::diag::SourceDiagnostic;
use uuid::Uuid;

use super::{Id, Suite};
use crate::doc::{compare, compile};

/// The result kind of a single test kind.
#[derive(Debug, Clone, Default)]
pub enum Kind {
    /// The test was cancelled or not started in the first place.
    #[default]
    Cancelled,

    /// The test was filtered out by a [`TestSet`].
    ///
    /// [`TestSet`]: crate::test_set::TestSet
    Filtered,

    /// The test failed compilation.
    FailedCompilation {
        /// The inner error.
        error: compile::Error,

        /// Whether this was a compilation failure of the reference.
        reference: bool,
    },

    /// The test passed compilation, but failed comparison.
    FailedComparison(compare::Error),

    /// The test passed compilation, but did not run comparison.
    PassedCompilation,

    /// The test passed compilation and comparison.
    PassedComparison,
}

/// The result of a single test run.
#[derive(Debug, Clone)]
pub struct TestResult {
    kind: Option<Kind>,
    warnings: EcoVec<SourceDiagnostic>,
    timestamp: Instant,
    duration: Duration,
}

impl TestResult {
    /// Create a fresh result for a test.
    pub fn new() -> Self {
        Self {
            kind: None,
            warnings: eco_vec![],
            timestamp: Instant::now(),
            duration: Duration::ZERO,
        }
    }

    /// Create a result for a test for a filtered test.
    pub fn filtered() -> Self {
        Self {
            kind: Some(Kind::Filtered),
            warnings: eco_vec![],
            timestamp: Instant::now(),
            duration: Duration::ZERO,
        }
    }
}

impl TestResult {
    /// The kind of this rest result, if it wasn't cancelled.
    pub fn kind(&self) -> Option<&Kind> {
        self.kind.as_ref()
    }

    /// The warnings of the test emitted by the compiler.
    pub fn warnings(&self) -> &[SourceDiagnostic] {
        &self.warnings
    }

    /// The timestamp at which the suite run started.
    pub fn timestamp(&self) -> Instant {
        self.timestamp
    }

    /// The duration of the test, this is a zero duration if this test wasn't
    /// run.
    pub fn duration(&self) -> Duration {
        self.duration
    }

    /// Whether the test was cancelled or not started.
    pub fn is_cancelled(&self) -> bool {
        self.kind.is_none()
    }

    /// Whether the test failed compilation or comparison.
    pub fn is_filtered(&self) -> bool {
        matches!(&self.kind, Some(Kind::Filtered))
    }

    /// Whether the test passed compilation or comparison.
    pub fn is_pass(&self) -> bool {
        matches!(
            &self.kind,
            Some(Kind::PassedCompilation | Kind::PassedComparison)
        )
    }

    /// Whether the test failed compilation or comparison.
    pub fn is_fail(&self) -> bool {
        matches!(
            &self.kind,
            Some(Kind::FailedCompilation { .. } | Kind::FailedComparison(..)),
        )
    }

    /// The errors emitted by the compiler if compilation failed.
    pub fn errors(&self) -> Option<&[SourceDiagnostic]> {
        match &self.kind {
            Some(Kind::FailedCompilation { error, .. }) => Some(&error.0),
            _ => None,
        }
    }
}

impl TestResult {
    /// Sets the timestamp to [`Instant::now`].
    ///
    /// See [`TestResult::end`].
    pub fn start(&mut self) {
        self.timestamp = Instant::now();
    }

    /// Sets the duration to the time elapsed since [`SuiteResult::start`] was
    /// called.
    pub fn end(&mut self) {
        self.duration = self.timestamp.elapsed();
    }

    /// Sets the kind for this test to a reference compilation failure.
    pub fn set_failed_reference_compilation(&mut self, error: compile::Error) {
        self.kind = Some(Kind::FailedCompilation {
            error,
            reference: true,
        });
    }

    /// Sets the kind for this test to a test compilation failure.
    pub fn set_failed_test_compilation(&mut self, error: compile::Error) {
        self.kind = Some(Kind::FailedCompilation {
            error,
            reference: false,
        });
    }

    /// Sets the kind for this test to a compilation pass.
    pub fn set_passed_compilation(&mut self) {
        self.kind = Some(Kind::PassedCompilation);
    }

    /// Sets the kind for this test to a comparison failure.
    pub fn set_failed_comparison(&mut self, error: compare::Error) {
        self.kind = Some(Kind::FailedComparison(error));
    }

    /// Sets the kind for this test to a test comparison pass.
    pub fn set_passed_comparison(&mut self) {
        self.kind = Some(Kind::PassedComparison);
    }

    /// Sets the warnings for this test.
    pub fn set_warnings<I>(&mut self, warnings: I)
    where
        I: Into<EcoVec<SourceDiagnostic>>,
    {
        self.warnings = warnings.into();
    }
}

impl Default for TestResult {
    fn default() -> Self {
        Self::new()
    }
}

/// The result of a test suite run, this contains results for all tests in a
/// suite, including filtered and not-yet-run tests, as well as cached values
/// for the number of filtered, passed and failed tests.
#[derive(Debug, Clone)]
pub struct SuiteResult {
    id: Uuid,
    total: usize,
    filtered: usize,
    passed: usize,
    failed: usize,
    timestamp: Instant,
    duration: Duration,
    results: BTreeMap<Id, TestResult>,
}

impl SuiteResult {
    /// Create a fresh result for a suite, this will have pre-filled results for
    /// all test set to cancelled, these results can be overridden while running
    /// the suite.
    pub fn new(suite: &Suite) -> Self {
        Self {
            id: Uuid::new_v4(),
            total: suite.len(),
            filtered: suite.filtered().len(),
            passed: 0,
            failed: 0,
            timestamp: Instant::now(),
            duration: Duration::ZERO,
            results: suite
                .matched()
                .keys()
                .map(|id| (id.clone(), TestResult::new()))
                .chain(
                    suite
                        .filtered()
                        .keys()
                        .map(|id| (id.clone(), TestResult::filtered())),
                )
                .collect(),
        }
    }
}

impl SuiteResult {
    /// The unique id of this run.
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// The total number of tests in the suite, including filtered ones.
    pub fn total(&self) -> usize {
        self.total
    }

    /// The number of tests in the suite which were expected to run, i.e. the
    /// number of tests which were _not_ filtered out.
    pub fn expected(&self) -> usize {
        self.total - self.filtered
    }

    /// The number of tests in the suite which were run, regardless of outcome.
    pub fn run(&self) -> usize {
        self.passed + self.failed
    }

    /// The number of tests in the suite which were filtered out.
    pub fn filtered(&self) -> usize {
        self.filtered
    }

    /// The number of tests in the suite which were _not_ run either by
    /// filtering or test run cancellation.
    pub fn skipped(&self) -> usize {
        self.total() - self.run()
    }

    /// The number of tests in the suite which passed.
    pub fn passed(&self) -> usize {
        self.passed
    }

    /// The number of tests in the suite which failed.
    pub fn failed(&self) -> usize {
        self.failed
    }

    /// The timestamp at which the suite run started.
    pub fn timestamp(&self) -> Instant {
        self.timestamp
    }

    /// The duration of the whole suite run.
    pub fn duration(&self) -> Duration {
        self.duration
    }

    /// The individual test results.
    ///
    /// This contains results for all tests in the a suite, not just those added
    /// in [`SuiteResult::set_test_result`].
    pub fn results(&self) -> &BTreeMap<Id, TestResult> {
        &self.results
    }

    /// Whether this suite can be considered a complete pass.
    pub fn is_complete_pass(&self) -> bool {
        self.expected() == self.passed()
    }
}

impl SuiteResult {
    /// Sets the timestamp to [`Instant::now`].
    ///
    /// See [`SuiteResult::end`].
    pub fn start(&mut self) {
        self.timestamp = Instant::now();
    }

    /// Sets the duration to the time elapsed since [`SuiteResult::start`] was
    /// called.
    pub fn end(&mut self) {
        self.duration = self.timestamp.elapsed();
    }

    /// Add a test result.
    ///
    /// - This should only add results for each test once, otherwise the test
    ///   will be counted multiple times.
    /// - The results should also only contain failures or passes, cancellations
    ///   and filtered results are ignored, as these are pre-filled when the
    ///   result is constructed.
    pub fn set_test_result(&mut self, id: Id, result: TestResult) {
        debug_assert!(self.results.contains_key(&id));
        debug_assert!(result.is_pass() || result.is_fail());

        if result.is_pass() {
            self.passed += 1;
        } else {
            self.failed += 1;
        }

        self.results.insert(id, result);
    }
}
