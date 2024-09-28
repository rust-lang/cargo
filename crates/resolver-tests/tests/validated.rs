use cargo::core::Dependency;

use resolver_tests::{
    helpers::{dep, dep_req, pkg, pkg_dep, pkg_dep_with, registry, ToDep},
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
            vec!["a".to_opt_dep(), "b".to_opt_dep(), "jpg".to_dep()],
            &[("default", &["a"]), ("b", &["dep:b"])],
        ),
        pkg!("jpg"),
        pkg!("log"),
        pkg!("man"),
        pkg_dep_with("rgb", vec!["man".to_opt_dep()], &[("man", &["dep:man"])]),
        pkg_dep_with(
            "dep",
            vec![
                "image".to_dep_with(&["b"]),
                "log".to_opt_dep(),
                "rgb".to_opt_dep(),
            ],
            &[
                ("default", &["log", "image/default"]),
                ("man", &["rgb?/man"]),
            ],
        ),
    ]);

    for deps in [
        vec!["dep".to_dep_with(&["default", "man", "log", "rgb"])],
        vec!["dep".to_dep_with(&["default"])],
        vec!["dep".to_dep_with(&[])],
        vec!["dep".to_dep_with(&["man"])],
        vec!["dep".to_dep_with(&["man", "rgb"])],
    ] {
        let mut sat_resolver = SatResolver::new(&reg);
        assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_ok());
    }
}

#[test]
fn missing_feature() {
    let reg = registry(vec![pkg!("a")]);
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(vec!["a".to_dep_with(&["f"])], &reg, &mut sat_resolver).is_err());
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

    let deps = vec![dep_req("a", "*").to_dep_with(&["f"]), dep("dep")];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_err());
}

#[test]
fn conflict_weak_features() {
    let reg = registry(vec![
        pkg(("a-sys", "1.0.0")),
        pkg(("a-sys", "2.0.0")),
        pkg_dep("a1", vec![dep_req("a-sys", "1.0.0").to_opt_dep()]),
        pkg_dep("a2", vec![dep_req("a-sys", "2.0.0").to_opt_dep()]),
        pkg_dep_with(
            "dep",
            vec!["a1".to_opt_dep(), "a2".to_opt_dep()],
            &[("a1", &["a1?/a-sys"]), ("a2", &["a2?/a-sys"])],
        ),
    ]);

    let deps = vec![dep("dep").to_dep_with(&["a1", "a2"])];

    // The following asserts should be updated when support for weak dependencies
    // is added to the dependency resolver.
    assert!(resolve(deps.clone(), &reg).is_err());
    assert!(SatResolver::new(&reg).sat_resolve(&deps));
}
