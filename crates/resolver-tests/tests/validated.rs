use cargo::core::{Dependency, dependency::DepKind};

use resolver_tests::{
    helpers::{
        ToDep, dep, dep_kind, dep_platform, dep_req, dep_req_kind, dep_req_platform, pkg, pkg_dep,
        pkg_dep_with, registry,
    },
    pkg, resolve, resolve_and_validated,
    sat::SatResolver,
};

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
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(vec![dep("f")], &reg, &mut sat_resolver).is_ok());
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
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(vec![dep("j")], &reg, &mut sat_resolver).is_err());
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
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(vec![dep("nA")], &reg, &mut sat_resolver).is_err());
}

#[test]
fn bad_lockfile_from_8249() {
    let input = vec![
        pkg!(("a-sys", "0.2.0")),
        pkg!(("a-sys", "0.1.0")),
        pkg!(("b", "0.1.0") => [
            dep_req("a-sys", "0.1"), // should be optional: true, but not needed for now
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
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(vec![dep("foo")], &reg, &mut sat_resolver).is_err());
}

#[test]
fn registry_with_features() {
    let reg = registry(vec![
        pkg!("a"),
        pkg!("b"),
        pkg_dep_with(
            "image",
            vec!["a".opt(), "b".opt(), "jpg".to_dep()],
            &[("default", &["a"]), ("b", &["dep:b"])],
        ),
        pkg!("jpg"),
        pkg!("log"),
        pkg!("man"),
        pkg_dep_with("rgb", vec!["man".opt()], &[("man", &["dep:man"])]),
        pkg_dep_with(
            "dep",
            vec!["image".with(&["b"]), "log".opt(), "rgb".opt()],
            &[
                ("default", &["log", "image/default"]),
                ("man", &["rgb?/man"]),
            ],
        ),
    ]);

    for deps in [
        vec!["dep".with(&["default", "man", "log", "rgb"])],
        vec!["dep".with(&["default"])],
        vec!["dep".with(&[])],
        vec!["dep".with(&["man"])],
        vec!["dep".with(&["man", "rgb"])],
    ] {
        let mut sat_resolver = SatResolver::new(&reg);
        assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_ok());
    }
}

#[test]
fn missing_feature() {
    let reg = registry(vec![pkg!("a")]);
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(vec!["a".with(&["f"])], &reg, &mut sat_resolver).is_err());
}

#[test]
fn missing_dep_feature() {
    let reg = registry(vec![
        pkg("a"),
        pkg_dep_with("dep", vec![dep("a")], &[("f", &["a/a"])]),
    ]);

    let deps = vec![dep("dep").with(&["f"])];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_err());
}

#[test]
fn missing_weak_dep_feature() {
    let reg = registry(vec![
        pkg("a"),
        pkg_dep_with("dep1", vec![dep("a")], &[("f", &["a/a"])]),
        pkg_dep_with("dep2", vec!["a".opt()], &[("f", &["a/a"])]),
        pkg_dep_with("dep3", vec!["a".opt()], &[("f", &["a?/a"])]),
        pkg_dep_with("dep4", vec!["x".opt()], &[("f", &["x?/a"])]),
    ]);

    let deps = vec![dep("dep1").with(&["f"])];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_err());

    let deps = vec![dep("dep2").with(&["f"])];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_err());

    let deps = vec![dep("dep2").with(&["a", "f"])];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_err());

    // Weak dependencies are not supported yet in the dependency resolver
    let deps = vec![dep("dep3").with(&["f"])];
    assert!(resolve(deps.clone(), &reg).is_err());
    assert!(SatResolver::new(&reg).sat_resolve(&deps));

    let deps = vec![dep("dep3").with(&["a", "f"])];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_err());

    // Weak dependencies are not supported yet in the dependency resolver
    let deps = vec![dep("dep4").with(&["f"])];
    assert!(resolve(deps.clone(), &reg).is_err());
    assert!(SatResolver::new(&reg).sat_resolve(&deps));

    let deps = vec![dep("dep4").with(&["x", "f"])];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_err());
}

