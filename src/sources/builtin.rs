use std::cell::RefCell;
use std::collections::hash_map::Entry;

use semver::Version;

use crate::{
    CargoResult,
    sources::{
        IndexSummary,
        source::{MaybePackage, QueryKind, Source},
    },
    util::{VersionReqMatchMode, internal},
    workspace::{Dependency, Package, PackageId, SourceId},
};
use cargo_util::collections::HashMap;

/// A builtin source represents standard library packages used in build-std, which are "built into"
/// the toolchain.
///
/// It's a thin wrapper around a [`RecursivePathSource`] located in the sysroot's library source
/// path.
pub struct BuiltinSource {
    /// The unique identifier for this source
    source_id: SourceId,
    /// Cache of packages returned by this source
    packages: RefCell<HashMap<PackageId, Package>>,
}

impl<'gctx> BuiltinSource {
    pub fn new(source_id: SourceId) -> CargoResult<Self> {
        Ok(Self {
            source_id,
            packages: Default::default(),
        })
    }

    fn get_package(&self, pkg_id: PackageId) -> CargoResult<Package> {
        let mut packages = self.packages.borrow_mut();
        let pkg = match packages.entry(pkg_id) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(Package::synthetic(pkg_id)?),
        };
        Ok(pkg.clone())
    }
}

#[async_trait::async_trait(?Send)]
impl Source for BuiltinSource {
    /// All builtin dependencies are opaque, so this will return a summary without any dependencies when queried
    async fn query(
        &self,
        dep: &Dependency,
        _kind: QueryKind,
        f: &mut dyn FnMut(IndexSummary),
    ) -> CargoResult<()> {
        if !dep.source_id().is_builtin() {
            // Not our job
            return Ok(());
        }
        let id = PackageId::new(dep.package_name(), Version::new(0, 0, 0), dep.source_id());
        let pkg = self.get_package(id)?;
        let summary = pkg.summary();

        if dep.matches(summary, VersionReqMatchMode::Default) {
            f(IndexSummary::Candidate(summary.clone()))
        }
        Ok(())
    }

    fn supports_checksums(&self) -> bool {
        false
    }

    fn requires_precise(&self) -> bool {
        false
    }

    fn source_id(&self) -> SourceId {
        self.source_id
    }

    async fn download(&self, id: PackageId) -> CargoResult<MaybePackage> {
        if !id.source_id().is_builtin() {
            return Err(internal(format!("failed to find {} in builtin source", id)));
        }
        Ok(MaybePackage::Ready(self.get_package(id)?))
    }

    async fn finish_download(&self, _id: PackageId, _data: Vec<u8>) -> CargoResult<Package> {
        panic!("no download should have started")
    }

    fn fingerprint(&self, _pkg: &Package) -> CargoResult<String> {
        Err(internal(format!("no fingerprint")))
    }

    fn describe(&self) -> String {
        self.source_id.to_string()
    }

    fn invalidate_cache(&self) {
        // No local cache
    }

    fn set_quiet(&mut self, _quiet: bool) {
        // Source does not display status
    }
}
