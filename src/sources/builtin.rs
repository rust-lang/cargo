use std::{cell::RefCell, path::Path};

use crate::{
    CargoResult, GlobalContext,
    sources::{
        IndexSummary, RecursivePathSource,
        source::{MaybePackage, QueryKind, Source},
    },
    util::data_structures::HashMap,
    workspace::{Dependency, Package, PackageId, SourceId, Summary},
};

/// A builtin source represents standard library packages used in build-std, which are "built into"
/// the toolchain. Returns opaque `Summary`s - see [`Summary::new_opaque()`]
///
/// It wraps a [`RecursivePathSource`] and uses that to discover packages
pub struct BuiltinSource<'gctx> {
    /// The unique identifier for this source
    source_id: SourceId,
    /// The underlying path source which discovers packages
    path_source: RecursivePathSource<'gctx>,
    /// Opaque summaries cached by the real package ID returned by the path source.
    opaque_summaries: RefCell<HashMap<PackageId, Summary>>,
}

impl<'gctx> BuiltinSource<'gctx> {
    pub fn new(path: &Path, source_id: SourceId, gctx: &'gctx GlobalContext) -> Self {
        assert!(
            source_id.is_builtin(),
            "source `{source_id} is not a builtin"
        );
        let path_source = RecursivePathSource::new(&path, source_id, gctx);
        Self {
            source_id,
            path_source,
            opaque_summaries: RefCell::new(HashMap::default()),
        }
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
        if !dep.source_id().is_builtin() {
            // Avoid loading packages in the path source if it's not needed
            return Ok(());
        }
        self.path_source
            .query(dep, kind, &mut |summary| {
                let summary = match summary {
                    IndexSummary::Candidate(summary) => {
                        let package_id = summary.package_id();
                        let opaque = self
                            .opaque_summaries
                            .borrow_mut()
                            .entry(package_id)
                            .or_insert_with(|| Summary::new_opaque(package_id, self.source_id))
                            .clone();
                        IndexSummary::Candidate(opaque)
                    }
                    summary => summary,
                };
                f(summary);
            })
            .await
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

    fn invalidate_cache(&self) {
        // The RecursivePathSource does not clear its cached Packages, meaning nothing can
        // invalidate our cached summaries
    }

    fn set_quiet(&mut self, quiet: bool) {
        self.path_source.set_quiet(quiet);
    }
}

#[cfg(test)]
mod test {
    use std::path::Path;

    use crate::{GlobalContext, sources::IndexSummary, workspace::Dependency};

    #[test]
    fn builtin_source() {
        let gctx = GlobalContext::default().unwrap();
        let mock_std_root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/testsuite/mock-std/library");

        let dep = Dependency::new_implicit_builtin("core".into(), &mock_std_root).unwrap();
        assert!(dep.is_opaque());

        let source = dep.source_id().load(&gctx).unwrap();
        let results =
            crate::util::block_on(source.query_vec(&dep, crate::sources::source::QueryKind::Exact))
                .unwrap();

        assert_eq!(results.len(), 1);
        let result = results[0].clone();
        if let IndexSummary::Candidate(s) = result {
            assert!(dep.matches(&s));
            assert!(s.dependencies().is_empty());
        }
    }
}
