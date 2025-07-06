use cargo::core::Dependency;
use cargo::core::dependency::DepKind;
use cargo::util::GlobalContext;

use resolver_tests::{
    helpers::{
        ToDep, ToPkgId, assert_contains, assert_same, dep, dep_kind, dep_loc, dep_req, loc_names,
        names, pkg, pkg_dep, pkg_dep_with, pkg_id, pkg_loc, registry,
    },
    pkg, resolve, resolve_with_global_context,
};

#[test]
#[should_panic(expected = "assertion failed: !name.is_empty()")]
fn test_dependency_with_empty_name() {
    // Bug 5229, dependency-names must not be empty
    dep("");
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

    let mut gctx = GlobalContext::default().unwrap();
    // -Z minimal-versions
    // When the minimal-versions config option is specified then the lowest
    // possible version of a package should be selected. "util 1.0.0" can't be
    // selected because of the requirements of "bar", so the minimum version
    // must be 1.1.1.
    gctx.nightly_features_allowed = true;
    gctx.configure(
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

    let res = resolve_with_global_context(
        vec![dep_req("foo", "1.0.0"), dep_req("bar", "1.0.0")],
        &reg,
        &gctx,
    )
    .unwrap()
    .into_iter()
    .map(|(pkg, _)| pkg)
    .collect::<Vec<_>>();

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
            .filter(|&x| package_to_yank != x.package_id())
            .cloned()
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
            .filter(|&x| package_to_yank != x.package_id())
            .cloned()
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
            .filter(|&x| package_to_yank != x.package_id())
            .cloned()
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
fn resolving_slow_case_missing_feature() {
    let mut reg = Vec::new();

    const LAST_CRATE_VERSION_COUNT: usize = 50;

    // increase in resolve time is at least cubic over `INTERMEDIATE_CRATES_VERSION_COUNT`.
    // it should be `>= LAST_CRATE_VERSION_COUNT` to reproduce slowdown.
    const INTERMEDIATE_CRATES_VERSION_COUNT: usize = LAST_CRATE_VERSION_COUNT + 5;

    // should be `>= 2` to reproduce slowdown
    const TRANSITIVE_CRATES_COUNT: usize = 3;

    reg.push(pkg_dep_with(("last", "1.0.0"), vec![], &[("f", &[])]));
    for v in 1..LAST_CRATE_VERSION_COUNT {
        reg.push(pkg(("last", format!("1.0.{v}"))));
    }

    reg.push(pkg_dep(
        ("dep", "1.0.0"),
        vec![
            dep("last"), // <-- needed to reproduce slowdown
            dep_req("intermediate-1", "1.0.0"),
        ],
    ));

    for n in 0..INTERMEDIATE_CRATES_VERSION_COUNT {
        let version = format!("1.0.{n}");
        for c in 1..TRANSITIVE_CRATES_COUNT {
            reg.push(pkg_dep(
                (format!("intermediate-{c}"), &version),
                vec![dep_req(&format!("intermediate-{}", c + 1), &version)],
            ));
        }
        reg.push(pkg_dep(
            (format!("intermediate-{TRANSITIVE_CRATES_COUNT}"), &version),
            vec![dep_req("last", "1.0.0").with(&["f"])],
        ));
    }

    let deps = vec![dep("dep")];
    let _ = resolve(deps, &reg);
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
