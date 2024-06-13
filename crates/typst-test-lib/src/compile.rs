use std::time::Duration;

pub mod external;
pub mod in_memory;

#[derive(Debug)]
pub struct Metrics {
    pub duration: Duration,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            duration: Duration::new(0, 0),
        }
    }
}
