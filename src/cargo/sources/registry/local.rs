//! Access to a regstiry on the local filesystem. See [`LocalRegistry`] for more.

use crate::core::PackageId;
use crate::sources::registry::{LoadResponse, MaybeLock, RegistryConfig, RegistryData};
use crate::util::errors::CargoResult;
use crate::util::{Config, Filesystem};
use cargo_util::{paths, Sha256};
use std::fs::File;
use std::io::SeekFrom;
use std::io::{self, prelude::*};
use std::path::Path;
use std::task::Poll;

/// A local registry is a registry that lives on the filesystem as a set of
/// `.crate` files with an `index` directory in the [same format] as a remote
/// registry.
///
/// This type is primarily accessed through the [`RegistryData`] trait.
///
/// When a local registry is requested for a package, it simply looks into what
/// its index has under the `index` directory. When [`LocalRegistry::download`]
/// is called, a local registry verifies the checksum of the requested `.crate`
/// tarball and then unpacks it to `$CARGO_HOME/.registry/src`.
///
/// > Note that there is a third-party subcommand [`cargo-local-registry`],
/// > which happened to be developed by a former Cargo team member when local
/// > registry was introduced. The tool is to ease the burden of maintaining
/// > local registries. However, in general the Cargo team avoids recommending
/// > any specific third-party crate. Just FYI.
///
/// [same format]: super#the-format-of-the-index
/// [`cargo-local-registry`]: https://crates.io/crates/cargo-local-registry
///
/// # Filesystem hierarchy
///
/// Here is an example layout of a local registry on a local filesystem:
///
/// ```text
/// [registry root]/
/// ├── index/                      # registry index
/// │  ├── an/
/// │  │  └── yh/
/// │  │     └── anyhow
/// │  ├── ru/
/// │  │  └── st/
/// │  │     ├── rustls
/// │  │     └── rustls-ffi
/// │  └── se/
/// │     └── mv/
/// │        └── semver
/// ├── anyhow-1.0.71.crate         # pre-downloaded crate tarballs
/// ├── rustls-0.20.8.crate
/// ├── rustls-ffi-0.8.2.crate
/// └── semver-1.0.17.crate
/// ```
///
/// For general concepts of registries, see the [module-level documentation](crate::sources::registry).
pub struct LocalRegistry<'cfg> {
    /// Path to the registry index.
    index_path: Filesystem,
    /// Root path of this local registry.
    root: Filesystem,
    /// Path where this local registry extract `.crate` tarballs to.
    src_path: Filesystem,
    config: &'cfg Config,
    /// Whether this source has updated all package information it may contain.
    updated: bool,
    /// Disables status messages.
    quiet: bool,
}

impl<'cfg> LocalRegistry<'cfg> {
    /// Creates a local registry at `root`.
    ///
    /// * `name` --- Name of a path segment where `.crate` tarballs are stored.
    ///   Expect to be unique.
    pub fn new(root: &Path, config: &'cfg Config, name: &str) -> LocalRegistry<'cfg> {
        LocalRegistry {
            src_path: config.registry_source_path().join(name),
            index_path: Filesystem::new(root.join("index")),
            root: Filesystem::new(root.to_path_buf()),
            config,
            updated: false,
            quiet: false,
        }
    }
}

impl<'cfg> RegistryData for LocalRegistry<'cfg> {
    fn prepare(&self) -> CargoResult<()> {
        Ok(())
    }

    fn index_path(&self) -> &Filesystem {
        &self.index_path
    }

    fn assert_index_locked<'a>(&self, path: &'a Filesystem) -> &'a Path {
        // Note that the `*_unlocked` variant is used here since we're not
        // modifying the index and it's required to be externally synchronized.
        path.as_path_unlocked()
    }

    fn load(
        &mut self,
        root: &Path,
        path: &Path,
        _index_version: Option<&str>,
    ) -> Poll<CargoResult<LoadResponse>> {
        if self.updated {
            let raw_data = match paths::read_bytes(&root.join(path)) {
                Err(e)
                    if e.downcast_ref::<io::Error>()
                        .map_or(false, |ioe| ioe.kind() == io::ErrorKind::NotFound) =>
                {
                    return Poll::Ready(Ok(LoadResponse::NotFound));
                }
                r => r,
            }?;
            Poll::Ready(Ok(LoadResponse::Data {
                raw_data,
                index_version: None,
            }))
        } else {
            Poll::Pending
        }
    }

    fn config(&mut self) -> Poll<CargoResult<Option<RegistryConfig>>> {
        // Local registries don't have configuration for remote APIs or anything
        // like that
        Poll::Ready(Ok(None))
    }

    fn block_until_ready(&mut self) -> CargoResult<()> {
        if self.updated {
            return Ok(());
        }
        // Nothing to update, we just use what's on disk. Verify it actually
        // exists though. We don't use any locks as we're just checking whether
        // these directories exist.
        let root = self.root.clone().into_path_unlocked();
        if !root.is_dir() {
            anyhow::bail!("local registry path is not a directory: {}", root.display());
        }
        let index_path = self.index_path.clone().into_path_unlocked();
        if !index_path.is_dir() {
            anyhow::bail!(
                "local registry index path is not a directory: {}",
                index_path.display()
            );
        }
        self.updated = true;
        Ok(())
    }

    fn invalidate_cache(&mut self) {
        // Local registry has no cache - just reads from disk.
    }

    fn set_quiet(&mut self, _quiet: bool) {
        self.quiet = true;
    }

    fn is_updated(&self) -> bool {
        self.updated
    }

    fn download(&mut self, pkg: PackageId, checksum: &str) -> CargoResult<MaybeLock> {
        // Note that the usage of `into_path_unlocked` here is because the local
        // crate files here never change in that we're not the one writing them,
        // so it's not our responsibility to synchronize access to them.
        let path = self.root.join(&pkg.tarball_name()).into_path_unlocked();
        let mut crate_file = paths::open(&path)?;

        // If we've already got an unpacked version of this crate, then skip the
        // checksum below as it is in theory already verified.
        let dst = path.file_stem().unwrap();
        if self.src_path.join(dst).into_path_unlocked().exists() {
            return Ok(MaybeLock::Ready(crate_file));
        }

        if !self.quiet {
            self.config.shell().status("Unpacking", pkg)?;
        }

        // We don't actually need to download anything per-se, we just need to
        // verify the checksum matches the .crate file itself.
        let actual = Sha256::new().update_file(&crate_file)?.finish_hex();
        if actual != checksum {
            anyhow::bail!("failed to verify the checksum of `{}`", pkg)
        }

        crate_file.seek(SeekFrom::Start(0))?;

        Ok(MaybeLock::Ready(crate_file))
    }

    fn finish_download(
        &mut self,
        _pkg: PackageId,
        _checksum: &str,
        _data: &[u8],
    ) -> CargoResult<File> {
        panic!("this source doesn't download")
    }
}
