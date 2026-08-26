#[cfg(test)]
mod test {
    use std::path::Path;

    use crate::{GlobalContext, workspace::Dependency};

    #[test]
    fn builtin_source() {
        let gctx = GlobalContext::default().unwrap();
        let mock_std_root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/testsuite/mock-std/library");

        let dep = Dependency::new_implicit_builtin("core".into(), &mock_std_root).unwrap();
        assert!(dep.is_opaque());

        // No valid source SourceKind::Builtin
        //let source = dep.source_id().load(&gctx).unwrap();
    }
}
