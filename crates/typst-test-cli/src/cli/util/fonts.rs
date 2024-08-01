use std::io;
use std::io::Write;

use serde::Serialize;
use termcolor::WriteColor;
use typst::text::FontStyle;

use crate::cli::Context;
use crate::report::{Report, Verbosity};
use crate::ui;
use crate::ui::{Heading, Indented};

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
pub struct FontsReport<'f> {
    fonts: Vec<FontJson<'f>>,
}

impl Report for FontsReport<'_> {
    fn report<W: WriteColor>(&self, writer: W, _verbosity: Verbosity) -> io::Result<()> {
        let mut w = Heading::new(writer, "Fonts");
        for font in &self.fonts {
            ui::write_ident(&mut w, |w| writeln!(w, "{}", font.name))?;

            let w = &mut Indented::new(&mut w, 2);
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
    let fonts = ctx.args.global.fonts.searcher();

    ctx.reporter.lock().unwrap().report(&FontsReport {
        fonts: fonts
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
    })?;

    Ok(())
}
