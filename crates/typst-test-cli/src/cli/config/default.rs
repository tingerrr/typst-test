use typst_test_lib::config;

use super::ConfigJson;
use crate::cli::Context;

#[derive(clap::Args, Debug, Clone)]
#[group(id = "config-set-args")]
pub struct Args {
    /// Whether to output the config as a manifest tool section
    #[arg(long, conflicts_with = "format")]
    toml: bool,
}

pub fn run(ctx: &mut Context, args: &Args) -> anyhow::Result<()> {
    let mut config = config::Config::default();
    config.set_fallbacks();

    ctx.reporter.report(&if args.toml {
        ConfigJson::Toml { inner: &config }
    } else {
        ConfigJson::Pretty {
            inner: config.pairs().collect(),
        }
    })?;

    Ok(())
}
