use std::io::prelude::*;
use std::io::SeekFrom;
use std::path::Path;

use crate::core::PackageId;
use crate::sources::registry::{MaybeLock, RegistryConfig, RegistryData};
use crate::util::errors::{CargoResult, CargoResultExt};
use crate::util::paths;
use crate::util::{Config, FileLock, Filesystem, Sha256};
use hex;

pub struct LocalRegistry<'cfg> {
    index_path: Filesystem,
    cache_path: Filesystem,
    root: Filesystem,
    config: &'cfg Config,
}

impl<'cfg> LocalRegistry<'cfg> {
    pub fn new(root: &Path, config: &'cfg Config, name: &str) -> LocalRegistry<'cfg> {
        LocalRegistry {
            cache_path: config.registry_cache_path().join(name),
            index_path: Filesystem::new(root.join("index")),
            root: Filesystem::new(root.to_path_buf()),
            config,
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

    fn load(
        &self,
        root: &Path,
        path: &Path,
        data: &mut dyn FnMut(&[u8]) -> CargoResult<()>,
    ) -> CargoResult<()> {
        data(&paths::read_bytes(&root.join(path))?)
    }

    fn config(&mut self) -> CargoResult<Option<RegistryConfig>> {
        // Local registries don't have configuration for remote APIs or anything
        // like that
        Ok(None)
    }

    fn update_index(&mut self) -> CargoResult<()> {
        // Nothing to update, we just use what's on disk. Verify it actually
        // exists though. We don't use any locks as we're just checking whether
        // these directories exist.
        let root = self.root.clone().into_path_unlocked();
        if !root.is_dir() {
            failure::bail!("local registry path is not a directory: {}", root.display())
        }
        let index_path = self.index_path.clone().into_path_unlocked();
        if !index_path.is_dir() {
            failure::bail!(
                "local registry index path is not a directory: {}",
                index_path.display()
            )
        }
        Ok(())
    }

    fn download(&mut self, pkg: PackageId, checksum: &str) -> CargoResult<MaybeLock> {
        let filename = format!("{}-{}.crate", pkg.name(), pkg.version());

        // Attempt to open an read-only lock first to avoid an exclusive write lock.
        //
        // If this fails then we fall through to the exclusive path where we copy
        // the file.
        if let Ok(dst) = self.cache_path.open_ro(&filename, self.config, &filename) {
            let meta = dst.file().metadata()?;
            if meta.len() > 0 {
                return Ok(MaybeLock::Ready(dst));
            }
        }

        self.config.shell().status("Unpacking", pkg)?;

        // Verify the checksum and copy over the .crate.
        let mut buf = Vec::new();
        let mut crate_file_source = self.root.open_ro(&filename, self.config, "crate file")?;
        let _ = crate_file_source
            .read_to_end(&mut buf)
            .chain_err(|| format!("failed to read `{}`", crate_file_source.path().display()))?;

        let mut state = Sha256::new();
        state.update(&buf);
        if hex::encode(state.finish()) != checksum {
            failure::bail!("failed to verify the checksum of `{}`", pkg)
        }

        let mut dst = self.cache_path.open_rw(&filename, self.config, &filename)?;
        dst.write_all(&buf)?;
        dst.seek(SeekFrom::Start(0))?;

        Ok(MaybeLock::Ready(dst))
    }

    fn finish_download(
        &mut self,
        _pkg: PackageId,
        _checksum: &str,
        _data: &[u8],
    ) -> CargoResult<FileLock> {
        panic!("this source doesn't download")
    }
}
