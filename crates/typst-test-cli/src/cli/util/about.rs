use std::io::Write;

use color_eyre::eyre;

use super::Context;

pub fn run(ctx: &mut Context) -> eyre::Result<()> {
    let mut w = ctx.ui.stderr();
    writeln!(w, "Version: {}", env!("CARGO_PKG_VERSION"))?;
    writeln!(w, "Typst Version: 0.12.0")?;

    Ok(())
}
