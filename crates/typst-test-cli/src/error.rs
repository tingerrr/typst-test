use std::error::Error;
use std::fmt::Display;
use std::io;

use thiserror::Error;

use crate::ui::Ui;

pub trait Failure: Error + Send + Sync + 'static {
    fn report(&self, ui: &Ui) -> io::Result<()>;
}

#[derive(Debug, Error)]
#[error("one or more tests failed")]
pub struct TestFailure;

#[derive(Debug)]
pub struct OperationFailure(pub Box<dyn Failure>);

impl Error for OperationFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.0.source()
    }
}

impl Display for OperationFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl<F: Failure> From<F> for OperationFailure {
    fn from(value: F) -> Self {
        OperationFailure(Box::new(value) as _)
    }
}
