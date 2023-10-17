use std::collections::HashMap;
use std::fmt::{self, Debug, Formatter};
use std::path::{Path, PathBuf};
use std::task::Poll;

use crate::core::{Dependency, Package, PackageId, SourceId, Summary};
use crate::sources::source::MaybePackage;
use crate::sources::source::QueryKind;
use crate::sources::source::Source;
use crate::sources::PathSource;
use crate::util::errors::CargoResult;
use crate::util::Config;

use anyhow::Context as _;
use cargo_util::{paths, Sha256};
use serde::Deserialize;

/// `DirectorySource` contains a number of crates on the file system. It was
/// designed for representing vendored dependencies for `cargo vendor`.
///
/// `DirectorySource` at this moment is just a root directory containing other
/// directories, which contain the source files of packages. Assumptions would
/// be made to determine if a directory should be included as a package of a
/// directory source's:
///
/// * Ignore directories starting with dot `.` (tend to be hidden).
/// * Only when a `Cargo.toml` exists in a directory will it be included as
///   a package. `DirectorySource` at this time only looks at one level of
///   directories and never went deeper.
/// * There must be a [`Checksum`] file `.cargo-checksum.json` file at the same
///   level of `Cargo.toml` to ensure the integrity when a directory source was
///   created (usually by `cargo vendor`). A failure to find or parse a single
///   checksum results in a denial of loading any package in this source.
/// * Otherwise, there is no other restrction of the name of directories. At
///   this moment, it is `cargo vendor` that defines the layout and the name of
///   each directory.
///
/// The file tree of a directory source may look like:
///
/// ```text
/// [source root]
/// ├── a-valid-crate/
/// │  ├── src/
/// │  ├── .cargo-checksum.json
/// │  └── Cargo.toml
/// ├── .ignored-a-dot-crate/
/// │  ├── src/
/// │  ├── .cargo-checksum.json
/// │  └── Cargo.toml
/// ├── skipped-no-manifest/
/// │  ├── src/
/// │  └── .cargo-checksum.json
/// └── no-checksum-so-fails-the-entire-source-reading/
///    └── Cargo.toml
/// ```
pub struct DirectorySource<'cfg> {
    /// The unique identifier of this source.
    source_id: SourceId,
    /// The root path of this source.
    root: PathBuf,
    /// Packages that this sources has discovered.
    packages: HashMap<PackageId, (Package, Checksum)>,
    config: &'cfg Config,
    updated: bool,
}

/// The checksum file to ensure the integrity of a package in a directory source.
///
/// The file name is simply `.cargo-checksum.json`. The checksum algorithm as
/// of now is SHA256.
#[derive(Deserialize)]
struct Checksum {
    /// Checksum of the package. Normally it is computed from the `.crate` file.
    package: Option<String>,
    /// Checksums of each source file.
    files: HashMap<String, String>,
}

impl<'cfg> DirectorySource<'cfg> {
    pub fn new(path: &Path, id: SourceId, config: &'cfg Config) -> DirectorySource<'cfg> {
        DirectorySource {
            source_id: id,
            root: path.to_path_buf(),
            config,
            packages: HashMap::new(),
            updated: false,
        }
    }
}

impl<'cfg> Debug for DirectorySource<'cfg> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "DirectorySource {{ root: {:?} }}", self.root)
    }
}

impl<'cfg> Source for DirectorySource<'cfg> {
    fn query(
        &mut self,
        dep: &Dependency,
        kind: QueryKind,
        f: &mut dyn FnMut(Summary),
    ) -> Poll<CargoResult<()>> {
        if !self.updated {
            return Poll::Pending;
        }
        let packages = self.packages.values().map(|p| &p.0);
        let matches = packages.filter(|pkg| match kind {
            QueryKind::Exact => dep.matches(pkg.summary()),
            QueryKind::Fuzzy => true,
        });
        for summary in matches.map(|pkg| pkg.summary().clone()) {
            f(summary);
        }
        Poll::Ready(Ok(()))
    }

    fn supports_checksums(&self) -> bool {
        true
    }

    fn requires_precise(&self) -> bool {
        true
    }

    fn source_id(&self) -> SourceId {
        self.source_id
    }

