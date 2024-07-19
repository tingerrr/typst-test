// SPDX-License-Identifier: Apache-2.0
// Credits: The Typst Authors

use std::fs;
use std::path::{Path, PathBuf};

use ecow::eco_format;
use once_cell::sync::OnceCell;
use typst::diag::{bail, PackageError, PackageResult, StrResult};
use typst::syntax::package::{PackageInfo, PackageSpec, PackageVersion, VersionlessPackageSpec};

use crate::cli::PackageArgs;
use crate::download::Downloader;

/// The default registry host.
pub const HOST: &str = "https://packages.typst.org";

/// The default packages sub directory.
pub const DEFAULT_PACKAGES_SUBDIR: &str = "typst/packages";

/// Holds information about where packages should be stored.
pub struct PackageStorage {
    /// The directory to store downloaded packages in.
    package_cache_path: Option<PathBuf>,

    /// The packages to search for packages in.
    package_path: Option<PathBuf>,

    /// The cached index.
    index: OnceCell<Vec<PackageInfo>>,

    /// The downlaoder to use for package downloads.
    downloader: Downloader,
}

impl PackageStorage {
    pub fn from_args(args: &PackageArgs) -> Self {
        let package_cache_path = args
            .package_cache_path
            .clone()
            .or_else(|| dirs::cache_dir().map(|cache_dir| cache_dir.join(DEFAULT_PACKAGES_SUBDIR)));
        let package_path = args
            .package_path
            .clone()
            .or_else(|| dirs::data_dir().map(|data_dir| data_dir.join(DEFAULT_PACKAGES_SUBDIR)));
        Self {
            package_cache_path,
            package_path,
            index: OnceCell::new(),
            downloader: Downloader::new(args.certificate.clone()),
        }
    }

    pub fn package_root(&self, spec: &PackageSpec) -> PackageResult<PathBuf> {
        let subdir = format!("{}/{}/{}", spec.namespace, spec.name, spec.version);

        if let Some(packages_dir) = &self.package_path {
            let dir = packages_dir.join(&subdir);
            if dir.exists() {
                return Ok(dir);
            }
        }

        if let Some(cache_dir) = &self.package_cache_path {
            let dir = cache_dir.join(&subdir);
            if dir.exists() {
                return Ok(dir);
            }

            if spec.namespace == "preview" {
                self.download_package(spec, &dir)?;
                if dir.exists() {
                    return Ok(dir);
                }
            }
        }

        Err(PackageError::NotFound(spec.clone()))
    }

    /// Try to determine the latest version of a package.
    pub fn determine_latest_version(
        &self,
        spec: &VersionlessPackageSpec,
    ) -> StrResult<PackageVersion> {
        if spec.namespace == "preview" {
            // For `@preview`, download the package index and find the latest
            // version.
            self.download_index()?
                .iter()
                .filter(|package| package.name == spec.name)
                .map(|package| package.version)
                .max()
                .ok_or_else(|| eco_format!("failed to find package {spec}"))
        } else {
            // For other namespaces, search locally. We only search in the data
            // directory and not the cache directory, because the latter is not
            // intended for storage of local packages.
            let subdir = format!("{}/{}", spec.namespace, spec.name);
            self.package_path
                .iter()
                .flat_map(|dir| std::fs::read_dir(dir.join(&subdir)).ok())
                .flatten()
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter_map(|path| path.file_name()?.to_string_lossy().parse().ok())
                .max()
                .ok_or_else(|| eco_format!("please specify the desired version"))
        }
    }

    /// Download a package over the network.
    fn download_package(&self, spec: &PackageSpec, package_dir: &Path) -> PackageResult<()> {
        // The `@preview` namespace is the only namespace that supports on-demand
        // fetching.
        assert_eq!(spec.namespace, "preview");

        let url = format!("{HOST}/preview/{}-{}.tar.gz", spec.name, spec.version);

        // let mut reporter = self.reporter.lock().unwrap();
        // reporter
        //     .write_annotated("download", Color::Cyan, |r| write!(r, "{spec}"))
        //     .unwrap();

        let data = match self.downloader.download_with_progress(&url) {
            Ok(data) => data,
            Err(ureq::Error::Status(404, _)) => {
                if let Ok(version) = self.determine_latest_version(&VersionlessPackageSpec {
                    namespace: spec.namespace.clone(),
                    name: spec.name.clone(),
                }) {
                    // TODO: version not found variant is already on upstream
                    return Err(PackageError::Other(Some(eco_format!(
                        "version {} not found, latest version is {}",
                        spec.clone(),
                        version
                    ))));
                } else {
                    return Err(PackageError::NotFound(spec.clone()));
                }
            }
            Err(err) => return Err(PackageError::NetworkFailed(Some(eco_format!("{err}")))),
        };

        let decompressed = flate2::read::GzDecoder::new(data.as_slice());
        tar::Archive::new(decompressed)
            .unpack(package_dir)
            .map_err(|err| {
                fs::remove_dir_all(package_dir).ok();
                PackageError::MalformedArchive(Some(eco_format!("{err}")))
            })
    }

    /// Download the `@preview` package index.
    ///
    /// To avoid downloading the index multiple times, the result is cached.
    fn download_index(&self) -> StrResult<&Vec<PackageInfo>> {
        self.index.get_or_try_init(|| {
            let url = format!("{HOST}/preview/index.json");
            match self.downloader.download(&url) {
                Ok(response) => response
                    .into_json()
                    .map_err(|err| eco_format!("failed to parse package index: {err}")),
                Err(ureq::Error::Status(404, _)) => {
                    bail!("failed to fetch package index (not found)")
                }
                Err(err) => bail!("failed to fetch package index ({err})"),
            }
        })
    }
}
