use std::io::Write;

use typst::visualize::Color;
use typst_test_lib::{compare, render};

use super::util::export;
use super::{run, Context};

#[derive(clap::Args, Debug, Clone)]
#[group(id = "compare-args")]
pub struct Args {
    #[command(flatten)]
    pub export_args: export::Args,

    /// The maximum delta in each channel of a pixel
    ///
    /// If a single channel (red/green/blue/alpha component) of a pixel differs
    /// by this much between reference and output the pixel is counted as a
    /// deviation.
    #[arg(long, default_value_t = 0)]
    pub max_delta: u8,

    /// The maximum deviation per reference
    ///
    /// If a reference and output image have more than the given deviations it's
    /// counted as a failure.
    #[arg(long, default_value_t = 0)]
    pub max_deviation: usize,
}

pub fn run(ctx: &mut Context, args: &Args) -> anyhow::Result<()> {
    let project = ctx.collect_tests(&args.export_args.run_args.op_args, None)?;

    let render_strategy = render::Strategy {
        pixel_per_pt: render::ppi_to_ppp(args.export_args.pixel_per_inch),
        fill: Color::WHITE,
    };

    let compare_strategy = compare::Strategy::Visual(compare::visual::Strategy::Simple {
        max_delta: args.max_delta,
        max_deviation: args.max_deviation,
    });

    // TODO: see super::export
    if args.export_args.pdf || args.export_args.svg {
        ctx.operation_failure(|r| writeln!(r, "PDF and SVGF export are not yet supported"))?;
        anyhow::bail!("Unsupported export mode used");
    }

    run::run(ctx, project, &args.export_args.run_args, |ctx| {
        ctx.with_compare_strategy(Some(compare_strategy))
            .with_render_strategy(Some(render_strategy))
            .with_no_save_temporary(args.export_args.no_save_temporary)
    })
}
