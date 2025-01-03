use std::io::Write;

use serde::Serialize;
use termcolor::WriteColor;
use typst::text::FontStyle;

use crate::cli::Context;
use crate::report::{Report, Verbosity};
use crate::ui::Indented;
use crate::{kit, ui};

#[derive(clap::Args, Debug, Clone)]
pub struct Args {
    /// List variants alongside fonts
    #[arg(long)]
    pub variants: bool,
}

#[derive(Debug, Serialize)]
struct FontVariantJson {
    style: &'static str,
    weight: u16,
    stretch: f64,
}

#[derive(Debug, Serialize)]
struct FontJson<'f> {
    name: &'f str,
    variants: Vec<FontVariantJson>,
}

#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct FontsReport<'f>(Vec<FontJson<'f>>);

impl Report for FontsReport<'_> {
    fn report<W: WriteColor>(&self, mut writer: W, _verbosity: Verbosity) -> anyhow::Result<()> {
        ui::write_bold(&mut writer, |w| writeln!(w, "Fonts"))?;

        let mut w = Indented::new(writer, 2);
        for font in &self.0 {
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
}

pub fn run(ctx: &mut Context, args: &Args) -> anyhow::Result<()> {
    let fonts = kit::fonts_from_args(&ctx.args.global.fonts);

    ctx.reporter.report(&FontsReport(
        fonts
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
            .collect(),
    ))?;

    Ok(())
}
