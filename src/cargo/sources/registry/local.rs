use std::io::SeekFrom;
use std::io::prelude::*;
use std::path::Path;

use core::PackageId;
use hex::ToHex;
use sources::registry::{RegistryData, RegistryConfig};
use util::FileLock;
use util::paths;
use util::{Config, Sha256, Filesystem};
use util::errors::{CargoResult, CargoResultExt};

pub struct LocalRegistry<'cfg> {
    index_path: Filesystem,
    root: Filesystem,
    src_path: Filesystem,
    config: &'cfg Config,
}

impl<'cfg> LocalRegistry<'cfg> {
    pub fn new(root: &Path,
               config: &'cfg Config,
               name: &str) -> LocalRegistry<'cfg> {
        LocalRegistry {
            src_path: config.registry_source_path().join(name),
            index_path: Filesystem::new(root.join("index")),
            root: Filesystem::new(root.to_path_buf()),
            config: config,
        }
    }
}

impl<'cfg> RegistryData for LocalRegistry<'cfg> {
    fn index_path(&self) -> &Filesystem {
        &self.index_path
    }

    fn load(&self,
            root: &Path,
            path: &Path,
            data: &mut FnMut(&[u8]) -> CargoResult<()>) -> CargoResult<()> {
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
            bail!("local registry path is not a directory: {}",
                  root.display())
        }
        let index_path = self.index_path.clone().into_path_unlocked();
        if !index_path.is_dir() {
            bail!("local registry index path is not a directory: {}",
                  index_path.display())
        }
        Ok(())
    }

    fn download(&mut self, pkg: &PackageId, checksum: &str)
                -> CargoResult<FileLock> {
        let crate_file = format!("{}-{}.crate", pkg.name(), pkg.version());
        let mut crate_file = self.root.open_ro(&crate_file,
                                               self.config,
                                               "crate file")?;

        // If we've already got an unpacked version of this crate, then skip the
        // checksum below as it is in theory already verified.
        let dst = format!("{}-{}", pkg.name(), pkg.version());
        if self.src_path.join(dst).into_path_unlocked().exists() {
            return Ok(crate_file)
        }

        self.config.shell().status("Unpacking", pkg)?;

        // We don't actually need to download anything per-se, we just need to
        // verify the checksum matches the .crate file itself.
        let mut state = Sha256::new();
        let mut buf = [0; 64 * 1024];
        loop {
            let n = crate_file.read(&mut buf).chain_err(|| {
                format!("failed to read `{}`", crate_file.path().display())
            })?;
            if n == 0 {
                break
            }
            state.update(&buf[..n]);
        }
        if state.finish().to_hex() != checksum {
            bail!("failed to verify the checksum of `{}`", pkg)
        }

        crate_file.seek(SeekFrom::Start(0))?;

        Ok(crate_file)
    }
}
