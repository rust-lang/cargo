use crate::core::{PackageId, SourceId};
use crate::sources::registry::{LoadResponse, MaybeLock, RegistryConfig, RegistryData};
use crate::util::{CargoResult, Filesystem, GlobalContext};
use std::fs::File;
use std::path::Path;
use std::task::Poll;

pub struct RemoteRegistry<'gctx> {
    index_path: Filesystem,
    cache_path: Filesystem,
    gctx: &'gctx GlobalContext,
}

impl<'gctx> RemoteRegistry<'gctx> {
    pub fn new(
        _source_id: SourceId,
        gctx: &'gctx GlobalContext,
        name: &str,
    ) -> RemoteRegistry<'gctx> {
        RemoteRegistry {
            index_path: gctx.registry_index_path().join(name),
            cache_path: gctx.registry_cache_path().join(name),
            gctx,
        }
    }
}

impl<'gctx> RegistryData for RemoteRegistry<'gctx> {
    fn prepare(&self) -> CargoResult<()> {
        Ok(())
    }

    fn index_path(&self) -> &Filesystem {
        &self.index_path
    }

    fn cache_path(&self) -> &Filesystem {
        &self.cache_path
    }

    fn load(
        &mut self,
        _root: &Path,
        _path: &Path,
        _index_version: Option<&str>,
    ) -> Poll<CargoResult<LoadResponse>> {
        Poll::Ready(Err(anyhow::anyhow!(
            "git registries are not available in the WASI Cargo CLI yet"
        )))
    }

    fn config(&mut self) -> Poll<CargoResult<Option<RegistryConfig>>> {
        Poll::Ready(Err(anyhow::anyhow!(
            "git registries are not available in the WASI Cargo CLI yet"
        )))
    }

    fn invalidate_cache(&mut self) {}

    fn set_quiet(&mut self, _quiet: bool) {}

    fn is_updated(&self) -> bool {
        false
    }

    fn download(&mut self, _pkg: PackageId, _checksum: &str) -> CargoResult<MaybeLock> {
        anyhow::bail!("registry downloads are not available in the WASI Cargo CLI yet")
    }

    fn finish_download(
        &mut self,
        _pkg: PackageId,
        _checksum: &str,
        _data: &[u8],
    ) -> CargoResult<File> {
        anyhow::bail!("registry downloads are not available in the WASI Cargo CLI yet")
    }

    fn is_crate_downloaded(&self, _pkg: PackageId) -> bool {
        false
    }

    fn assert_index_locked<'a>(&self, path: &'a Filesystem) -> &'a Path {
        self.gctx.assert_package_cache_locked(
            crate::util::cache_lock::CacheLockMode::DownloadExclusive,
            path,
        )
    }

    fn block_until_ready(&mut self) -> CargoResult<()> {
        anyhow::bail!("git registries are not available in the WASI Cargo CLI yet")
    }
}
