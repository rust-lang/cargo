//! Differential tests for the conservative-update paths of the pubgrub
//! resolver.
//!
//! Building against an existing `Cargo.lock`, `cargo update -p <crate>`, and
//! `--precise` all reach the resolver as [`VersionPreferences`]: a set of
//! previously selected packages (or exact dependencies) that resolution should
//! reuse when still valid. The production glue in `ops::resolve` constructs
//! those preferences; here we construct them directly with [`prefs_from_lock`]
//! and a hand-built [`VersionPreferences`], which is the faithful
//! resolver-level slice of those flows.
//!
//! Each test follows the same shape:
//!
//! 1. resolve a manifest fresh,
//! 2. derive preferences from that first resolution (the "lock"),
//! 3. mutate the registry and/or the root manifest (publish a new version, free
//!    one crate, add a dependency, pin an exact version),
//! 4. re-resolve *with those preferences* under both the default resolver and
//!    the pubgrub resolver,
//! 5. assert the two resolvers agree on the full graph (nodes **and** edges).
//!
//! The default resolver is the oracle: these tests pin down that pubgrub honors
//! preferences the same way Cargo already does, rather than asserting a
//! particular hand-computed lock (which would re-encode the very logic under
//! test).

use std::collections::BTreeSet;

use cargo::core::Resolve;
use cargo::core::resolver::VersionPreferences;
use cargo::core::{Dependency, Summary};
use cargo::util::GlobalContext;

use resolver_tests::{
    helpers::{dep, dep_req, pkg, pkg_dep, pkg_id, registry},
    prefs_from_lock, resolve_with_prefs_raw,
};

fn default_gctx() -> GlobalContext {
    GlobalContext::default().unwrap()
}

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

/// `name/version` for every resolved package.
fn nodes(r: &Resolve) -> BTreeSet<String> {
    r.iter()
        .map(|p| format!("{}/{}", p.name(), p.version()))
        .collect()
}

/// `parent -> child` for every resolved edge.
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

/// Resolve `deps` against `reg` with the default resolver and no preferences;
/// this stands in for a fresh `generate-lockfile`.
#[track_caller]
fn lock(deps: Vec<Dependency>, reg: &[Summary]) -> Resolve {
    resolve_with_prefs_raw(
        deps,
        reg,
        pkg_id("root"),
        &default_gctx(),
        VersionPreferences::default(),
    )
    .expect("initial resolution should succeed")
}

/// Re-resolve `deps` against `reg` with `prefs` under both resolvers and assert
/// the resulting graphs are identical.
#[track_caller]
fn assert_same_update(deps: Vec<Dependency>, reg: &[Summary], prefs: impl Fn() -> VersionPreferences) {
    let default = resolve_with_prefs_raw(
        deps.clone(),
        reg,
        pkg_id("root"),
        &default_gctx(),
        prefs(),
    );
    let pubgrub = resolve_with_prefs_raw(deps, reg, pkg_id("root"), &pubgrub_gctx(), prefs());
    match (default, pubgrub) {
        (Ok(d), Ok(p)) => {
            assert_eq!(
                nodes(&d),
                nodes(&p),
                "node set mismatch:\n  default: {:?}\n  pubgrub: {:?}",
                nodes(&d),
                nodes(&p),
            );
            assert_eq!(
                edges(&d),
                edges(&p),
                "edge set mismatch:\n  default: {:?}\n  pubgrub: {:?}",
                edges(&d),
                edges(&p),
            );
        }
        (Err(_), Err(_)) => {}
        (d, p) => panic!(
            "resolvers disagree on solvability: default={} pubgrub={}",
            d.is_ok(),
            p.is_ok()
        ),
    }
}

/// Building against an untouched lock keeps the previously selected version
/// even after a newer one is published.
#[test]
fn keeps_locked_version_when_newer_published() {
    // Lock against an index that only has foo 1.0.0.
    let old_reg = registry(vec![
        pkg(("foo", "1.0.0")),
    ]);
    let locked = lock(vec![dep_req("foo", "^1")], &old_reg);

    // foo 1.1.0 is now published, but the lock prefers 1.0.0.
    let new_reg = registry(vec![
        pkg(("foo", "1.0.0")),
        pkg(("foo", "1.1.0")),
    ]);
    assert_same_update(vec![dep_req("foo", "^1")], &new_reg, || {
        prefs_from_lock(&locked, &[])
    });
}

/// `cargo update -p foo` frees `foo` (only) to advance to the newest
/// compatible version while everything else stays put.
#[test]
fn update_single_package_advances_only_it() {
    let old_reg = registry(vec![
        pkg(("foo", "1.0.0")),
        pkg(("bar", "1.0.0")),
    ]);
    let locked = lock(vec![dep_req("foo", "^1"), dep_req("bar", "^1")], &old_reg);

    // Both foo and bar have newer releases now; updating only foo should move
    // foo to 1.1.0 while bar stays at 1.0.0.
    let new_reg = registry(vec![
        pkg(("foo", "1.0.0")),
        pkg(("foo", "1.1.0")),
        pkg(("bar", "1.0.0")),
        pkg(("bar", "1.1.0")),
    ]);
    assert_same_update(
        vec![dep_req("foo", "^1"), dep_req("bar", "^1")],
        &new_reg,
        || prefs_from_lock(&locked, &["foo"]),
    );
}

