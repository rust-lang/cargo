use std::task::ready;

use tracing::debug;

use crate::sources::IndexSummary;

use super::source::{MaybePackage, Source};

/// A `Source` that overlays one source over another, pretending that the packages
/// available in the overlay are actually available in the other one.
///
/// This is a massive footgun and a terrible idea, so we do not (and never will)
/// expose this publicly. However, it is useful for some very specific private
/// things, like locally verifying a bunch of packages at a time before any of
/// them have been published.
pub struct DependencyConfusionThreatOverlaySource<'gctx> {
    // The overlay source. The naming here comes from the main application of this,
    // where there is a remote registry that we overlay some local packages on.
    local: Box<dyn Source + 'gctx>,
    // The source we're impersonating.
    remote: Box<dyn Source + 'gctx>,
}

impl<'gctx> DependencyConfusionThreatOverlaySource<'gctx> {
    pub fn new(local: Box<dyn Source + 'gctx>, remote: Box<dyn Source + 'gctx>) -> Self {
        debug!(
            "overlaying {} on {}",
            local.source_id().as_url(),
            remote.source_id().as_url()
        );
        Self { local, remote }
    }
}

impl<'gctx> Source for DependencyConfusionThreatOverlaySource<'gctx> {
    fn source_id(&self) -> crate::core::SourceId {
        self.remote.source_id()
    }

    fn supports_checksums(&self) -> bool {
        self.local.supports_checksums() && self.remote.supports_checksums()
    }

    fn requires_precise(&self) -> bool {
        self.local.requires_precise() || self.remote.requires_precise()
    }

    fn query(
        &mut self,
        dep: &crate::core::Dependency,
        kind: super::source::QueryKind,
        f: &mut dyn FnMut(super::IndexSummary),
    ) -> std::task::Poll<crate::CargoResult<()>> {
        let local_source = self.local.source_id();
        let remote_source = self.remote.source_id();

        let local_dep = dep.clone().map_source(remote_source, local_source);
        let mut local_packages = std::collections::HashSet::new();
        let mut local_callback = |index: IndexSummary| {
            let index = index.map_summary(|s| s.map_source(local_source, remote_source));
            local_packages.insert(index.as_summary().clone());
            f(index)
        };
        ready!(self.local.query(&local_dep, kind, &mut local_callback))?;

        let mut remote_callback = |index: IndexSummary| {
            if local_packages.contains(index.as_summary()) {
                tracing::debug!(?local_source, ?remote_source, ?index, "package collision");
            } else {
                f(index)
            }
        };
        ready!(self.remote.query(dep, kind, &mut remote_callback))?;

        std::task::Poll::Ready(Ok(()))
    }

    fn invalidate_cache(&mut self) {
        self.local.invalidate_cache();
        self.remote.invalidate_cache();
    }

    fn set_quiet(&mut self, quiet: bool) {
        self.local.set_quiet(quiet);
        self.remote.set_quiet(quiet);
    }

    fn download(
        &mut self,
        package: crate::core::PackageId,
    ) -> crate::CargoResult<super::source::MaybePackage> {
        let local_source = self.local.source_id();
        let remote_source = self.remote.source_id();

        self.local
            .download(package.map_source(remote_source, local_source))
            .map(|maybe_pkg| match maybe_pkg {
                MaybePackage::Ready(pkg) => {
                    MaybePackage::Ready(pkg.map_source(local_source, remote_source))
                }
                x => x,
            })
            .or_else(|_| self.remote.download(package))
    }

    fn finish_download(
        &mut self,
        pkg_id: crate::core::PackageId,
        contents: Vec<u8>,
    ) -> crate::CargoResult<crate::core::Package> {
        // The local registry should never return MaybePackage::Download from `download`, so any
        // downloads that need to be finished come from the remote registry.
        self.remote.finish_download(pkg_id, contents)
    }

    fn fingerprint(&self, pkg: &crate::core::Package) -> crate::CargoResult<String> {
        Ok(pkg.package_id().version().to_string())
    }

    fn describe(&self) -> String {
        self.remote.describe()
    }

    fn add_to_yanked_whitelist(&mut self, pkgs: &[crate::core::PackageId]) {
        self.local.add_to_yanked_whitelist(pkgs);
        self.remote.add_to_yanked_whitelist(pkgs);
    }

    fn is_yanked(
        &mut self,
        pkg: crate::core::PackageId,
    ) -> std::task::Poll<crate::CargoResult<bool>> {
        self.remote.is_yanked(pkg)
    }

    fn block_until_ready(&mut self) -> crate::CargoResult<()> {
        self.local.block_until_ready()?;
        self.remote.block_until_ready()
    }
}
