//! Validates the `-Zpubgrub-resolver` resolver against the SAT reference
//! resolver over a spread of resolution scenarios.

use cargo::core::Dependency;
use cargo::util::GlobalContext;

use resolver_tests::{
    helpers::{
        ToDep, dep, dep_req, pkg, pkg_dep, pkg_dep_link, pkg_dep_with, registry,
    },
    pkg, resolve_with_global_context,
    sat::SatResolver,
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

/// Resolve `deps` against `reg` with pubgrub and check the result agrees with
/// the SAT reference resolver.
#[track_caller]
fn check(deps: Vec<Dependency>, reg: &[cargo::core::Summary]) {
    let gctx = pubgrub_gctx();
    let mut sat = SatResolver::new(reg);
    match resolve_with_global_context(deps.clone(), reg, &gctx) {
        Ok(out) => assert!(
            sat.sat_is_valid_solution(&out),
            "pubgrub produced a solution the SAT resolver rejects:\n{out:?}",
        ),
        Err(e) => assert!(
            !sat.sat_resolve(&deps),
            "pubgrub failed but SAT says it is solvable:\n{e:?}\n{}",
            sat.used_packages().unwrap_or_default(),
        ),
    }
}

#[test]
fn transitive() {
    let reg = registry(vec![
        pkg_dep(("a", "1.0.0"), vec![dep_req("b", "^1.0")]),
        pkg(("b", "1.2.0")),
        pkg(("b", "1.0.0")),
    ]);
    check(vec![dep("a")], &reg);
}

#[test]
fn incompatible_majors_coexist() {
    let reg = registry(vec![
        pkg_dep(("a", "1.0.0"), vec![dep_req("b", "^1.0")]),
        pkg_dep(("c", "1.0.0"), vec![dep_req("b", "^2.0")]),
        pkg(("b", "1.0.0")),
        pkg(("b", "2.0.0")),
    ]);
    check(vec![dep("a"), dep("c")], &reg);
}

#[test]
fn pick_highest_compatible() {
    let reg = registry(vec![
        pkg(("a", "1.0.0")),
        pkg(("a", "1.1.0")),
        pkg(("a", "1.2.0")),
        pkg(("a", "2.0.0")),
    ]);
    check(vec![dep_req("a", "^1.0")], &reg);
}

#[test]
fn named_feature() {
    let reg = registry(vec![
        pkg(("b", "1.0.0")),
        pkg_dep_with("a", vec!["b".opt()], &[("f", &["b"])]),
    ]);
    check(vec!["a".with(&["f"])], &reg);
}

#[test]
fn default_feature() {
    let reg = registry(vec![
        pkg(("b", "1.0.0")),
        pkg_dep_with("a", vec!["b".opt()], &[("default", &["b"])]),
    ]);
    check(vec![dep("a")], &reg);
}

#[test]
fn dep_colon_feature() {
    let reg = registry(vec![
        pkg(("b", "1.0.0")),
        pkg_dep_with("a", vec!["b".opt()], &[("f", &["dep:b"])]),
    ]);
    check(vec!["a".with(&["f"])], &reg);
}

#[test]
fn dep_slash_feature() {
    let reg = registry(vec![
        pkg_dep_with("b", vec![], &[("inner", &[])]),
        pkg_dep_with("a", vec!["b".to_dep()], &[("f", &["b/inner"])]),
    ]);
    check(vec!["a".with(&["f"])], &reg);
}

#[test]
fn unselected_optional_dep() {
    let reg = registry(vec![
        pkg(("b", "1.0.0")),
        pkg_dep_with("a", vec!["b".opt()], &[("f", &["b"])]),
    ]);
    // `f` not enabled, so `b` must not be pulled in.
    check(vec![dep("a")], &reg);
}

#[test]
fn links_conflict_is_unsat() {
    let reg = registry(vec![
        pkg_dep_link("foo", "foo", vec![]),
        pkg_dep_link("bar", "foo", vec![]),
    ]);
    check(vec![dep("foo"), dep("bar")], &reg);
}

#[test]
fn missing_dependency_is_unsat() {
    let reg = registry(vec![pkg_dep(("a", "1.0.0"), vec![dep_req("b", "^2.0")]), pkg(("b", "1.0.0"))]);
    check(vec![dep("a")], &reg);
}

#[test]
fn diamond() {
    let reg = registry(vec![
        pkg_dep(("a", "1.0.0"), vec![dep("b"), dep("c")]),
        pkg_dep(("b", "1.0.0"), vec![dep_req("d", "^1.0")]),
        pkg_dep(("c", "1.0.0"), vec![dep_req("d", "^1.0")]),
        pkg(("d", "1.0.0")),
        pkg(("d", "1.5.0")),
    ]);
    check(vec![dep("a")], &reg);
}