/// Adding a brand-new dependency to the manifest leaves the locked packages
/// untouched and only selects the new crate (and its subtree).
#[test]
fn adding_a_dependency_keeps_the_rest_locked() {
    let old_reg = registry(vec![
        pkg(("foo", "1.0.0")),
        pkg(("foo", "1.1.0")),
    ]);
    let locked = lock(vec![dep_req("foo", "^1")], &old_reg);

    // Now `bar` is added to the manifest (and bar depends on foo too). foo must
    // stay at its locked version even though bar would otherwise pull the newest.
    let new_reg = registry(vec![
        pkg(("foo", "1.0.0")),
        pkg(("foo", "1.1.0")),
        pkg_dep(("bar", "1.0.0"), vec![dep_req("foo", "^1")]),
    ]);
    assert_same_update(
        vec![dep_req("foo", "^1"), dep_req("bar", "^1")],
        &new_reg,
        || prefs_from_lock(&locked, &[]),
    );
}

/// A locked transitive dependency shared by two parents stays pinned across a
/// re-resolve when a newer compatible version appears.
#[test]
fn shared_transitive_stays_locked() {
    let old_reg = registry(vec![
        pkg(("baz", "1.0.0")),
        pkg_dep(("foo", "1.0.0"), vec![dep_req("baz", "^1")]),
        pkg_dep(("bar", "1.0.0"), vec![dep_req("baz", "^1")]),
    ]);
    let locked = lock(vec![dep_req("foo", "^1"), dep_req("bar", "^1")], &old_reg);

    let new_reg = registry(vec![
        pkg(("baz", "1.0.0")),
        pkg(("baz", "1.1.0")),
        pkg_dep(("foo", "1.0.0"), vec![dep_req("baz", "^1")]),
        pkg_dep(("bar", "1.0.0"), vec![dep_req("baz", "^1")]),
    ]);
    assert_same_update(
        vec![dep_req("foo", "^1"), dep_req("bar", "^1")],
        &new_reg,
        || prefs_from_lock(&locked, &[]),
    );
}

/// `--precise` pins an exact version via a preferred dependency; resolution
/// must select it even when a newer one is available.
#[test]
fn precise_pins_exact_version() {
    let reg = registry(vec![
        pkg(("foo", "1.0.0")),
        pkg(("foo", "1.1.0")),
        pkg(("foo", "1.2.0")),
    ]);
    // `cargo update foo --precise 1.1.0` is modeled as preferring `foo =1.1.0`.
    assert_same_update(vec![dep_req("foo", "^1")], &reg, || {
        let mut prefs = VersionPreferences::default();
        prefs.prefer_dependency(dep_req("foo", "=1.1.0"));
        prefs
    });
}

/// Unlocking everything (`cargo update` with no `-p`) lets all packages move to
/// the newest compatible versions.
#[test]
fn full_update_advances_everything() {
    let old_reg = registry(vec![
        pkg(("foo", "1.0.0")),
        pkg(("bar", "1.0.0")),
    ]);
    let locked = lock(vec![dep_req("foo", "^1"), dep_req("bar", "^1")], &old_reg);

    let new_reg = registry(vec![
        pkg(("foo", "1.0.0")),
        pkg(("foo", "1.1.0")),
        pkg(("bar", "1.0.0")),
        pkg(("bar", "1.1.0")),
    ]);
    // Free both crates: prefs become empty, so this is a fresh resolution.
    assert_same_update(
        vec![dep_req("foo", "^1"), dep_req("bar", "^1")],
        &new_reg,
        || prefs_from_lock(&locked, &["foo", "bar"]),
    );
}

/// A locked version that is no longer valid (the manifest constraint moved to a
/// new major) must be dropped in favor of a compatible one, despite the
/// preference.
#[test]
fn stale_lock_is_overridden_by_constraint() {
    let old_reg = registry(vec![
        pkg(("foo", "1.0.0")),
    ]);
    let locked = lock(vec![dep_req("foo", "^1")], &old_reg);

    // The manifest now requires foo ^2; the locked 1.0.0 cannot satisfy it.
    let new_reg = registry(vec![
        pkg(("foo", "1.0.0")),
        pkg(("foo", "2.0.0")),
    ]);
    assert_same_update(vec![dep_req("foo", "^2")], &new_reg, || {
        prefs_from_lock(&locked, &[])
    });
}

/// Sanity check that `dep` (wildcard) and feature-free graphs round-trip the
/// preference path too, not just exact requirements.
#[test]
fn wildcard_dep_keeps_locked_version() {
    let old_reg = registry(vec![
        pkg(("foo", "1.0.0")),
    ]);
    let locked = lock(vec![dep("foo")], &old_reg);

    let new_reg = registry(vec![
        pkg(("foo", "1.0.0")),
        pkg(("foo", "1.1.0")),
        pkg(("foo", "2.0.0")),
    ]);
    assert_same_update(vec![dep("foo")], &new_reg, || prefs_from_lock(&locked, &[]));
}
