use typst_test_lib::config::ConfigError;

use super::{ConfigJson, UnknownConfigKeys};
use crate::cli::Context;
use crate::error::OperationFailure;

#[derive(clap::Args, Debug, Clone)]
#[group(id = "config-get-args")]
pub struct Args {
    /// The key to get the value for
    #[arg(num_args(1..), required = true)]
    keys: Vec<String>,
}

pub fn run(ctx: &mut Context, args: &Args) -> anyhow::Result<()> {
    let project = ctx.ensure_project()?;

    let config = project.config();
    let kvs: Vec<_> = args
        .keys
        .iter()
        .map(|key| (key.as_str(), config.get(key)))
        .collect();

    let mut errors = vec![];
    let mut vals = vec![];

    for (k, res) in kvs {
        match res {
            Ok(v) => vals.push((k, v)),
            Err(ConfigError::UnknownKey { .. }) => errors.push(k.to_owned()),
            Err(err) => anyhow::bail!(err),
        }
    }

    if !errors.is_empty() {
        anyhow::bail!(OperationFailure::from(UnknownConfigKeys(errors)));
    };

    ctx.reporter.report(&ConfigJson::Pretty {
        inner: vals.into_iter().collect(),
    })?;

    Ok(())
}
