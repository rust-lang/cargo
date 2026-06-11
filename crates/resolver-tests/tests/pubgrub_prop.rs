use std::io::IsTerminal;

use cargo::util::GlobalContext;
use cargo_util::is_ci;

use resolver_tests::{
    PrettyPrintRegistry,
    helpers::{dep_req, registry},
    registry_strategy, resolve_with_global_context,
    sat::SatResolver,
};

use proptest::prelude::*;

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
}
