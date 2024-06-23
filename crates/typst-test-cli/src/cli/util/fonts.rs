use typst::text::FontVariant;

use crate::cli::{CliResult, Context, Global};

#[derive(clap::Args, Debug, Clone)]
pub struct Args {
    /// List variants alongside fonts
    #[arg(long)]
    pub variants: bool,
}

pub fn run(_ctx: Context, global: &Global, args: &Args) -> anyhow::Result<CliResult> {
    for (name, infos) in global.fonts.searcher().book.families() {
        println!("{name}");
        if args.variants {
            for info in infos {
                let FontVariant {
                    style,
                    weight,
                    stretch,
                } = info.variant;
                println!("- Style: {style:?}, Weight: {weight:?}, Stretch: {stretch:?}");
            }
        }
    }

    Ok(CliResult::Ok)
}
