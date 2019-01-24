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
    root: Filesystem,
    src_path: Filesystem,
    config: &'cfg Config,
}

impl<'cfg> LocalRegistry<'cfg> {
    pub fn new(root: &Path, config: &'cfg Config, name: &str) -> LocalRegistry<'cfg> {
        LocalRegistry {
            src_path: config.registry_source_path().join(name),
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
        let crate_file = format!("{}-{}.crate", pkg.name(), pkg.version());
        let mut crate_file = self.root.open_ro(&crate_file, self.config, "crate file")?;

        // If we've already got an unpacked version of this crate, then skip the
        // checksum below as it is in theory already verified.
        let dst = format!("{}-{}", pkg.name(), pkg.version());
        if self.src_path.join(dst).into_path_unlocked().exists() {
            return Ok(MaybeLock::Ready(crate_file));
        }

        self.config.shell().status("Unpacking", pkg)?;

        // We don't actually need to download anything per-se, we just need to
        // verify the checksum matches the .crate file itself.
        let mut state = Sha256::new();
        let mut buf = [0; 64 * 1024];
        loop {
            let n = crate_file
                .read(&mut buf)
                .chain_err(|| format!("failed to read `{}`", crate_file.path().display()))?;
            if n == 0 {
                break;
            }
            state.update(&buf[..n]);
        }
        if hex::encode(state.finish()) != checksum {
            failure::bail!("failed to verify the checksum of `{}`", pkg)
        }

        crate_file.seek(SeekFrom::Start(0))?;

        Ok(MaybeLock::Ready(crate_file))
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
