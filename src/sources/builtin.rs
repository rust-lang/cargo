use crate::{
    CargoResult, GlobalContext,
    compiler::{CompileKind, RustcTargetData, standard_lib::detect_sysroot_src_path},
    sources::{
        IndexSummary, RecursivePathSource,
        source::{MaybePackage, QueryKind, Source},
    },
    workspace::{Dependency, Package, PackageId, SourceId},
};

/// A builtin source represents standard library packages used in build-std, which are "built into"
/// the toolchain.
///
/// It's a thin wrapper around a [`RecursivePathSource`] located in the sysroot's library source
/// path.
pub struct BuiltinSource<'gctx> {
    /// The unique identifier for this source
    source_id: SourceId,
    /// The underlying path source which discovers packages
    path_source: RecursivePathSource<'gctx>,
}

impl<'gctx> BuiltinSource<'gctx> {
    pub fn new(source_id: SourceId, gctx: &'gctx GlobalContext) -> CargoResult<Self> {
        let target_data = RustcTargetData::new(gctx, None, &[CompileKind::Host])?;
        let path = detect_sysroot_src_path(&target_data)?;
        let path_source = RecursivePathSource::new(&path, source_id, gctx);
        Ok(Self {
            source_id,
            path_source,
        })
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
        self.path_source.query(dep, kind, f).await
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
        self.path_source.invalidate_cache();
    }

    fn set_quiet(&mut self, quiet: bool) {
        self.path_source.set_quiet(quiet);
    }
}

#[cfg(test)]
mod test {
    use std::path::Path;

    use crate::{
        GlobalContext,
        sources::IndexSummary,
        util::{VersionReqMatchMode, data_structures::HashMap},
        workspace::{Dependency, SourceId},
    };

    #[test]
    fn builtin_source() {
        let mut gctx = GlobalContext::default().unwrap();
        let mock_std_root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/testsuite/mock-std/library");
        let env = HashMap::from_iter([(
            "__CARGO_TESTS_ONLY_SRC_ROOT".to_string(),
            mock_std_root.display().to_string(),
        )]);
        gctx.set_env(env);

        let source_id = SourceId::for_builtin().unwrap();
        let dep = Dependency::new_override("core".into(), source_id);

        let source = dep.source_id().load(&gctx).unwrap();
        let results =
            crate::util::block_on(source.query_vec(&dep, crate::sources::source::QueryKind::Exact))
                .unwrap();

        assert_eq!(results.len(), 1);
        let result = results[0].clone();
        if let IndexSummary::Candidate(s) = result {
            assert!(dep.matches(&s, VersionReqMatchMode::Default));
            assert!(s.dependencies().is_empty());
            assert!(s.source_id().is_builtin())
        } else {
            panic!("no candidate found");
        };
    }
}