#[test]
fn conflict_feature_and_sys() {
    let reg = registry(vec![
        pkg(("a-sys", "1.0.0")),
        pkg(("a-sys", "2.0.0")),
        pkg_dep_with(
            ("a", "1.0.0"),
            vec![dep_req("a-sys", "1.0.0")],
            &[("f", &[])],
        ),
        pkg_dep_with(
            ("a", "2.0.0"),
            vec![dep_req("a-sys", "2.0.0")],
            &[("g", &[])],
        ),
        pkg_dep("dep", vec![dep_req("a", "2.0.0")]),
    ]);

    let deps = vec![dep_req("a", "*").with(&["f"]), dep("dep")];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_err());
}

#[test]
fn conflict_weak_features() {
    let reg = registry(vec![
        pkg(("a-sys", "1.0.0")),
        pkg(("a-sys", "2.0.0")),
        pkg_dep("a1", vec![dep_req("a-sys", "1.0.0").opt()]),
        pkg_dep("a2", vec![dep_req("a-sys", "2.0.0").opt()]),
        pkg_dep_with(
            "dep",
            vec!["a1".opt(), "a2".opt()],
            &[("a1", &["a1?/a-sys"]), ("a2", &["a2?/a-sys"])],
        ),
    ]);

    let deps = vec![dep("dep").with(&["a1", "a2"])];

    // Weak dependencies are not supported yet in the dependency resolver
    assert!(resolve(deps.clone(), &reg).is_err());
    assert!(SatResolver::new(&reg).sat_resolve(&deps));
}

#[test]
fn multiple_dep_kinds_and_targets() {
    let reg = registry(vec![
        pkg(("a-sys", "1.0.0")),
        pkg(("a-sys", "2.0.0")),
        pkg_dep_with(
            "dep1",
            vec![
                dep_req_platform("a-sys", "1.0.0", "cfg(all())").opt(),
                dep_req("a-sys", "1.0.0").opt(),
                dep_req_kind("a-sys", "2.0.0", DepKind::Build).opt(),
            ],
            &[("default", &["dep:a-sys"])],
        ),
        pkg_dep_with(
            "dep2",
            vec![
                dep_req_platform("a-sys", "1.0.0", "cfg(all())").opt(),
                dep_req("a-sys", "1.0.0").opt(),
                dep_req_kind("a-sys", "2.0.0", DepKind::Development).rename("a-sys-dev"),
            ],
            &[("default", &["dep:a-sys", "a-sys-dev/bad"])],
        ),
    ]);

    let deps = vec![dep("dep1")];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_err());

    let deps = vec![dep("dep2")];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_ok());

    let deps = vec![
        dep_req("a-sys", "1.0.0"),
        dep_req_kind("a-sys", "2.0.0", DepKind::Build).rename("a2"),
    ];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_err());

    let deps = vec![
        dep_req("a-sys", "1.0.0"),
        dep_req_kind("a-sys", "2.0.0", DepKind::Development).rename("a2"),
    ];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_err());
}

#[test]
fn multiple_dep_kinds_and_targets_with_different_packages() {
    let reg = registry(vec![
        pkg_dep_with("a", vec![], &[("f", &[])]),
        pkg_dep_with("b", vec![], &[("f", &[])]),
        pkg_dep_with("c", vec![], &[("g", &[])]),
        pkg_dep_with(
            "dep1",
            vec![
                "a".opt().rename("x").with(&["f"]),
                dep_platform("a", "cfg(all())").opt().rename("x"),
                dep_kind("b", DepKind::Build).opt().rename("x").with(&["f"]),
            ],
            &[("default", &["x"])],
        ),
        pkg_dep_with(
            "dep2",
            vec![
                "a".opt().rename("x").with(&["f"]),
                dep_platform("a", "cfg(all())").opt().rename("x"),
                dep_kind("c", DepKind::Build).opt().rename("x").with(&["f"]),
            ],
            &[("default", &["x"])],
        ),
    ]);

    let deps = vec!["dep1".with(&["default"])];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_ok());

    let deps = vec!["dep2".with(&["default"])];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_err());
}

#[test]
fn dep_feature_with_shadowing_feature() {
    let reg = registry(vec![
        pkg_dep_with("a", vec![], &[("b", &[])]),
        pkg_dep_with(
            "dep",
            vec!["a".opt().rename("aa"), "c".opt()],
            &[("default", &["aa/b"]), ("aa", &["c"])],
        ),
    ]);

    let deps = vec!["dep".with(&["default"])];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_err());
}

