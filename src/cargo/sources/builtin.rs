use std::{cell::RefCell, path::Path};

use crate::{
    CargoResult, GlobalContext,
    core::{Dependency, Package, PackageId, SourceId, Summary},
    sources::source::{MaybePackage, QueryKind, Source},
    sources::{IndexSummary, PathSource},
};

/// A builtin source represents standard library packages used in build-std, which are "built into"
/// the toolchain.
///
/// It is very similar to a path source, but as all builtin dependencies are opaque
/// returns an opaque summary when queried, with no dependencies, and is
pub struct BuiltinSource<'gctx> {
    /// The unique identifier for this source
    source_id: SourceId,
    /// The underlying path source which discovers packages
    path_source: PathSource<'gctx>,
    /// The cached opaque summary
    opaque_summary: RefCell<Option<Summary>>,
}

impl<'gctx> BuiltinSource<'gctx> {
    pub fn from_path(path: &Path, source_id: SourceId, gctx: &'gctx GlobalContext) -> Self {
        assert!(
            source_id.is_builtin(),
            "source `{source_id} is not a builtin"
        );
        let path_source = PathSource::new(path, source_id, gctx);
        Self {
            source_id,
            path_source,
            opaque_summary: RefCell::new(None),
        }
    }

    pub fn preload_with(pkg: Package, gctx: &'gctx GlobalContext) -> CargoResult<Self> {
        assert!(pkg.package_id().source_id().is_builtin());
        let summary = pkg.summary().clone();
        let source_id = summary.source_id();
        let inner = PathSource::preload_with(pkg, gctx);
        Ok(Self {
            source_id,
            path_source: inner,
            opaque_summary: RefCell::new(Some(summary)),
        })
    }

    fn load(&self) -> CargoResult<()> {
        let mut summary = self.opaque_summary.borrow_mut();
        if summary.is_none() {
            let p = self.path_source.root_package()?;
            *summary = Some(p.summary().clone().to_opaque_builtin_summary()?);
        }
        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl<'gctx> Source for BuiltinSource<'gctx> {
    /// All builtin dependencies are opaque, so this will return a summary without any dependencies when queried
    async fn query(
        &self,
        dep: &Dependency,
        kind: QueryKind,
        f: &mut dyn FnMut(IndexSummary),
    ) -> CargoResult<()> {
        self.load()?;
        if let Some(s) = self.opaque_summary.borrow().as_ref() {
            let matched = match kind {
                QueryKind::Exact | QueryKind::RejectedVersions => dep.matches(s),
                QueryKind::AlternativeNames => true,
                QueryKind::Normalized => dep.matches(s),
            };
            if matched {
                f(IndexSummary::Candidate(s.clone()));
            }
        }
        Ok(())
    }

    fn supports_checksums(&self) -> bool {
        self.path_source.supports_checksums()
    }

    fn requires_precise(&self) -> bool {
        self.path_source.requires_precise()
    }

    fn source_id(&self) -> SourceId {
        self.source_id
    }

    async fn download(&self, id: PackageId) -> CargoResult<MaybePackage> {
        self.path_source.download(id).await
    }

    async fn finish_download(&self, id: PackageId, data: Vec<u8>) -> CargoResult<Package> {
        self.path_source.finish_download(id, data).await
    }

    fn fingerprint(&self, pkg: &Package) -> CargoResult<String> {
        self.path_source.fingerprint(pkg)
    }

    fn describe(&self) -> String {
        self.source_id.to_string()
    }

    fn add_to_yanked_whitelist(&self, _pkgs: &[PackageId]) {
        // Builtin source cannot be yanked
    }

    async fn is_yanked(&self, _pkg: PackageId) -> CargoResult<bool> {
        Ok(false)
    }

    fn invalidate_cache(&self) {
        // Builtin source has no local cache.
    }

    fn set_quiet(&mut self, _quiet: bool) {
        // Builtin source does not display status
    }
}
