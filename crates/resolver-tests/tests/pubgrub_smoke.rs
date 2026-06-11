use cargo::util::GlobalContext;

use resolver_tests::{
    helpers::{dep, dep_req, pkg, pkg_dep, registry},
    resolve_with_global_context,
};

fn pubgrub_gctx() -> GlobalContext {
    let mut gctx = GlobalContext::default().unwrap();
    gctx.nightly_features_allowed = true;
    gctx.configure(
        0,
        false,
        None,
        false,
        false,
        false,
        &None,
        &["pubgrub-resolver".to_string()],
        &[],
    )
    .unwrap();
    gctx
}

#[test]
fn smoke_single() {
    let reg = registry(vec![pkg(("a", "1.0.0"))]);
    let gctx = pubgrub_gctx();
    let res = resolve_with_global_context(vec![dep("a")], &reg, &gctx);
    eprintln!("RESULT: {res:?}");
    assert!(res.is_ok(), "{res:?}");
}

#[test]
fn smoke_transitive() {
    let reg = registry(vec![
        pkg_dep(("a", "1.0.0"), vec![dep_req("b", "^1.0")]),
        pkg(("b", "1.2.0")),
        pkg(("b", "1.0.0")),
    ]);
    let gctx = pubgrub_gctx();
    let res = resolve_with_global_context(vec![dep("a")], &reg, &gctx);
    eprintln!("RESULT: {res:?}");
    assert!(res.is_ok(), "{res:?}");
}
