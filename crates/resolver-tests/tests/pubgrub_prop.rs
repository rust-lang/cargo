use std::io::IsTerminal;

use cargo::util::GlobalContext;
use cargo_util::is_ci;

use cargo::core::Resolve;
use cargo::core::resolver::VersionPreferences;
use cargo::core::PackageId;
use cargo::util::interning::InternedString;

use resolver_tests::{
    PrettyPrintRegistry,
    helpers::{dep_req, pkg_id, registry},
    prefs_from_lock, registry_strategy, resolve_with_global_context, resolve_with_prefs_raw,
    sat::SatResolver,
};

use proptest::prelude::*;

/// Project a [`Resolve`] into the `(PackageId, features)` shape the SAT
/// reference resolver validates.
fn collect_features(resolve: &Resolve) -> Vec<(PackageId, Vec<InternedString>)> {
    resolve
        .sort()
        .iter()
        .map(|&pkg| (pkg, resolve.features(pkg).to_vec()))
        .collect()
}

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

proptest! {
    #![proptest_config(ProptestConfig {
        max_shrink_iters:
            if is_ci() || !std::io::stderr().is_terminal() {
                0
            } else {
                u32::MAX
            },
        result_cache: prop::test_runner::basic_result_cache,
        .. ProptestConfig::default()
    })]

    /// The pubgrub resolver must agree with the SAT reference resolver: every
    /// solution it produces is valid, and it only fails when there is no
    /// solution.
    ///
    /// NOTE: this is a form of fuzz testing; a failure indicates a real
    /// problem, but passing does not prove correctness.
    #[test]
    fn prop_pubgrub_passes_validation(
        PrettyPrintRegistry(input) in registry_strategy(50, 20, 60)
    ) {
        let reg = registry(input.clone());
        let gctx = pubgrub_gctx();
        let mut sat = SatResolver::new(&reg);
        for this in input.iter().rev().take(20) {
            let deps = vec![dep_req(&this.name(), &format!("={}", this.version()))];
            match resolve_with_global_context(deps.clone(), &reg, &gctx) {
                Ok(out) => prop_assert!(
                    sat.sat_is_valid_solution(&out),
                    "pubgrub solution rejected by SAT for `{}={}`:\n{:?}\n{:?}",
                    this.name(), this.version(), out, PrettyPrintRegistry(input.clone()),
                ),
                Err(_) => prop_assert!(
                    !sat.sat_resolve(&deps),
                    "pubgrub failed but SAT says solvable for `{}={}`\n{:?}",
                    this.name(), this.version(), PrettyPrintRegistry(input.clone()),
                ),
            }
        }
    }

    /// The conservative-update paths (building against a lock, `cargo update
    /// -p`) must stay correct: after resolving once and feeding the result back
    /// as version preferences, a second pubgrub resolution must still produce a
    /// SAT-valid solution.
    ///
    /// Preferences only reorder the candidates pubgrub considers; they must
    /// never let it accept an invalid solution nor fail when one exists. We
    /// cross-check the locked re-resolution against the default resolver run
    /// with the same preferences so the two resolvers agree on solvability.
    #[test]
    fn prop_pubgrub_locked_reresolve_passes_validation(
        PrettyPrintRegistry(input) in registry_strategy(50, 20, 60)
    ) {
        let reg = registry(input.clone());
        let gctx = pubgrub_gctx();
        let default_gctx = GlobalContext::default().unwrap();
        let mut sat = SatResolver::new(&reg);

        for this in input.iter().rev().take(20) {
            let deps = vec![dep_req(&this.name(), &format!("={}", this.version()))];

            // First pass: a fresh pubgrub resolution acts as the "lock".
            let Ok(locked) = resolve_with_prefs_raw(
                deps.clone(), &reg, pkg_id("root"), &gctx, VersionPreferences::default(),
            ) else {
                continue;
            };

            // Re-resolve with everything preferred (building against the lock)
            // and with the requested dependency freed (`cargo update -p <dep>`).
            let unlock_dep = this.name();
            for unlock in [Vec::new(), vec![unlock_dep.as_str()]] {
                let prefs = || prefs_from_lock(&locked, &unlock);

                let pubgrub = resolve_with_prefs_raw(
                    deps.clone(), &reg, pkg_id("root"), &gctx, prefs(),
                );
                let default = resolve_with_prefs_raw(
                    deps.clone(), &reg, pkg_id("root"), &default_gctx, prefs(),
                );

                match pubgrub {
                    Ok(out) => {
                        prop_assert!(
                            sat.sat_is_valid_solution(&collect_features(&out)),
                            "locked pubgrub solution rejected by SAT for `{}={}` (unlock={:?}):\n{:?}",
                            this.name(), this.version(), unlock, PrettyPrintRegistry(input.clone()),
                        );
                        prop_assert!(
                            default.is_ok(),
                            "locked pubgrub resolved but default failed for `{}={}` (unlock={:?})\n{:?}",
                            this.name(), this.version(), unlock, PrettyPrintRegistry(input.clone()),
                        );
                    }
                    Err(_) => prop_assert!(
                        default.is_err(),
                        "locked pubgrub failed but default resolved for `{}={}` (unlock={:?})\n{:?}",
                        this.name(), this.version(), unlock, PrettyPrintRegistry(input.clone()),
                    ),
                }
            }
        }
    }
}
