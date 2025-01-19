use std::io::Write;

use color_eyre::eyre;
use typst::text::FontStyle;

use crate::cli::Context;
use crate::json::{FontJson, FontVariantJson};
use crate::ui::Indented;
use crate::{kit, ui};

#[derive(clap::Args, Debug, Clone)]
#[group(id = "util-font-args")]
pub struct Args {
    /// List variants alongside fonts
    #[arg(long)]
    pub variants: bool,

    /// Print a JSON describing the project to stdout
    #[arg(long)]
    pub json: bool,
}

pub fn run(ctx: &mut Context, args: &Args) -> eyre::Result<()> {
    let fonts = kit::fonts_from_args(&ctx.args.global.fonts);

    let fonts = fonts
        .book
        .families()
        .map(|(name, info)| FontJson {
            name,
            variants: if args.variants {
                info.map(|info| FontVariantJson {
                    style: match info.variant.style {
                        FontStyle::Normal => "normal",
                        FontStyle::Italic => "italic",
                        FontStyle::Oblique => "oblique",
                    },
                    weight: info.variant.weight.to_number(),
                    stretch: info.variant.stretch.to_ratio().get(),
                })
                .collect()
            } else {
                vec![]
            },
        })
        .collect::<Vec<_>>();

    if args.json {
        serde_json::to_writer_pretty(ctx.ui.stdout(), &fonts)?;
        return Ok(());
    }

    let mut w = ctx.ui.stderr();

    ui::write_bold(&mut w, |w| writeln!(w, "Fonts"))?;

    let mut w = Indented::new(w, 2);
    for font in fonts {
        ui::write_ident(&mut w, |w| writeln!(w, "{}", font.name))?;

        let mut w = Indented::new(&mut w, 2);
        for variant in &font.variants {
            writeln!(
                w,
                "Style: {}, Weight: {}, Stretch: {}",
                variant.style, variant.weight, variant.stretch
            )?;
        }
    }

    Ok(())
}
