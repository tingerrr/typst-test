use context::Context;

pub mod context;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Test {
    name: String,
    folder: bool,
    // TODO: comparison
    // TODO: actions done before/after compiling/comparing
}

impl Test {
    pub fn new(name: String, folder: bool) -> Self {
        Self { name, folder }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn folder(&self) -> bool {
        self.folder
    }

    #[tracing::instrument(skip_all)]
    pub fn run(&self, context: &Context) -> anyhow::Result<()> {
        let context = context.test(self);
        context.prepare()?;
        context.compile()?;
        context.compare()?;
        context.cleanup()?;
        Ok(())
    }
}
