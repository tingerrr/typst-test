use std::path::PathBuf;

use color_eyre::eyre;
use typst_kit::download::Downloader;
use typst_kit::fonts::{FontSearcher, Fonts};
use typst_kit::package::PackageStorage;

use crate::cli::{CompileArgs, FontArgs, PackageArgs};
use crate::world::SystemWorld;

pub fn world(
    project_root: PathBuf,
    font_args: &FontArgs,
    package_args: &PackageArgs,
    compile_args: &CompileArgs,
) -> eyre::Result<SystemWorld> {
    let world = SystemWorld::new(
        project_root,
        fonts_from_args(font_args),
        package_storage_from_args(package_args),
        compile_args.now,
    )?;

    Ok(world)
}

pub fn downloader_from_args(args: &PackageArgs) -> Downloader {
    let agent = format!("{}/{}", lib::TOOL_NAME, env!("CARGO_PKG_VERSION"));

    match args.certificate.clone() {
        Some(path) => Downloader::with_path(agent, path),
        None => Downloader::new(agent),
    }
}

pub fn package_storage_from_args(args: &PackageArgs) -> PackageStorage {
    PackageStorage::new(
        args.package_cache_path.clone(),
        args.package_path.clone(),
        downloader_from_args(args),
    )
}

pub fn fonts_from_args(args: &FontArgs) -> Fonts {
    let _span = tracing::debug_span!(
        "searching for fonts",
        paths = ?args.font_paths,
        include_system_fonts = ?!args.ignore_system_fonts,
    );

    let mut searcher = FontSearcher::new();

    #[cfg(feature = "embed-fonts")]
    searcher.include_embedded_fonts(true);
    searcher.include_system_fonts(!args.ignore_system_fonts);

    let fonts = searcher.search_with(args.font_paths.iter().map(PathBuf::as_path));

    tracing::debug!(fonts = ?fonts.fonts.len(), "collected fonts");
    fonts
}