    fn block_until_ready(&mut self) -> CargoResult<()> {
        if self.updated {
            return Ok(());
        }
        self.packages.clear();
        let entries = self.root.read_dir().with_context(|| {
            format!(
                "failed to read root of directory source: {}",
                self.root.display()
            )
        })?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            // Ignore hidden/dot directories as they typically don't contain
            // crates and otherwise may conflict with a VCS
            // (rust-lang/cargo#3414).
            if let Some(s) = path.file_name().and_then(|s| s.to_str()) {
                if s.starts_with('.') {
                    continue;
                }
            }

            // Vendor directories are often checked into a VCS, but throughout
            // the lifetime of a vendor dir crates are often added and deleted.
            // Some VCS implementations don't always fully delete the directory
            // when a dir is removed from a different checkout. Sometimes a
            // mostly-empty dir is left behind.
            //
            // Additionally vendor directories are sometimes accompanied with
            // readme files and other auxiliary information not too interesting
            // to Cargo.
            //
            // To help handle all this we only try processing folders with a
            // `Cargo.toml` in them. This has the upside of being pretty
            // flexible with the contents of vendor directories but has the
            // downside of accidentally misconfigured vendor directories
            // silently returning less crates.
            if !path.join("Cargo.toml").exists() {
                continue;
            }

            let mut src = PathSource::new(&path, self.source_id, self.config);
            src.update()?;
            let mut pkg = src.root_package()?;

            let cksum_file = path.join(".cargo-checksum.json");
            let cksum = paths::read(&path.join(cksum_file)).with_context(|| {
                format!(
                    "failed to load checksum `.cargo-checksum.json` \
                     of {} v{}",
                    pkg.package_id().name(),
                    pkg.package_id().version()
                )
            })?;
            let cksum: Checksum = serde_json::from_str(&cksum).with_context(|| {
                format!(
                    "failed to decode `.cargo-checksum.json` of \
                     {} v{}",
                    pkg.package_id().name(),
                    pkg.package_id().version()
                )
            })?;

            if let Some(package) = &cksum.package {
                pkg.manifest_mut()
                    .summary_mut()
                    .set_checksum(package.clone());
            }
            self.packages.insert(pkg.package_id(), (pkg, cksum));
        }

        self.updated = true;
        Ok(())
    }

    fn download(&mut self, id: PackageId) -> CargoResult<MaybePackage> {
        self.packages
            .get(&id)
            .map(|p| &p.0)
            .cloned()
            .map(MaybePackage::Ready)
            .ok_or_else(|| anyhow::format_err!("failed to find package with id: {}", id))
    }

    fn finish_download(&mut self, _id: PackageId, _data: Vec<u8>) -> CargoResult<Package> {
        panic!("no downloads to do")
    }

    fn fingerprint(&self, pkg: &Package) -> CargoResult<String> {
        Ok(pkg.package_id().version().to_string())
    }

    fn verify(&self, id: PackageId) -> CargoResult<()> {
        let Some((pkg, cksum)) = self.packages.get(&id) else {
            anyhow::bail!("failed to find entry for `{}` in directory source", id);
        };

        for (file, cksum) in cksum.files.iter() {
            let file = pkg.root().join(file);
            let actual = Sha256::new()
                .update_path(&file)
                .with_context(|| format!("failed to calculate checksum of: {}", file.display()))?
                .finish_hex();
            if &*actual != cksum {
                anyhow::bail!(
                    "the listed checksum of `{}` has changed:\n\
                     expected: {}\n\
                     actual:   {}\n\
                     \n\
                     directory sources are not intended to be edited, if \
                     modifications are required then it is recommended \
                     that `[patch]` is used with a forked copy of the \
                     source\
                     ",
                    file.display(),
                    cksum,
                    actual
                );
            }
        }

        Ok(())
    }

    fn describe(&self) -> String {
        format!("directory source `{}`", self.root.display())
    }

    fn add_to_yanked_whitelist(&mut self, _pkgs: &[PackageId]) {}

    fn is_yanked(&mut self, _pkg: PackageId) -> Poll<CargoResult<bool>> {
        Poll::Ready(Ok(false))
    }

    fn invalidate_cache(&mut self) {
        // Directory source has no local cache.
    }

    fn set_quiet(&mut self, _quiet: bool) {
        // Directory source does not display status
    }
}
