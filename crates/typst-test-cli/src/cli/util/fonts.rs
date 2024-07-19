use std::io::Write;

use termcolor::Color;
use typst::text::FontVariant;

use crate::cli::Context;

#[derive(clap::Args, Debug, Clone)]
pub struct Args {
    /// List variants alongside fonts
    #[arg(long)]
    pub variants: bool,
}

pub fn run(ctx: &mut Context, args: &Args) -> anyhow::Result<()> {
    let mut reporter = ctx.reporter.lock().unwrap();

    for (name, infos) in ctx.args.global.fonts.searcher().book.families() {
        reporter.write_annotated("font", Color::Cyan, None, |r| {
            writeln!(r, "{name}")?;
            if args.variants {
                for info in infos {
                    let FontVariant {
                        style,
                        weight,
                        stretch,
                    } = info.variant;
                    r.write_annotated("variant", Color::Cyan, None, |r| {
                        writeln!(
                            r,
                            "Style: {style:?}, Weight: {weight:?}, Stretch: {stretch:?}",
                        )
                    })?;
                }
            }

            Ok(())
        })?;
    }

    Ok(())
}
