use std::fs::{self, File};
use std::io::prelude::*;
use std::path::{PathBuf, Path};

use rustc_serialize::hex::ToHex;

use core::PackageId;
use sources::registry::{RegistryData, RegistryConfig};
use util::{Config, CargoResult, ChainError, human, Sha256};

pub struct LocalRegistry<'cfg> {
    index_path: PathBuf,
    root: PathBuf,
    src_path: PathBuf,
    config: &'cfg Config,
}

impl<'cfg> LocalRegistry<'cfg> {
    pub fn new(root: &Path,
               config: &'cfg Config,
               name: &str) -> LocalRegistry<'cfg> {
        LocalRegistry {
            src_path: config.registry_source_path().join(name),
            index_path: root.join("index"),
            root: root.to_path_buf(),
            config: config,
        }
    }
}

impl<'cfg> RegistryData for LocalRegistry<'cfg> {
    fn index_path(&self) -> &Path {
        &self.index_path
    }

    fn config(&self) -> CargoResult<Option<RegistryConfig>> {
        // Local registries don't have configuration for remote APIs or anything
        // like that
        Ok(None)
    }

    fn update_index(&mut self) -> CargoResult<()> {
        // Nothing to update, we just use what's on disk. Verify it actually
        // exists though
        if !self.root.is_dir() {
            bail!("local registry path is not a directory: {}",
                  self.root.display())
        }
        if !self.index_path.is_dir() {
            bail!("local registry index path is not a directory: {}",
                  self.index_path.display())
        }
        Ok(())
    }

    fn download(&mut self, pkg: &PackageId, checksum: &str)
                -> CargoResult<PathBuf> {
        let crate_file = format!("{}-{}.crate", pkg.name(), pkg.version());
        let crate_file = self.root.join(&crate_file);

        // If we've already got an unpacked version of this crate, then skip the
        // checksum below as it is in theory already verified.
        let dst = format!("{}-{}", pkg.name(), pkg.version());
        let dst = self.src_path.join(&dst);
        if fs::metadata(&dst).is_ok() {
            return Ok(crate_file)
        }

        try!(self.config.shell().status("Unpacking", pkg));

        // We don't actually need to download anything per-se, we just need to
        // verify the checksum matches the .crate file itself.
        let mut file = try!(File::open(&crate_file).chain_error(|| {
            human(format!("failed to read `{}` for `{}`", crate_file.display(),
                          pkg))
        }));
        let mut data = Vec::new();
        try!(file.read_to_end(&mut data).chain_error(|| {
            human(format!("failed to read `{}`", crate_file.display()))
        }));
        let mut state = Sha256::new();
        state.update(&data);
        if state.finish().to_hex() != checksum {
            bail!("failed to verify the checksum of `{}`", pkg)
        }

        Ok(crate_file)
    }
}
