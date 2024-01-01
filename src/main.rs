use std::{fs, thread};

use clap::Parser;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::prelude::*;
use tracing_tree::HierarchicalLayer;

use self::project::test::context::Context;
use self::project::Project;

mod cli;
mod project;
mod util;

fn main() -> anyhow::Result<()> {
    let args = cli::Args::parse();

    tracing_subscriber::registry()
        .with(HierarchicalLayer::new(4).with_targets(true))
        .with(Targets::new().with_target(std::env!("CARGO_CRATE_NAME"), LevelFilter::DEBUG))
        .init();

    let root = if let Some(root) = args.root {
        let root = fs::canonicalize(root)?;
        anyhow::ensure!(
            project::is_dir_project_root(&root)?,
            "--root must contain a typst.toml manifest file",
        );
        root
    } else {
        let pwd = std::env::current_dir()?;
        if let Some(root) = project::try_find_project_root(&pwd)? {
            root
        } else {
            anyhow::bail!("must but inside a typst project or pass the project root using --root");
        }
    };

    let mut project = Project::new(root, Some("tests".into()));
    project.load_tests()?;

    let ctx = Context::new(project.clone(), args.typst);

    // wow rust makes this so easy
    thread::scope(|scope| -> anyhow::Result<()> {
        for test in project.tests() {
            scope.spawn(|| -> anyhow::Result<()> {
                test.run(&ctx)?;
                Ok(())
            });
        }

        Ok(())
    })?;

    Ok(())
}
