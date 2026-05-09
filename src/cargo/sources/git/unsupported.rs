use crate::core::{Dependency, Package, PackageId, SourceId};
use crate::sources::IndexSummary;
use crate::sources::source::{MaybePackage, QueryKind, Source};
use crate::util::{CargoResult, GlobalContext};
use std::marker::PhantomData;
use std::task::Poll;
use url::Url;

pub struct GitRemote;
pub struct GitDatabase;
pub struct GitCheckout<'a>(PhantomData<&'a ()>);

pub struct GitSource<'gctx> {
    source_id: SourceId,
    _gctx: &'gctx GlobalContext,
}

impl<'gctx> GitSource<'gctx> {
    pub fn new(source_id: SourceId, gctx: &'gctx GlobalContext) -> CargoResult<GitSource<'gctx>> {
        assert!(source_id.is_git(), "id is not git, id={}", source_id);
        Ok(GitSource {
            source_id,
            _gctx: gctx,
        })
    }

    pub fn url(&self) -> &Url {
        self.source_id.url()
    }

    pub fn read_packages(&mut self) -> CargoResult<Vec<Package>> {
        anyhow::bail!("git sources are not available in the WASI Cargo CLI yet")
    }
}

impl<'gctx> Source for GitSource<'gctx> {
    fn source_id(&self) -> SourceId {
        self.source_id
    }

    fn supports_checksums(&self) -> bool {
        false
    }

    fn requires_precise(&self) -> bool {
        true
    }

    fn query(
        &mut self,
        _dep: &Dependency,
        _kind: QueryKind,
        _f: &mut dyn FnMut(IndexSummary),
    ) -> Poll<CargoResult<()>> {
        Poll::Ready(Err(anyhow::anyhow!(
            "git sources are not available in the WASI Cargo CLI yet"
        )))
    }

    fn invalidate_cache(&mut self) {}

    fn set_quiet(&mut self, _quiet: bool) {}

    fn download(&mut self, _package: PackageId) -> CargoResult<MaybePackage> {
        anyhow::bail!("git sources are not available in the WASI Cargo CLI yet")
    }

    fn finish_download(&mut self, _pkg_id: PackageId, _contents: Vec<u8>) -> CargoResult<Package> {
        anyhow::bail!("git sources are not available in the WASI Cargo CLI yet")
    }

    fn fingerprint(&self, _pkg: &Package) -> CargoResult<String> {
        anyhow::bail!("git sources are not available in the WASI Cargo CLI yet")
    }

    fn describe(&self) -> String {
        format!("Git repository {}", self.source_id)
    }

    fn add_to_yanked_whitelist(&mut self, _pkgs: &[PackageId]) {}

    fn is_yanked(&mut self, _pkg: PackageId) -> Poll<CargoResult<bool>> {
        Poll::Ready(Ok(false))
    }

    fn block_until_ready(&mut self) -> CargoResult<()> {
        anyhow::bail!("git sources are not available in the WASI Cargo CLI yet")
    }
}

pub fn fetch() -> CargoResult<()> {
    anyhow::bail!("git fetch is not available in the WASI Cargo CLI yet")
}

pub fn resolve_ref() -> CargoResult<()> {
    anyhow::bail!("git ref resolution is not available in the WASI Cargo CLI yet")
}
