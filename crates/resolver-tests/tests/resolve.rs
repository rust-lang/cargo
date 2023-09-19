use std::io::IsTerminal;

use cargo::core::dependency::DepKind;
use cargo::core::Dependency;
use cargo::util::Config;
use cargo_util::is_ci;

use resolver_tests::{
    assert_contains, assert_same, dep, dep_kind, dep_loc, dep_req, dep_req_kind, loc_names, names,
    pkg, pkg_id, pkg_loc, registry, registry_strategy, remove_dep, resolve, resolve_and_validated,
    resolve_with_config, PrettyPrintRegistry, SatResolve, ToDep, ToPkgId,
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
        let sat_resolve = SatResolve::new(&reg);
        // there is only a small chance that any one
        // crate will be interesting.
        // So we try some of the most complicated.
        for this in input.iter().rev().take(20) {
            let _ = resolve_and_validated(
                vec![dep_req(&this.name(), &format!("={}", this.version()))],
                &reg,
                Some(sat_resolve.clone()),
            );
        }
    }

    /// NOTE: if you think this test has failed spuriously see the note at the top of this macro.
    #[test]
    fn prop_minimum_version_errors_the_same(
            PrettyPrintRegistry(input) in registry_strategy(50, 20, 60)
    ) {
        let mut config = Config::default().unwrap();
        config.nightly_features_allowed = true;
        config
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
        // there is only a small chance that any one
        // crate will be interesting.
        // So we try some of the most complicated.
        for this in input.iter().rev().take(10) {
            // minimal-versions change what order the candidates
            // are tried but not the existence of a solution
            let res = resolve(
                vec![dep_req(&this.name(), &format!("={}", this.version()))],
                &reg,
            );

            let mres = resolve_with_config(
                vec![dep_req(&this.name(), &format!("={}", this.version()))],
                &reg,
                &config,
            );

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
        let mut config = Config::default().unwrap();
        config.nightly_features_allowed = true;
        config
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
        // there is only a small chance that any one
        // crate will be interesting.
        // So we try some of the most complicated.
        for this in input.iter().rev().take(10) {
            // direct-minimal-versions reduces the number of available solutions, so we verify that
            // we do not come up with solutions maximal versions does not
            let res = resolve(
                vec![dep_req(&this.name(), &format!("={}", this.version()))],
                &reg,
            );

            let mres = resolve_with_config(
                vec![dep_req(&this.name(), &format!("={}", this.version()))],
                &reg,
                &config,
            );

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
        // there is only a small chance that any one
        // crate will be interesting.
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
        // there is only a small chance that any one
        // crate will be interesting.
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
                        .iter()
                        .cloned()
                        .filter(|x| !r.contains(&x.package_id()))
                        .collect();
                    if !not_selected.is_empty() {
                        let indexes_to_unpublish: Vec<_> = indexes_to_unpublish.iter().map(|x| x.get(&not_selected)).collect();

                        let new_reg = registry(
                            input
                                .iter()
                                .cloned()
                                .filter(|x| !indexes_to_unpublish.contains(&x))
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
                            .iter()
                            .cloned()
                            .filter(|x| !indexes_to_unpublish.contains(&x))
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

#[test]
#[should_panic(expected = "pub dep")] // The error handling is not yet implemented.
fn pub_fail() {
    let input = vec![
        pkg!(("a", "0.0.4")),
        pkg!(("a", "0.0.5")),
        pkg!(("e", "0.0.6") => [dep_req_kind("a", "<= 0.0.4", DepKind::Normal, true),]),
        pkg!(("kB", "0.0.3") => [dep_req("a", ">= 0.0.5"),dep("e"),]),
    ];
    let reg = registry(input);
    assert!(resolve_and_validated(vec![dep("kB")], &reg, None).is_err());
}

#[test]
fn basic_public_dependency() {
    let reg = registry(vec![
        pkg!(("A", "0.1.0")),
        pkg!(("A", "0.2.0")),
        pkg!("B" => [dep_req_kind("A", "0.1", DepKind::Normal, true)]),
        pkg!("C" => [dep("A"), dep("B")]),
    ]);

    let res = resolve_and_validated(vec![dep("C")], &reg, None).unwrap();
    assert_same(
        &res,
        &names(&[
            ("root", "1.0.0"),
            ("C", "1.0.0"),
            ("B", "1.0.0"),
            ("A", "0.1.0"),
        ]),
    );
}

#[test]
fn public_dependency_filling_in() {
    // The resolver has an optimization where if a candidate to resolve a dependency
    // has already bean activated then we skip looking at the candidates dependencies.
    // However, we have to be careful as the new path may make pub dependencies invalid.

    // Triggering this case requires dependencies to be resolved in a specific order.
    // Fuzzing found this unintuitive case, that triggers this unfortunate order of operations:
    // 1. `d`'s dep on `c` is resolved
    // 2. `d`'s dep on `a` is resolved with `0.1.1`
    // 3. `c`'s dep on `b` is resolved with `0.0.2`
    // 4. `b`'s dep on `a` is resolved with `0.0.6` no pub dev conflict as `b` is private to `c`
    // 5. `d`'s dep on `b` is resolved with `0.0.2` triggering the optimization.
    // Do we notice that `d` has a pub dep conflict on `a`? Lets try it and see.
    let reg = registry(vec![
        pkg!(("a", "0.0.6")),
        pkg!(("a", "0.1.1")),
        pkg!(("b", "0.0.0") => [dep("bad")]),
        pkg!(("b", "0.0.1") => [dep("bad")]),
        pkg!(("b", "0.0.2") => [dep_req_kind("a", "=0.0.6", DepKind::Normal, true)]),
        pkg!("c" => [dep_req("b", ">=0.0.1")]),
        pkg!("d" => [dep("c"), dep("a"), dep("b")]),
    ]);

    let res = resolve_and_validated(vec![dep("d")], &reg, None).unwrap();
    assert_same(
        &res,
        &names(&[
            ("root", "1.0.0"),
            ("d", "1.0.0"),
            ("c", "1.0.0"),
            ("b", "0.0.2"),
            ("a", "0.0.6"),
        ]),
    );
}

#[test]
fn public_dependency_filling_in_and_update() {
    // The resolver has an optimization where if a candidate to resolve a dependency
    // has already bean activated then we skip looking at the candidates dependencies.
    // However, we have to be careful as the new path may make pub dependencies invalid.

    // Triggering this case requires dependencies to be resolved in a specific order.
    // Fuzzing found this unintuitive case, that triggers this unfortunate order of operations:
    // 1. `D`'s dep on `B` is resolved
    // 2. `D`'s dep on `C` is resolved
    // 3. `B`'s dep on `A` is resolved with `0.0.0`
    // 4. `C`'s dep on `B` triggering the optimization.
    // So did we add `A 0.0.0` to the deps `C` can see?
    // Or are we going to  resolve `C`'s dep on `A` with `0.0.2`?
    // Lets try it and see.
    let reg = registry(vec![
        pkg!(("A", "0.0.0")),
        pkg!(("A", "0.0.2")),
        pkg!("B" => [dep_req_kind("A", "=0.0.0", DepKind::Normal, true),]),
        pkg!("C" => [dep("A"),dep("B")]),
        pkg!("D" => [dep("B"),dep("C")]),
    ]);
    let res = resolve_and_validated(vec![dep("D")], &reg, None).unwrap();
    assert_same(
        &res,
        &names(&[
            ("root", "1.0.0"),
            ("D", "1.0.0"),
            ("C", "1.0.0"),
            ("B", "1.0.0"),
            ("A", "0.0.0"),
        ]),
    );
}

#[test]
fn public_dependency_skipping() {
    // When backtracking due to a failed dependency, if Cargo is
    // trying to be clever and skip irrelevant dependencies, care must
    // the effects of pub dep must be accounted for.
    let input = vec![
        pkg!(("a", "0.2.0")),
        pkg!(("a", "2.0.0")),
        pkg!(("b", "0.0.0") => [dep("bad")]),
        pkg!(("b", "0.2.1") => [dep_req_kind("a", "0.2.0", DepKind::Normal, true)]),
        pkg!("c" => [dep("a"),dep("b")]),
    ];
    let reg = registry(input);

    resolve_and_validated(vec![dep("c")], &reg, None).unwrap();
}

#[test]
fn public_dependency_skipping_in_backtracking() {
    // When backtracking due to a failed dependency, if Cargo is
    // trying to be clever and skip irrelevant dependencies, care must
    // the effects of pub dep must be accounted for.
    let input = vec![
        pkg!(("A", "0.0.0") => [dep("bad")]),
        pkg!(("A", "0.0.1") => [dep("bad")]),
        pkg!(("A", "0.0.2") => [dep("bad")]),
        pkg!(("A", "0.0.3") => [dep("bad")]),
        pkg!(("A", "0.0.4")),
        pkg!(("A", "0.0.5")),
        pkg!("B" => [dep_req_kind("A", ">= 0.0.3", DepKind::Normal, true)]),
        pkg!("C" => [dep_req("A", "<= 0.0.4"), dep("B")]),
    ];
    let reg = registry(input);

    resolve_and_validated(vec![dep("C")], &reg, None).unwrap();
}

#[test]
fn public_sat_topological_order() {
    let input = vec![
        pkg!(("a", "0.0.1")),
        pkg!(("a", "0.0.0")),
        pkg!(("b", "0.0.1") => [dep_req_kind("a", "= 0.0.1", DepKind::Normal, true),]),
        pkg!(("b", "0.0.0") => [dep("bad"),]),
        pkg!("A" => [dep_req("a", "= 0.0.0"),dep_req_kind("b", "*", DepKind::Normal, true)]),
    ];

    let reg = registry(input);
    assert!(resolve_and_validated(vec![dep("A")], &reg, None).is_err());
}

#[test]
fn public_sat_unused_makes_things_pub() {
    let input = vec![
        pkg!(("a", "0.0.1")),
        pkg!(("a", "0.0.0")),
        pkg!(("b", "8.0.1") => [dep_req_kind("a", "= 0.0.1", DepKind::Normal, true),]),
        pkg!(("b", "8.0.0") => [dep_req("a", "= 0.0.1"),]),
        pkg!("c" => [dep_req("b", "= 8.0.0"),dep_req("a", "= 0.0.0"),]),
    ];
    let reg = registry(input);

    resolve_and_validated(vec![dep("c")], &reg, None).unwrap();
}

#[test]
fn public_sat_unused_makes_things_pub_2() {
    let input = vec![
        pkg!(("c", "0.0.2")),
        pkg!(("c", "0.0.1")),
        pkg!(("a-sys", "0.0.2")),
        pkg!(("a-sys", "0.0.1") => [dep_req_kind("c", "= 0.0.1", DepKind::Normal, true),]),
        pkg!("P" => [dep_req_kind("a-sys", "*", DepKind::Normal, true),dep_req("c", "= 0.0.1"),]),
        pkg!("A" => [dep("P"),dep_req("c", "= 0.0.2"),]),
    ];
    let reg = registry(input);

    resolve_and_validated(vec![dep("A")], &reg, None).unwrap();
}

#[test]
#[should_panic(expected = "assertion failed: !name.is_empty()")]
fn test_dependency_with_empty_name() {
    // Bug 5229, dependency-names must not be empty
    "".to_dep();
}

#[test]
fn test_resolving_empty_dependency_list() {
    let res = resolve(Vec::new(), &registry(vec![])).unwrap();

    assert_eq!(res, names(&["root"]));
}

#[test]
fn test_resolving_only_package() {
    let reg = registry(vec![pkg!("foo")]);
    let res = resolve(vec![dep("foo")], &reg).unwrap();
    assert_same(&res, &names(&["root", "foo"]));
}

#[test]
fn test_resolving_one_dep() {
    let reg = registry(vec![pkg!("foo"), pkg!("bar")]);
    let res = resolve(vec![dep("foo")], &reg).unwrap();
    assert_same(&res, &names(&["root", "foo"]));
}

#[test]
fn test_resolving_multiple_deps() {
    let reg = registry(vec![pkg!("foo"), pkg!("bar"), pkg!("baz")]);
    let res = resolve(vec![dep("foo"), dep("baz")], &reg).unwrap();
    assert_same(&res, &names(&["root", "foo", "baz"]));
}

#[test]
fn test_resolving_transitive_deps() {
    let reg = registry(vec![pkg!("foo"), pkg!("bar" => ["foo"])]);
    let res = resolve(vec![dep("bar")], &reg).unwrap();

    assert_same(&res, &names(&["root", "foo", "bar"]));
}

#[test]
fn test_resolving_common_transitive_deps() {
    let reg = registry(vec![pkg!("foo" => ["bar"]), pkg!("bar")]);
    let res = resolve(vec![dep("foo"), dep("bar")], &reg).unwrap();

    assert_same(&res, &names(&["root", "foo", "bar"]));
}

#[test]
fn test_resolving_with_same_name() {
    let list = vec![
        pkg_loc("foo", "https://first.example.com"),
        pkg_loc("bar", "https://second.example.com"),
    ];

    let reg = registry(list);
    let res = resolve(
        vec![
            dep_loc("foo", "https://first.example.com"),
            dep_loc("bar", "https://second.example.com"),
        ],
        &reg,
    )
    .unwrap();

    let mut names = loc_names(&[
        ("foo", "https://first.example.com"),
        ("bar", "https://second.example.com"),
    ]);

    names.push(pkg_id("root"));
    assert_same(&res, &names);
}

#[test]
fn test_resolving_with_dev_deps() {
    let reg = registry(vec![
        pkg!("foo" => ["bar", dep_kind("baz", DepKind::Development)]),
        pkg!("baz" => ["bat", dep_kind("bam", DepKind::Development)]),
        pkg!("bar"),
        pkg!("bat"),
    ]);

    let res = resolve(
        vec![dep("foo"), dep_kind("baz", DepKind::Development)],
        &reg,
    )
    .unwrap();

    assert_same(&res, &names(&["root", "foo", "bar", "baz", "bat"]));
}

#[test]
fn resolving_with_many_versions() {
    let reg = registry(vec![pkg!(("foo", "1.0.1")), pkg!(("foo", "1.0.2"))]);

    let res = resolve(vec![dep("foo")], &reg).unwrap();

    assert_same(&res, &names(&[("root", "1.0.0"), ("foo", "1.0.2")]));
}

#[test]
fn resolving_with_specific_version() {
    let reg = registry(vec![pkg!(("foo", "1.0.1")), pkg!(("foo", "1.0.2"))]);

    let res = resolve(vec![dep_req("foo", "=1.0.1")], &reg).unwrap();

    assert_same(&res, &names(&[("root", "1.0.0"), ("foo", "1.0.1")]));
}

#[test]
fn test_resolving_maximum_version_with_transitive_deps() {
    let reg = registry(vec![
        pkg!(("util", "1.2.2")),
        pkg!(("util", "1.0.0")),
        pkg!(("util", "1.1.1")),
        pkg!("foo" => [dep_req("util", "1.0.0")]),
        pkg!("bar" => [dep_req("util", ">=1.0.1")]),
    ]);

    let res = resolve(vec![dep_req("foo", "1.0.0"), dep_req("bar", "1.0.0")], &reg).unwrap();

    assert_contains(
        &res,
        &names(&[
            ("root", "1.0.0"),
            ("foo", "1.0.0"),
            ("bar", "1.0.0"),
            ("util", "1.2.2"),
        ]),
    );
    assert!(!res.contains(&("util", "1.0.1").to_pkgid()));
    assert!(!res.contains(&("util", "1.1.1").to_pkgid()));
}

#[test]
fn test_resolving_minimum_version_with_transitive_deps() {
    let reg = registry(vec![
        pkg!(("util", "1.2.2")),
        pkg!(("util", "1.0.0")),
        pkg!(("util", "1.1.1")),
        pkg!("foo" => [dep_req("util", "1.0.0")]),
        pkg!("bar" => [dep_req("util", ">=1.0.1")]),
    ]);

    let mut config = Config::default().unwrap();
    // -Z minimal-versions
    // When the minimal-versions config option is specified then the lowest
    // possible version of a package should be selected. "util 1.0.0" can't be
    // selected because of the requirements of "bar", so the minimum version
    // must be 1.1.1.
    config.nightly_features_allowed = true;
    config
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

    let res = resolve_with_config(
        vec![dep_req("foo", "1.0.0"), dep_req("bar", "1.0.0")],
        &reg,
        &config,
    )
    .unwrap();

    assert_contains(
        &res,
        &names(&[
            ("root", "1.0.0"),
            ("foo", "1.0.0"),
            ("bar", "1.0.0"),
            ("util", "1.1.1"),
        ]),
    );
    assert!(!res.contains(&("util", "1.2.2").to_pkgid()));
    assert!(!res.contains(&("util", "1.0.0").to_pkgid()));
}

#[test]
fn resolving_incompat_versions() {
    let reg = registry(vec![
        pkg!(("foo", "1.0.1")),
        pkg!(("foo", "1.0.2")),
        pkg!("bar" => [dep_req("foo", "=1.0.2")]),
    ]);

    assert!(resolve(vec![dep_req("foo", "=1.0.1"), dep("bar")], &reg).is_err());
}

#[test]
fn resolving_wrong_case_from_registry() {
    // In the future we may #5678 allow this to happen.
    // For back compatibility reasons, we probably won't.
    // But we may want to future prove ourselves by understanding it.
    // This test documents the current behavior.
    let reg = registry(vec![pkg!(("foo", "1.0.0")), pkg!("bar" => ["Foo"])]);

    assert!(resolve(vec![dep("bar")], &reg).is_err());
}

#[test]
fn resolving_mis_hyphenated_from_registry() {
    // In the future we may #2775 allow this to happen.
    // For back compatibility reasons, we probably won't.
    // But we may want to future prove ourselves by understanding it.
    // This test documents the current behavior.
    let reg = registry(vec![pkg!(("fo-o", "1.0.0")), pkg!("bar" => ["fo_o"])]);

    assert!(resolve(vec![dep("bar")], &reg).is_err());
}

#[test]
fn resolving_backtrack() {
    let reg = registry(vec![
        pkg!(("foo", "1.0.2") => [dep("bar")]),
        pkg!(("foo", "1.0.1") => [dep("baz")]),
        pkg!("bar" => [dep_req("foo", "=2.0.2")]),
        pkg!("baz"),
    ]);

    let res = resolve(vec![dep_req("foo", "^1")], &reg).unwrap();

    assert_contains(
        &res,
        &names(&[("root", "1.0.0"), ("foo", "1.0.1"), ("baz", "1.0.0")]),
    );
}

#[test]
fn resolving_backtrack_features() {
    // test for cargo/issues/4347
    let mut bad = dep("bar");
    bad.set_features(vec!["bad"]);

    let reg = registry(vec![
        pkg!(("foo", "1.0.2") => [bad]),
        pkg!(("foo", "1.0.1") => [dep("bar")]),
        pkg!("bar"),
    ]);

    let res = resolve(vec![dep_req("foo", "^1")], &reg).unwrap();

    assert_contains(
        &res,
        &names(&[("root", "1.0.0"), ("foo", "1.0.1"), ("bar", "1.0.0")]),
    );
}

#[test]
fn resolving_allows_multiple_compatible_versions() {
    let reg = registry(vec![
        pkg!(("foo", "1.0.0")),
        pkg!(("foo", "2.0.0")),
        pkg!(("foo", "0.1.0")),
        pkg!(("foo", "0.2.0")),
        pkg!("bar" => ["d1", "d2", "d3", "d4"]),
        pkg!("d1" => [dep_req("foo", "1")]),
        pkg!("d2" => [dep_req("foo", "2")]),
        pkg!("d3" => [dep_req("foo", "0.1")]),
        pkg!("d4" => [dep_req("foo", "0.2")]),
    ]);

    let res = resolve(vec![dep("bar")], &reg).unwrap();

    assert_same(
        &res,
        &names(&[
            ("root", "1.0.0"),
            ("foo", "1.0.0"),
            ("foo", "2.0.0"),
            ("foo", "0.1.0"),
            ("foo", "0.2.0"),
            ("d1", "1.0.0"),
            ("d2", "1.0.0"),
            ("d3", "1.0.0"),
            ("d4", "1.0.0"),
            ("bar", "1.0.0"),
        ]),
    );
}

#[test]
fn resolving_with_deep_backtracking() {
    let reg = registry(vec![
        pkg!(("foo", "1.0.1") => [dep_req("bar", "1")]),
        pkg!(("foo", "1.0.0") => [dep_req("bar", "2")]),
        pkg!(("bar", "1.0.0") => [dep_req("baz", "=1.0.2"),
                                  dep_req("other", "1")]),
        pkg!(("bar", "2.0.0") => [dep_req("baz", "=1.0.1")]),
        pkg!(("baz", "1.0.2") => [dep_req("other", "2")]),
        pkg!(("baz", "1.0.1")),
        pkg!(("dep_req", "1.0.0")),
        pkg!(("dep_req", "2.0.0")),
    ]);

    let res = resolve(vec![dep_req("foo", "1")], &reg).unwrap();

    assert_same(
        &res,
        &names(&[
            ("root", "1.0.0"),
            ("foo", "1.0.0"),
            ("bar", "2.0.0"),
            ("baz", "1.0.1"),
        ]),
    );
}

#[test]
fn resolving_with_sys_crates() {
    // This is based on issues/4902
    // With `l` a normal library we get 2copies so everyone gets the newest compatible.
    // But `l-sys` a library with a links attribute we make sure there is only one.
    let reg = registry(vec![
        pkg!(("l-sys", "0.9.1")),
        pkg!(("l-sys", "0.10.0")),
        pkg!(("l", "0.9.1")),
        pkg!(("l", "0.10.0")),
        pkg!(("d", "1.0.0") => [dep_req("l-sys", ">=0.8.0, <=0.10.0"), dep_req("l", ">=0.8.0, <=0.10.0")]),
        pkg!(("r", "1.0.0") => [dep_req("l-sys", "0.9"), dep_req("l", "0.9")]),
    ]);

    let res = resolve(vec![dep_req("d", "1"), dep_req("r", "1")], &reg).unwrap();

    assert_same(
        &res,
        &names(&[
            ("root", "1.0.0"),
            ("d", "1.0.0"),
            ("r", "1.0.0"),
            ("l-sys", "0.9.1"),
            ("l", "0.9.1"),
            ("l", "0.10.0"),
        ]),
    );
}

#[test]
fn resolving_with_constrained_sibling_backtrack_parent() {
    // There is no point in considering all of the backtrack_trap{1,2}
    // candidates since they can't change the result of failing to
    // resolve 'constrained'. Cargo should (ideally) skip past them and resume
    // resolution once the activation of the parent, 'bar', is rolled back.
    // Note that the traps are slightly more constrained to make sure they
    // get picked first.
    let mut reglist = vec![
        pkg!(("foo", "1.0.0") => [dep_req("bar", "1.0"),
                                  dep_req("constrained", "=1.0.0")]),
        pkg!(("bar", "1.0.0") => [dep_req("backtrack_trap1", "1.0.2"),
                                  dep_req("backtrack_trap2", "1.0.2"),
                                  dep_req("constrained", "1.0.0")]),
        pkg!(("constrained", "1.0.0")),
        pkg!(("backtrack_trap1", "1.0.0")),
        pkg!(("backtrack_trap2", "1.0.0")),
    ];
    // Bump this to make the test harder - it adds more versions of bar that will
    // fail to resolve, and more versions of the traps to consider.
    const NUM_BARS_AND_TRAPS: usize = 50; // minimum 2
    for i in 1..NUM_BARS_AND_TRAPS {
        let vsn = format!("1.0.{}", i);
        reglist.push(
            pkg!(("bar", vsn.clone()) => [dep_req("backtrack_trap1", "1.0.2"),
                                                   dep_req("backtrack_trap2", "1.0.2"),
                                                   dep_req("constrained", "1.0.1")]),
        );
        reglist.push(pkg!(("backtrack_trap1", vsn.clone())));
        reglist.push(pkg!(("backtrack_trap2", vsn.clone())));
        reglist.push(pkg!(("constrained", vsn.clone())));
    }
    let reg = registry(reglist);

    let res = resolve(vec![dep_req("foo", "1")], &reg).unwrap();

    assert_contains(
        &res,
        &names(&[
            ("root", "1.0.0"),
            ("foo", "1.0.0"),
            ("bar", "1.0.0"),
            ("constrained", "1.0.0"),
        ]),
    );
}

#[test]
fn resolving_with_many_equivalent_backtracking() {
    let mut reglist = Vec::new();

    const DEPTH: usize = 200;
    const BRANCHING_FACTOR: usize = 100;

    // Each level depends on the next but the last level does not exist.
    // Without cashing we need to test every path to the last level O(BRANCHING_FACTOR ^ DEPTH)
    // and this test will time out. With cashing we need to discover that none of these
    // can be activated O(BRANCHING_FACTOR * DEPTH)
    for l in 0..DEPTH {
        let name = format!("level{}", l);
        let next = format!("level{}", l + 1);
        for i in 1..BRANCHING_FACTOR {
            let vsn = format!("1.0.{}", i);
            reglist.push(pkg!((name.as_str(), vsn.as_str()) => [dep(next.as_str())]));
        }
    }

    let reg = registry(reglist.clone());

    let res = resolve(vec![dep("level0")], &reg);

    assert!(res.is_err());

    // It is easy to write code that quickly returns an error.
    // Lets make sure we can find a good answer if it is there.
    reglist.push(pkg!(("level0", "1.0.0")));

    let reg = registry(reglist.clone());

    let res = resolve(vec![dep("level0")], &reg).unwrap();

    assert_contains(&res, &names(&[("root", "1.0.0"), ("level0", "1.0.0")]));

    // Make sure we have not special case no candidates.
    reglist.push(pkg!(("constrained", "1.1.0")));
    reglist.push(pkg!(("constrained", "1.0.0")));
    reglist.push(
        pkg!((format!("level{}", DEPTH).as_str(), "1.0.0") => [dep_req("constrained", "=1.0.0")]),
    );

    let reg = registry(reglist.clone());

    let res = resolve(vec![dep("level0"), dep("constrained")], &reg).unwrap();

    assert_contains(
        &res,
        &names(&[
            ("root", "1.0.0"),
            ("level0", "1.0.0"),
            ("constrained", "1.1.0"),
        ]),
    );

    let reg = registry(reglist.clone());

    let res = resolve(vec![dep_req("level0", "1.0.1"), dep("constrained")], &reg).unwrap();

    assert_contains(
        &res,
        &names(&[
            ("root", "1.0.0"),
            (format!("level{}", DEPTH).as_str(), "1.0.0"),
            ("constrained", "1.0.0"),
        ]),
    );

    let reg = registry(reglist);

    let res = resolve(
        vec![dep_req("level0", "1.0.1"), dep_req("constrained", "1.1.0")],
        &reg,
    );

    assert!(res.is_err());
}

#[test]
fn resolving_with_deep_traps() {
    let mut reglist = Vec::new();

    const DEPTH: usize = 200;
    const BRANCHING_FACTOR: usize = 100;

    // Each backtrack_trap depends on the next, and adds a backtrack frame.
    // None of witch is going to help with `bad`.
    for l in 0..DEPTH {
        let name = format!("backtrack_trap{}", l);
        let next = format!("backtrack_trap{}", l + 1);
        for i in 1..BRANCHING_FACTOR {
            let vsn = format!("1.0.{}", i);
            reglist.push(pkg!((name.as_str(), vsn.as_str()) => [dep(next.as_str())]));
        }
    }
    {
        let name = format!("backtrack_trap{}", DEPTH);
        for i in 1..BRANCHING_FACTOR {
            let vsn = format!("1.0.{}", i);
            reglist.push(pkg!((name.as_str(), vsn.as_str())));
        }
    }
    {
        // slightly less constrained to make sure `cloaking` gets picked last.
        for i in 1..(BRANCHING_FACTOR + 10) {
            let vsn = format!("1.0.{}", i);
            reglist.push(pkg!(("cloaking", vsn.as_str()) => [dep_req("bad", "1.0.1")]));
        }
    }

    let reg = registry(reglist);

    let res = resolve(vec![dep("backtrack_trap0"), dep("cloaking")], &reg);

    assert!(res.is_err());
}

#[test]
fn resolving_with_constrained_cousins_backtrack() {
    let mut reglist = Vec::new();

    const DEPTH: usize = 100;
    const BRANCHING_FACTOR: usize = 50;

    // Each backtrack_trap depends on the next.
    // The last depends on a specific ver of constrained.
    for l in 0..DEPTH {
        let name = format!("backtrack_trap{}", l);
        let next = format!("backtrack_trap{}", l + 1);
        for i in 1..BRANCHING_FACTOR {
            let vsn = format!("1.0.{}", i);
            reglist.push(pkg!((name.as_str(), vsn.as_str()) => [dep(next.as_str())]));
        }
    }
    {
        let name = format!("backtrack_trap{}", DEPTH);
        for i in 1..BRANCHING_FACTOR {
            let vsn = format!("1.0.{}", i);
            reglist.push(
                pkg!((name.as_str(), vsn.as_str()) => [dep_req("constrained", ">=1.1.0, <=2.0.0")]),
            );
        }
    }
    {
        // slightly less constrained to make sure `constrained` gets picked last.
        for i in 0..(BRANCHING_FACTOR + 10) {
            let vsn = format!("1.0.{}", i);
            reglist.push(pkg!(("constrained", vsn.as_str())));
        }
        reglist.push(pkg!(("constrained", "1.1.0")));
        reglist.push(pkg!(("constrained", "2.0.0")));
        reglist.push(pkg!(("constrained", "2.0.1")));
    }
    reglist.push(pkg!(("cloaking", "1.0.0") => [dep_req("constrained", "~1.0.0")]));

    let reg = registry(reglist.clone());

    // `backtrack_trap0 = "*"` is a lot of ways of saying `constrained = ">=1.1.0, <=2.0.0"`
    // but `constrained= "2.0.1"` is already picked.
    // Only then to try and solve `constrained= "~1.0.0"` which is incompatible.
    let res = resolve(
        vec![
            dep("backtrack_trap0"),
            dep_req("constrained", "2.0.1"),
            dep("cloaking"),
        ],
        &reg,
    );

    assert!(res.is_err());

    // Each level depends on the next but the last depends on incompatible deps.
    // Let's make sure that we can cache that a dep has incompatible deps.
    for l in 0..DEPTH {
        let name = format!("level{}", l);
        let next = format!("level{}", l + 1);
        for i in 1..BRANCHING_FACTOR {
            let vsn = format!("1.0.{}", i);
            reglist.push(pkg!((name.as_str(), vsn.as_str()) => [dep(next.as_str())]));
        }
    }
    reglist.push(
        pkg!((format!("level{}", DEPTH).as_str(), "1.0.0") => [dep("backtrack_trap0"),
            dep("cloaking")
        ]),
    );

    let reg = registry(reglist);

    let res = resolve(vec![dep("level0"), dep_req("constrained", "2.0.1")], &reg);

    assert!(res.is_err());

    let res = resolve(vec![dep("level0"), dep_req("constrained", "2.0.0")], &reg).unwrap();

    assert_contains(
        &res,
        &names(&[("constrained", "2.0.0"), ("cloaking", "1.0.0")]),
    );
}

#[test]
fn resolving_with_constrained_sibling_backtrack_activation() {
    // It makes sense to resolve most-constrained deps first, but
    // with that logic the backtrack traps here come between the two
    // attempted resolutions of 'constrained'. When backtracking,
    // cargo should skip past them and resume resolution once the
    // number of activations for 'constrained' changes.
    let mut reglist = vec![
        pkg!(("foo", "1.0.0") => [dep_req("bar", "=1.0.0"),
                                  dep_req("backtrack_trap1", "1.0"),
                                  dep_req("backtrack_trap2", "1.0"),
                                  dep_req("constrained", "<=1.0.60")]),
        pkg!(("bar", "1.0.0") => [dep_req("constrained", ">=1.0.60")]),
    ];
    // Bump these to make the test harder, but you'll also need to
    // change the version constraints on `constrained` above. To correctly
    // exercise Cargo, the relationship between the values is:
    // NUM_CONSTRAINED - vsn < NUM_TRAPS < vsn
    // to make sure the traps are resolved between `constrained`.
    const NUM_TRAPS: usize = 45; // min 1
    const NUM_CONSTRAINED: usize = 100; // min 1
    for i in 0..NUM_TRAPS {
        let vsn = format!("1.0.{}", i);
        reglist.push(pkg!(("backtrack_trap1", vsn.clone())));
        reglist.push(pkg!(("backtrack_trap2", vsn.clone())));
    }
    for i in 0..NUM_CONSTRAINED {
        let vsn = format!("1.0.{}", i);
        reglist.push(pkg!(("constrained", vsn.clone())));
    }
    let reg = registry(reglist);

    let res = resolve(vec![dep_req("foo", "1")], &reg).unwrap();

    assert_contains(
        &res,
        &names(&[
            ("root", "1.0.0"),
            ("foo", "1.0.0"),
            ("bar", "1.0.0"),
            ("constrained", "1.0.60"),
        ]),
    );
}

#[test]
fn resolving_with_public_constrained_sibling() {
    // It makes sense to resolve most-constrained deps first, but
    // with that logic the backtrack traps here come between the two
    // attempted resolutions of 'constrained'. When backtracking,
    // cargo should skip past them and resume resolution once the
    // number of activations for 'constrained' changes.
    let mut reglist = vec![
        pkg!(("foo", "1.0.0") => [dep_req("bar", "=1.0.0"),
                                  dep_req("backtrack_trap1", "1.0"),
                                  dep_req("backtrack_trap2", "1.0"),
                                  dep_req("constrained", "<=60")]),
        pkg!(("bar", "1.0.0") => [dep_req_kind("constrained", ">=60", DepKind::Normal, true)]),
    ];
    // Bump these to make the test harder, but you'll also need to
    // change the version constraints on `constrained` above. To correctly
    // exercise Cargo, the relationship between the values is:
    // NUM_CONSTRAINED - vsn < NUM_TRAPS < vsn
    // to make sure the traps are resolved between `constrained`.
    const NUM_TRAPS: usize = 45; // min 1
    const NUM_CONSTRAINED: usize = 100; // min 1
    for i in 0..NUM_TRAPS {
        let vsn = format!("1.0.{}", i);
        reglist.push(pkg!(("backtrack_trap1", vsn.clone())));
        reglist.push(pkg!(("backtrack_trap2", vsn.clone())));
    }
    for i in 0..NUM_CONSTRAINED {
        let vsn = format!("{}.0.0", i);
        reglist.push(pkg!(("constrained", vsn.clone())));
    }
    let reg = registry(reglist);

    let _ = resolve_and_validated(vec![dep_req("foo", "1")], &reg, None);
}

#[test]
fn resolving_with_constrained_sibling_transitive_dep_effects() {
    // When backtracking due to a failed dependency, if Cargo is
    // trying to be clever and skip irrelevant dependencies, care must
    // be taken to not miss the transitive effects of alternatives. E.g.
    // in the right-to-left resolution of the graph below, B may
    // affect whether D is successfully resolved.
    //
    //    A
    //  / | \
    // B  C  D
    // |  |
    // C  D
    let reg = registry(vec![
        pkg!(("A", "1.0.0") => [dep_req("B", "1.0"),
                                dep_req("C", "1.0"),
                                dep_req("D", "1.0.100")]),
        pkg!(("B", "1.0.0") => [dep_req("C", ">=1.0.0")]),
        pkg!(("B", "1.0.1") => [dep_req("C", ">=1.0.1")]),
        pkg!(("C", "1.0.0") => [dep_req("D", "1.0.0")]),
        pkg!(("C", "1.0.1") => [dep_req("D", ">=1.0.1,<1.0.100")]),
        pkg!(("C", "1.0.2") => [dep_req("D", ">=1.0.2,<1.0.100")]),
        pkg!(("D", "1.0.0")),
        pkg!(("D", "1.0.1")),
        pkg!(("D", "1.0.2")),
        pkg!(("D", "1.0.100")),
        pkg!(("D", "1.0.101")),
        pkg!(("D", "1.0.102")),
        pkg!(("D", "1.0.103")),
        pkg!(("D", "1.0.104")),
        pkg!(("D", "1.0.105")),
    ]);

    let res = resolve(vec![dep_req("A", "1")], &reg).unwrap();

    assert_same(
        &res,
        &names(&[
            ("root", "1.0.0"),
            ("A", "1.0.0"),
            ("B", "1.0.0"),
            ("C", "1.0.0"),
            ("D", "1.0.105"),
        ]),
    );
}

#[test]
fn incomplete_information_skipping() {
    // When backtracking due to a failed dependency, if Cargo is
    // trying to be clever and skip irrelevant dependencies, care must
    // be taken to not miss the transitive effects of alternatives.
    // Fuzzing discovered that for some reason cargo was skipping based
    // on incomplete information in the following case:
    // minimized bug found in:
    // https://github.com/rust-lang/cargo/commit/003c29b0c71e5ea28fbe8e72c148c755c9f3f8d9
    let input = vec![
        pkg!(("a", "1.0.0")),
        pkg!(("a", "1.1.0")),
        pkg!("b" => [dep("a")]),
        pkg!(("c", "1.0.0")),
        pkg!(("c", "1.1.0")),
        pkg!("d" => [dep_req("c", "=1.0")]),
        pkg!(("e", "1.0.0")),
        pkg!(("e", "1.1.0") => [dep_req("c", "1.1")]),
        pkg!("to_yank"),
        pkg!(("f", "1.0.0") => [
            dep("to_yank"),
            dep("d"),
        ]),
        pkg!(("f", "1.1.0") => [dep("d")]),
        pkg!("g" => [
            dep("b"),
            dep("e"),
            dep("f"),
        ]),
    ];
    let reg = registry(input.clone());

    let res = resolve(vec![dep("g")], &reg).unwrap();
    let package_to_yank = "to_yank".to_pkgid();
    // this package is not used in the resolution.
    assert!(!res.contains(&package_to_yank));
    // so when we yank it
    let new_reg = registry(
        input
            .iter()
            .cloned()
            .filter(|x| package_to_yank != x.package_id())
            .collect(),
    );
    assert_eq!(input.len(), new_reg.len() + 1);
    // it should still build
    assert!(resolve(vec![dep("g")], &new_reg).is_ok());
}

#[test]
fn incomplete_information_skipping_2() {
    // When backtracking due to a failed dependency, if Cargo is
    // trying to be clever and skip irrelevant dependencies, care must
    // be taken to not miss the transitive effects of alternatives.
    // Fuzzing discovered that for some reason cargo was skipping based
    // on incomplete information in the following case:
    // https://github.com/rust-lang/cargo/commit/003c29b0c71e5ea28fbe8e72c148c755c9f3f8d9
    let input = vec![
        pkg!(("b", "3.8.10")),
        pkg!(("b", "8.7.4")),
        pkg!(("b", "9.4.6")),
        pkg!(("c", "1.8.8")),
        pkg!(("c", "10.2.5")),
        pkg!(("d", "4.1.2") => [
            dep_req("bad", "=6.10.9"),
        ]),
        pkg!(("d", "5.5.6")),
        pkg!(("d", "5.6.10")),
        pkg!(("to_yank", "8.0.1")),
        pkg!(("to_yank", "8.8.1")),
        pkg!(("e", "4.7.8") => [
            dep_req("d", ">=5.5.6, <=5.6.10"),
            dep_req("to_yank", "=8.0.1"),
        ]),
        pkg!(("e", "7.4.9") => [
            dep_req("bad", "=4.7.5"),
        ]),
        pkg!("f" => [
            dep_req("d", ">=4.1.2, <=5.5.6"),
        ]),
        pkg!("g" => [
            dep("bad"),
        ]),
        pkg!(("h", "3.8.3") => [
            dep("g"),
        ]),
        pkg!(("h", "6.8.3") => [
            dep("f"),
        ]),
        pkg!(("h", "8.1.9") => [
            dep_req("to_yank", "=8.8.1"),
        ]),
        pkg!("i" => [
            dep("b"),
            dep("c"),
            dep("e"),
            dep("h"),
        ]),
    ];
    let reg = registry(input.clone());

    let res = resolve(vec![dep("i")], &reg).unwrap();
    let package_to_yank = ("to_yank", "8.8.1").to_pkgid();
    // this package is not used in the resolution.
    assert!(!res.contains(&package_to_yank));
    // so when we yank it
    let new_reg = registry(
        input
            .iter()
            .cloned()
            .filter(|x| package_to_yank != x.package_id())
            .collect(),
    );
    assert_eq!(input.len(), new_reg.len() + 1);
    // it should still build
    assert!(resolve(vec![dep("i")], &new_reg).is_ok());
}

#[test]
fn incomplete_information_skipping_3() {
    // When backtracking due to a failed dependency, if Cargo is
    // trying to be clever and skip irrelevant dependencies, care must
    // be taken to not miss the transitive effects of alternatives.
    // Fuzzing discovered that for some reason cargo was skipping based
    // on incomplete information in the following case:
    // minimized bug found in:
    // https://github.com/rust-lang/cargo/commit/003c29b0c71e5ea28fbe8e72c148c755c9f3f8d9
    let input = vec![
        pkg! {("to_yank", "3.0.3")},
        pkg! {("to_yank", "3.3.0")},
        pkg! {("to_yank", "3.3.1")},
        pkg! {("a", "3.3.0") => [
            dep_req("to_yank", "=3.0.3"),
        ] },
        pkg! {("a", "3.3.2") => [
            dep_req("to_yank", "<=3.3.0"),
        ] },
        pkg! {("b", "0.1.3") => [
            dep_req("a", "=3.3.0"),
        ] },
        pkg! {("b", "2.0.2") => [
            dep_req("to_yank", "3.3.0"),
            dep("a"),
        ] },
        pkg! {("b", "2.3.3") => [
            dep_req("to_yank", "3.3.0"),
            dep_req("a", "=3.3.0"),
        ] },
    ];
    let reg = registry(input.clone());

    let res = resolve(vec![dep("b")], &reg).unwrap();
    let package_to_yank = ("to_yank", "3.0.3").to_pkgid();
    // this package is not used in the resolution.
    assert!(!res.contains(&package_to_yank));
    // so when we yank it
    let new_reg = registry(
        input
            .iter()
            .cloned()
            .filter(|x| package_to_yank != x.package_id())
            .collect(),
    );
    assert_eq!(input.len(), new_reg.len() + 1);
    // it should still build
    assert!(resolve(vec![dep("b")], &new_reg).is_ok());
}

#[test]
fn resolving_but_no_exists() {
    let reg = registry(vec![]);

    let res = resolve(vec![dep_req("foo", "1")], &reg);
    assert!(res.is_err());

    assert_eq!(
        res.err().unwrap().to_string(),
        "no matching package named `foo` found\n\
         location searched: registry `https://example.com/`\n\
         required by package `root v1.0.0 (registry `https://example.com/`)`\
         "
    );
}

#[test]
fn resolving_cycle() {
    let reg = registry(vec![pkg!("foo" => ["foo"])]);

    let _ = resolve(vec![dep_req("foo", "1")], &reg);
}

#[test]
fn hard_equality() {
    let reg = registry(vec![
        pkg!(("foo", "1.0.1")),
        pkg!(("foo", "1.0.0")),
        pkg!(("bar", "1.0.0") => [dep_req("foo", "1.0.0")]),
    ]);

    let res = resolve(vec![dep_req("bar", "1"), dep_req("foo", "=1.0.0")], &reg).unwrap();

    assert_same(
        &res,
        &names(&[("root", "1.0.0"), ("foo", "1.0.0"), ("bar", "1.0.0")]),
    );
}

#[test]
fn large_conflict_cache() {
    let mut input = vec![
        pkg!(("last", "0.0.0") => [dep("bad")]), // just to make sure last is less constrained
    ];
    let mut root_deps = vec![dep("last")];
    const NUM_VERSIONS: u8 = 20;
    for name in 0..=NUM_VERSIONS {
        // a large number of conflicts can easily be generated by a sys crate.
        let sys_name = format!("{}-sys", (b'a' + name) as char);
        let in_len = input.len();
        input.push(pkg!(("last", format!("{}.0.0", in_len)) => [dep_req(&sys_name, "=0.0.0")]));
        root_deps.push(dep_req(&sys_name, ">= 0.0.1"));

        // a large number of conflicts can also easily be generated by a major release version.
        let plane_name = format!("{}", (b'a' + name) as char);
        let in_len = input.len();
        input.push(pkg!(("last", format!("{}.0.0", in_len)) => [dep_req(&plane_name, "=1.0.0")]));
        root_deps.push(dep_req(&plane_name, ">= 1.0.1"));

        for i in 0..=NUM_VERSIONS {
            input.push(pkg!((&sys_name, format!("{}.0.0", i))));
            input.push(pkg!((&plane_name, format!("1.0.{}", i))));
        }
    }
    let reg = registry(input);
    let _ = resolve(root_deps, &reg);
}

#[test]
fn off_by_one_bug() {
    let input = vec![
        pkg!(("A-sys", "0.0.1")),
        pkg!(("A-sys", "0.0.4")),
        pkg!(("A-sys", "0.0.6")),
        pkg!(("A-sys", "0.0.7")),
        pkg!(("NA", "0.0.0") => [dep_req("A-sys", "<= 0.0.5"),]),
        pkg!(("NA", "0.0.1") => [dep_req("A-sys", ">= 0.0.6, <= 0.0.8"),]),
        pkg!(("a", "0.0.1")),
        pkg!(("a", "0.0.2")),
        pkg!(("aa", "0.0.0") => [dep_req("A-sys", ">= 0.0.4, <= 0.0.6"),dep_req("NA", "<= 0.0.0"),]),
        pkg!(("f", "0.0.3") => [dep("NA"),dep_req("a", "<= 0.0.2"),dep("aa"),]),
    ];

    let reg = registry(input);
    let _ = resolve_and_validated(vec![dep("f")], &reg, None);
}

#[test]
fn conflict_store_bug() {
    let input = vec![
        pkg!(("A", "0.0.3")),
        pkg!(("A", "0.0.5")),
        pkg!(("A", "0.0.9") => [dep("bad"),]),
        pkg!(("A", "0.0.10") => [dep("bad"),]),
        pkg!(("L-sys", "0.0.1") => [dep("bad"),]),
        pkg!(("L-sys", "0.0.5")),
        pkg!(("R", "0.0.4") => [
            dep_req("L-sys", "= 0.0.5"),
        ]),
        pkg!(("R", "0.0.6")),
        pkg!(("a-sys", "0.0.5")),
        pkg!(("a-sys", "0.0.11")),
        pkg!(("c", "0.0.12") => [
            dep_req("R", ">= 0.0.3, <= 0.0.4"),
        ]),
        pkg!(("c", "0.0.13") => [
            dep_req("a-sys", ">= 0.0.8, <= 0.0.11"),
        ]),
        pkg!(("c0", "0.0.6") => [
            dep_req("L-sys", "<= 0.0.2"),
        ]),
        pkg!(("c0", "0.0.10") => [
            dep_req("A", ">= 0.0.9, <= 0.0.10"),
            dep_req("a-sys", "= 0.0.5"),
        ]),
        pkg!("j" => [
            dep_req("A", ">= 0.0.3, <= 0.0.5"),
            dep_req("R", ">=0.0.4, <= 0.0.6"),
            dep_req("c", ">= 0.0.9"),
            dep_req("c0", ">= 0.0.6"),
        ]),
    ];

    let reg = registry(input);
    let _ = resolve_and_validated(vec![dep("j")], &reg, None);
}

#[test]
fn conflict_store_more_then_one_match() {
    let input = vec![
        pkg!(("A", "0.0.0")),
        pkg!(("A", "0.0.1")),
        pkg!(("A-sys", "0.0.0")),
        pkg!(("A-sys", "0.0.1")),
        pkg!(("A-sys", "0.0.2")),
        pkg!(("A-sys", "0.0.3")),
        pkg!(("A-sys", "0.0.12")),
        pkg!(("A-sys", "0.0.16")),
        pkg!(("B-sys", "0.0.0")),
        pkg!(("B-sys", "0.0.1")),
        pkg!(("B-sys", "0.0.2") => [dep_req("A-sys", "= 0.0.12"),]),
        pkg!(("BA-sys", "0.0.0") => [dep_req("A-sys","= 0.0.16"),]),
        pkg!(("BA-sys", "0.0.1") => [dep("bad"),]),
        pkg!(("BA-sys", "0.0.2") => [dep("bad"),]),
        pkg!("nA" => [
            dep("A"),
            dep_req("A-sys", "<= 0.0.3"),
            dep("B-sys"),
            dep("BA-sys"),
        ]),
    ];
    let reg = registry(input);
    let _ = resolve_and_validated(vec![dep("nA")], &reg, None);
}

#[test]
fn bad_lockfile_from_8249() {
    let input = vec![
        pkg!(("a-sys", "0.2.0")),
        pkg!(("a-sys", "0.1.0")),
        pkg!(("b", "0.1.0") => [
            dep_req("a-sys", "0.1"), // should be optional: true, but not deeded for now
        ]),
        pkg!(("c", "1.0.0") => [
            dep_req("b", "=0.1.0"),
        ]),
        pkg!("foo" => [
            dep_req("a-sys", "=0.2.0"),
            {
                let mut b = dep_req("b", "=0.1.0");
                b.set_features(vec!["a-sys"]);
                b
            },
            dep_req("c", "=1.0.0"),
        ]),
    ];
    let reg = registry(input);
    let _ = resolve_and_validated(vec![dep("foo")], &reg, None);
}

#[test]
fn cyclic_good_error_message() {
    let input = vec![
        pkg!(("A", "0.0.0") => [dep("C")]),
        pkg!(("B", "0.0.0") => [dep("C")]),
        pkg!(("C", "0.0.0") => [dep("A")]),
    ];
    let reg = registry(input);
    let error = resolve(vec![dep("A"), dep("B")], &reg).unwrap_err();
    println!("{}", error);
    assert_eq!("\
cyclic package dependency: package `A v0.0.0 (registry `https://example.com/`)` depends on itself. Cycle:
package `A v0.0.0 (registry `https://example.com/`)`
    ... which satisfies dependency `A = \"*\"` of package `C v0.0.0 (registry `https://example.com/`)`
    ... which satisfies dependency `C = \"*\"` of package `A v0.0.0 (registry `https://example.com/`)`\
", error.to_string());
}

#[test]
fn shortest_path_in_error_message() {
    let input = vec![
        pkg!(("F", "0.1.2")),
        pkg!(("F", "0.1.1") => [dep("bad"),]),
        pkg!(("F", "0.1.0") => [dep("bad"),]),
        pkg!("E" => [dep_req("F", "^0.1.2"),]),
        pkg!("D" => [dep_req("F", "^0.1.2"),]),
        pkg!("C" => [dep("D"),]),
        pkg!("A" => [dep("C"),dep("E"),dep_req("F", "<=0.1.1"),]),
    ];
    let error = resolve(vec![dep("A")], &registry(input)).unwrap_err();
    println!("{}", error);
    assert_eq!(
        "\
failed to select a version for `F`.
    ... required by package `A v1.0.0 (registry `https://example.com/`)`
    ... which satisfies dependency `A = \"*\"` of package `root v1.0.0 (registry `https://example.com/`)`
versions that meet the requirements `<=0.1.1` are: 0.1.1, 0.1.0

all possible versions conflict with previously selected packages.

  previously selected package `F v0.1.2 (registry `https://example.com/`)`
    ... which satisfies dependency `F = \"^0.1.2\"` of package `E v1.0.0 (registry `https://example.com/`)`
    ... which satisfies dependency `E = \"*\"` of package `A v1.0.0 (registry `https://example.com/`)`
    ... which satisfies dependency `A = \"*\"` of package `root v1.0.0 (registry `https://example.com/`)`

failed to select a version for `F` which could resolve this conflict\
    ",
        error.to_string()
    );
}
