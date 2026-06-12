//! Compares the full resolved dependency *graph* (edges, not just the package
//! set) produced by the pubgrub resolver against the default resolver.

use std::collections::BTreeSet;

use cargo::core::Resolve;
use cargo::core::{Dependency, Summary};
use cargo::util::GlobalContext;

use resolver_tests::{
    helpers::{ToDep, dep, pkg, pkg_dep, pkg_dep_with, pkg_id, registry},
    resolve_with_global_context_raw,
};

fn pubgrub_gctx() -> GlobalContext {
    let mut gctx = GlobalContext::default().unwrap();
    gctx.nightly_features_allowed = true;
    gctx.configure(
        0, false, None, false, false, false, &None,
        &["pubgrub-resolver".to_string()], &[],
    )
    .unwrap();
    gctx
}

fn edges(r: &Resolve) -> BTreeSet<(String, String)> {
    let mut e = BTreeSet::new();
    for p in r.iter() {
        for (dp, _) in r.deps(p) {
            e.insert((
                format!("{}/{}", p.name(), p.version()),
                format!("{}/{}", dp.name(), dp.version()),
            ));
        }
    }
    e
}

#[track_caller]
fn assert_same_graph(deps: Vec<Dependency>, reg: &[Summary]) {
    let default = resolve_with_global_context_raw(
        deps.clone(),
        reg,
        pkg_id("root"),
        &GlobalContext::default().unwrap(),
    );
    let pubgrub = resolve_with_global_context_raw(
        deps,
        reg,
        pkg_id("root"),
        &pubgrub_gctx(),
    );
    match (default, pubgrub) {
        (Ok(d), Ok(p)) => {
            let de = edges(&d);
            let pe = edges(&p);
            let missing: Vec<_> = de.difference(&pe).collect();
            let extra: Vec<_> = pe.difference(&de).collect();
            assert!(
                missing.is_empty() && extra.is_empty(),
                "graph mismatch:\n  missing in pubgrub: {missing:?}\n  extra in pubgrub: {extra:?}",
            );
        }
        (Err(_), Err(_)) => {}
        (d, p) => panic!("resolvers disagree on solvability: default={:?} pubgrub={:?}", d.is_ok(), p.is_ok()),
    }
}

/// An optional dependency that IS activated (via `features = [..]` on the dep)
/// must appear as an edge.
#[test]
fn activated_optional_edge_present() {
    let reg = registry(vec![
        pkg(("serde", "1.0.0")),
        pkg_dep_with("bstr", vec!["serde".opt()], &[]),
        pkg_dep(("consumer", "1.0.0"), vec!["bstr".with(&["serde"])]),
    ]);
    assert_same_graph(vec![dep("consumer")], &reg);
}

/// A weak dependency feature (`dep?/feat`) on an enabled feature still records
/// the optional dependency as an edge in the lock graph (mirrors bstr's
/// `std = ["serde?/std"]`), matching Cargo's v1 lock resolver.
#[test]
fn weak_dep_feature_records_edge() {
    let reg = registry(vec![
        pkg_dep_with("serde", vec![], &[("std", &[])]),
        pkg_dep_with("bstr", vec!["serde".opt()], &[("std", &["serde?/std"])]),
        pkg_dep(("consumer", "1.0.0"), vec!["bstr".with(&["std"])]),
    ]);
    assert_same_graph(vec![dep("consumer")], &reg);
}

/// An optional dependency that is NOT activated must NOT create an edge, even
/// if its target is otherwise present in the lock (regression for the
/// schemars->url cycle).
#[test]
fn unactivated_optional_edge_absent() {
    let reg = registry(vec![
        // b optionally depends on a, but that optional dep is never enabled.
        pkg_dep_with("b", vec!["a".opt()], &[]),
        // a depends on b normally; a is also independently in the graph.
        pkg_dep(("a", "1.0.0"), vec![dep("b")]),
    ]);
    // Resolving `a` pulls in b; b's optional `a` is not enabled, so there must
    // be no b->a edge (which would be a cycle).
    assert_same_graph(vec![dep("a")], &reg);
}