#[test]
fn dep_feature_not_optional_with_shadowing_feature() {
    let reg = registry(vec![
        pkg_dep_with("a", vec![], &[("b", &[])]),
        pkg_dep_with(
            "dep",
            vec!["a".rename("aa"), "c".opt()],
            &[("default", &["aa/b"]), ("aa", &["c"])],
        ),
    ]);

    let deps = vec!["dep".with(&["default"])];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_ok());
}

#[test]
fn dep_feature_weak_with_shadowing_feature() {
    let reg = registry(vec![
        pkg_dep_with("a", vec![], &[("b", &[])]),
        pkg_dep_with(
            "dep",
            vec!["a".opt().rename("aa"), "c".opt()],
            &[("default", &["aa?/b"]), ("aa", &["c"])],
        ),
    ]);

    let deps = vec!["dep".with(&["default"])];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_ok());
}

#[test]
fn dep_feature_duplicate_with_shadowing_feature() {
    let reg = registry(vec![
        pkg_dep_with("a", vec![], &[("b", &[])]),
        pkg_dep_with(
            "dep",
            vec![
                "a".opt().rename("aa"),
                dep_kind("a", DepKind::Build).rename("aa"),
                "c".opt(),
            ],
            &[("default", &["aa/b"]), ("aa", &["c"])],
        ),
    ]);

    let deps = vec!["dep".with(&["default"])];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_err());
}

#[test]
fn optional_dep_features() {
    let reg = registry(vec![
        pkg_dep("a", vec!["bad".opt()]),
        pkg_dep("b", vec!["a".opt().with(&["bad"])]),
        pkg_dep("dep", vec![dep("a"), dep("b")]),
    ]);

    let deps = vec![dep("dep")];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_ok());
}

#[test]
fn optional_dep_features_with_rename() {
    let reg = registry(vec![
        pkg("x1"),
        pkg_dep("a", vec!["x1".opt(), "x2".opt(), "x3".opt()]),
        pkg_dep(
            "dep1",
            vec![
                "a".opt().with(&["x1"]),
                dep_kind("a", DepKind::Build).opt().with(&["x2"]),
            ],
        ),
        pkg_dep(
            "dep2",
            vec![
                "a".opt().with(&["x1"]),
                "a".opt().rename("a2").with(&["x3"]),
            ],
        ),
    ]);

    let deps = vec!["dep1".with(&["a"])];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_err());

    let deps = vec!["dep2".with(&["a"])];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_ok());
}

#[test]
fn optional_weak_dep_features() {
    let reg = registry(vec![
        pkg_dep("a", vec!["bad".opt()]),
        pkg_dep("b", vec![dep("a")]),
        pkg_dep_with("dep", vec!["a".opt(), dep("b")], &[("f", &["a?/bad"])]),
    ]);

    let deps = vec!["dep".with(&["f"])];

    // Weak dependencies are not supported yet in the dependency resolver
    assert!(resolve(deps.clone(), &reg).is_err());
    assert!(SatResolver::new(&reg).sat_resolve(&deps));
}

#[test]
fn default_feature_multiple_major_versions() {
    let reg = registry(vec![
        pkg_dep_with(("a", "0.2.0"), vec![], &[("default", &[])]),
        pkg(("a", "0.3.0")),
        pkg_dep_with(("a", "0.4.0"), vec![], &[("default", &[])]),
        pkg_dep(
            "dep1",
            vec![
                dep_req("a", ">=0.2, <0.4").with_default(),
                dep_req("a", "0.2").rename("a2").with(&[]),
            ],
        ),
        pkg_dep(
            "dep2",
            vec![
                dep_req("a", ">=0.2, <0.4").with_default(),
                dep_req("a", "0.3").rename("a2").with(&[]),
            ],
        ),
        pkg_dep(
            "dep3",
            vec![
                dep_req("a", ">=0.2, <0.4").with_default(),
                dep_req("a", "0.2").rename("a1").with(&[]),
                dep_req("a", "0.3").rename("a2").with(&[]),
            ],
        ),
        pkg_dep("dep4", vec![dep_req("a", ">=0.2, <0.4").with_default()]),
        pkg_dep("dep5", vec![dep_req("a", ">=0.3, <0.5").with_default()]),
    ]);

    let deps = vec![dep("dep1")];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_ok());

    let deps = vec![dep("dep2")];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_ok());

    let deps = vec![dep("dep3")];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_ok());

    let deps = vec![dep("dep4")];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_ok());

    let deps = vec![dep("dep5")];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_ok());
}
