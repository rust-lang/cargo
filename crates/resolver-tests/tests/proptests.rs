use std::io::IsTerminal;

use cargo::util::GlobalContext;
use cargo_util::is_ci;

use resolver_tests::{
    PrettyPrintRegistry,
    helpers::{dep_req, registry, remove_dep},
    registry_strategy, resolve, resolve_and_validated, resolve_with_global_context,
    sat::SatResolver,
};

use proptest::prelude::*;

// NOTE: proptest is a form of fuzz testing. It generates random input and makes sure that
// certain universal truths are upheld. Therefore, it can pass when there is a problem,
// but if it fails then there really is something wrong. When testing something as
// complicated as the resolver, the problems can be very subtle and hard to generate.
// We have had a history of these tests only failing on PRs long after a bug is introduced.
// If you have one of these test fail please report it on #6258,
// and if you did not change the resolver then feel free to retry without concern.
proptest! {
    #![proptest_config(ProptestConfig {
        max_shrink_iters:
            if is_ci() || !std::io::stderr().is_terminal() {
                // This attempts to make sure that CI will fail fast,
                0
            } else {
                // but that local builds will give a small clear test case.
                u32::MAX
            },
        result_cache: prop::test_runner::basic_result_cache,
        .. ProptestConfig::default()
    })]

    /// NOTE: if you think this test has failed spuriously see the note at the top of this macro.
    #[test]
    fn prop_passes_validation(
        PrettyPrintRegistry(input) in registry_strategy(50, 20, 60)
    )  {
        let reg = registry(input.clone());
        let mut sat_resolver = SatResolver::new(&reg);

        // There is only a small chance that a crate will be interesting.
        // So we try some of the most complicated.
        for this in input.iter().rev().take(20) {
            let _ = resolve_and_validated(
                vec![dep_req(&this.name(), &format!("={}", this.version()))],
                &reg,
                &mut sat_resolver,
            );
        }
    }

    /// NOTE: if you think this test has failed spuriously see the note at the top of this macro.
    #[test]
    fn prop_minimum_version_errors_the_same(
            PrettyPrintRegistry(input) in registry_strategy(50, 20, 60)
    ) {
        let mut gctx = GlobalContext::default().unwrap();
        gctx.nightly_features_allowed = true;
        gctx
            .configure(
                1,
                false,
                None,
                false,
                false,
                false,
                &None,
                &["minimal-versions".to_string()],
                &[],
            )
            .unwrap();

        let reg = registry(input.clone());

        // There is only a small chance that a crate will be interesting.
        // So we try some of the most complicated.
        for this in input.iter().rev().take(10) {
            let deps = vec![dep_req(&this.name(), &format!("={}", this.version()))];
            let res = resolve(deps.clone(), &reg);
            let mres = resolve_with_global_context(deps, &reg, &gctx);

            // `minimal-versions` changes what order the candidates are tried but not the existence of a solution.
            prop_assert_eq!(
                res.is_ok(),
                mres.is_ok(),
                "minimal-versions and regular resolver disagree about whether `{} = \"={}\"` can resolve",
                this.name(),
                this.version()
            )
        }
    }

    /// NOTE: if you think this test has failed spuriously see the note at the top of this macro.
    #[test]
    fn prop_direct_minimum_version_error_implications(
            PrettyPrintRegistry(input) in registry_strategy(50, 20, 60)
    ) {
        let mut gctx = GlobalContext::default().unwrap();
        gctx.nightly_features_allowed = true;
        gctx
            .configure(
                1,
                false,
                None,
                false,
                false,
                false,
                &None,
                &["direct-minimal-versions".to_string()],
                &[],
            )
            .unwrap();

        let reg = registry(input.clone());

        // There is only a small chance that a crate will be interesting.
        // So we try some of the most complicated.
        for this in input.iter().rev().take(10) {
            let deps = vec![dep_req(&this.name(), &format!("={}", this.version()))];
            let res = resolve(deps.clone(), &reg);
            let mres = resolve_with_global_context(deps, &reg, &gctx);

            // `direct-minimal-versions` reduces the number of available solutions,
            //  so we verify that we do not come up with solutions not seen in `maximal-versions`.
            if res.is_err() {
                prop_assert!(
                    mres.is_err(),
                    "direct-minimal-versions should not have more solutions than the regular, maximal resolver but found one when resolving `{} = \"={}\"`",
                    this.name(),
                    this.version()
                )
            }
            if mres.is_ok() {
                prop_assert!(
                    res.is_ok(),
                    "direct-minimal-versions should not have more solutions than the regular, maximal resolver but found one when resolving `{} = \"={}\"`",
                    this.name(),
                    this.version()
                )
            }
        }
    }

    /// NOTE: if you think this test has failed spuriously see the note at the top of this macro.
    #[test]
    fn prop_removing_a_dep_cant_break(
            PrettyPrintRegistry(input) in registry_strategy(50, 20, 60),
            indexes_to_remove in prop::collection::vec((any::<prop::sample::Index>(), any::<prop::sample::Index>()), ..10)
    ) {
        let reg = registry(input.clone());
        let mut removed_input = input.clone();
        for (summary_idx, dep_idx) in indexes_to_remove {
            if !removed_input.is_empty() {
                let summary_idx = summary_idx.index(removed_input.len());
                let deps = removed_input[summary_idx].dependencies();
                if !deps.is_empty() {
                    let new = remove_dep(&removed_input[summary_idx], dep_idx.index(deps.len()));
                    removed_input[summary_idx] = new;
                }
            }
        }
        let removed_reg = registry(removed_input);

        // There is only a small chance that a crate will be interesting.
        // So we try some of the most complicated.
        for this in input.iter().rev().take(10) {
            if resolve(
                vec![dep_req(&this.name(), &format!("={}", this.version()))],
                &reg,
            ).is_ok() {
                prop_assert!(
                    resolve(
                        vec![dep_req(&this.name(), &format!("={}", this.version()))],
                        &removed_reg,
                    ).is_ok(),
                    "full index worked for `{} = \"={}\"` but removing some deps broke it!",
                    this.name(),
                    this.version(),
                )
            }
        }
    }

    /// NOTE: if you think this test has failed spuriously see the note at the top of this macro.
    #[test]
    fn prop_limited_independence_of_irrelevant_alternatives(
        PrettyPrintRegistry(input) in registry_strategy(50, 20, 60),
        indexes_to_unpublish in prop::collection::vec(any::<prop::sample::Index>(), ..10)
    )  {
        let reg = registry(input.clone());

        // There is only a small chance that a crate will be interesting.
        // So we try some of the most complicated.
        for this in input.iter().rev().take(10) {
            let res = resolve(
                vec![dep_req(&this.name(), &format!("={}", this.version()))],
                &reg,
            );

            match res {
                Ok(r) => {
                    // If resolution was successful, then unpublishing a version of a crate
                    // that was not selected should not change that.
                    let not_selected: Vec<_> = input
                        .iter().filter(|&x| !r.contains(&x.package_id())).cloned()
                        .collect();

                    if !not_selected.is_empty() {
                        let indexes_to_unpublish: Vec<_> = indexes_to_unpublish.iter().map(|x| x.get(&not_selected)).collect();

                        let new_reg = registry(
                            input
                                .iter().filter(|&x| !indexes_to_unpublish.contains(&x)).cloned()
                                .collect(),
                        );

                        let res = resolve(
                            vec![dep_req(&this.name(), &format!("={}", this.version()))],
                            &new_reg,
                        );

                        // Note: that we can not assert that the two `res` are identical
                        // as the resolver does depend on irrelevant alternatives.
                        // It uses how constrained a dependency requirement is
                        // to determine what order to evaluate requirements.

                        prop_assert!(
                            res.is_ok(),
                            "unpublishing {:?} stopped `{} = \"={}\"` from working",
                            indexes_to_unpublish.iter().map(|x| x.package_id()).collect::<Vec<_>>(),
                            this.name(),
                            this.version()
                        )
                    }
                }

                Err(_) => {
                    // If resolution was unsuccessful, then it should stay unsuccessful
                    // even if any version of a crate is unpublished.
                    let indexes_to_unpublish: Vec<_> = indexes_to_unpublish.iter().map(|x| x.get(&input)).collect();

                    let new_reg = registry(
                        input
                            .iter().filter(|&x| !indexes_to_unpublish.contains(&x)).cloned()
                            .collect(),
                    );

                    let res = resolve(
                        vec![dep_req(&this.name(), &format!("={}", this.version()))],
                        &new_reg,
                    );

                    prop_assert!(
                        res.is_err(),
                        "full index did not work for `{} = \"={}\"` but unpublishing {:?} fixed it!",
                        this.name(),
                        this.version(),
                        indexes_to_unpublish.iter().map(|x| x.package_id()).collect::<Vec<_>>(),
                    )
                }
            }
        }
    }
}
