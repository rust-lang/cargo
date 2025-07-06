use cargo::core::{Dependency, dependency::DepKind};

use resolver_tests::{
    helpers::{
        ToDep, dep, dep_kind, dep_req, dep_req_kind, pkg, pkg_dep, pkg_dep_link, pkg_dep_with,
        pkg_id_source, registry,
    },
    pkg, resolve, resolve_and_validated, resolve_and_validated_raw,
    sat::SatResolver,
};

#[test]
fn test_01_renamed_package() {
    let reg = registry(vec![
        pkg_dep_with(
            "a",
            vec!["b".opt().rename("b_package")],
            &[("default", &["b_package"])],
        ),
        pkg("b"),
    ]);

    let deps = vec!["a".with(&["default"])];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_ok());
}

#[test]
fn test_02_renamed_package_no_shadowing() {
    let reg = registry(vec![
        pkg("url"),
        pkg_dep("wasmi", vec!["wasmparser-nostd".rename("wasmparser")]),
        pkg_dep("wasmparser", vec!["url".to_dep()]),
    ]);

    let deps = vec![dep("wasmi")];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_err());
}

#[test]
fn test_03_prerelease_semver() {
    let reg = registry(vec![
        pkg!("parking_lot_core" => [dep_req("smallvec", "^1.6.1")]),
        pkg(("smallvec", "2.0.0-alpha.3")),
        pkg_dep_with(
            ("tokio", "1.35.1"),
            vec!["parking_lot_core".opt()],
            &[("default", &["parking_lot_core"])],
        ),
    ]);

    let deps = vec!["tokio".with(&["default"])];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_err());
}

#[test]
fn test_04_cyclic_features() {
    let reg = registry(vec![pkg_dep_with(
        "windows",
        vec![],
        &[
            ("Win32", &["Win32_Foundation"]),
            ("Win32_Foundation", &["Win32"]),
        ],
    )]);

    let deps = vec!["windows".with(&["Win32_Foundation"])];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_ok());
}

#[test]
fn test_05_cyclic_optional_dependencies() {
    let reg = registry(vec![
        pkg_dep("async-global-executor", vec!["io-lifetimes".opt()]),
        pkg_dep(
            "io-lifetimes",
            vec!["test".with(&["async-global-executor"])],
        ),
        pkg_dep_with(
            "test",
            vec!["async-global-executor".opt().with(&["io-lifetimes"])],
            &[],
        ),
    ]);

    let deps = vec![dep("test")];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_ok());
}

#[test]
fn test_06_cyclic_dependencies() {
    let reg = registry(vec![
        pkg(("a", "1.0.0")),
        pkg_dep(("a", "1.0.1"), vec![dep("dep")]),
        pkg_dep("dep", vec![dep("a")]),
    ]);

    let deps = vec![dep("dep")];

    // Cyclic dependencies are not checked in the SAT resolver
    assert!(resolve(deps.clone(), &reg).is_err());
    assert!(SatResolver::new(&reg).sat_resolve(&deps));
}

#[test]
fn test_07_self_dependency() {
    let reg = registry(vec![pkg_dep("dep", vec![dep("dep")])]);

    let deps = vec![dep("dep")];

    // Cyclic dependencies are not checked in the SAT resolver
    assert!(resolve(deps.clone(), &reg).is_err());
    assert!(SatResolver::new(&reg).sat_resolve(&deps));
}

#[test]
fn test_08_activated_optional_self_dependency() {
    let reg = registry(vec![pkg_dep("a", vec!["a".opt()])]);

    let deps = vec!["a".with(&["a"])];

    // Cyclic dependencies are not checked in the SAT resolver
    assert!(resolve(deps.clone(), &reg).is_err());
    assert!(SatResolver::new(&reg).sat_resolve(&deps));
}

#[test]
fn test_09_build_dependency_with_same_name() {
    let reg = registry(vec![
        pkg("memchr"),
        pkg_dep_with(
            ("regex", "1.4.6"),
            vec!["memchr".opt()],
            &[("default", &["memchr"])],
        ),
        pkg_dep("sv-parser", vec!["regex".with(&["default"])]),
        pkg_dep(
            "svlint",
            vec![
                dep_req("regex", "^1.5"),
                dep_req_kind("regex", "^1", DepKind::Build),
                dep("sv-parser"),
            ],
        ),
    ]);

    let deps = vec![dep("svlint")];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_err());
}

#[test]
fn test_10_root_dev_dependency_with_same_name() {
    let reg = registry(vec![pkg(("root", "1.0.1"))]);

    let deps = vec![dep_req_kind("root", "=1.0.1", DepKind::Development).rename("root101")];
    let source = pkg_id_source("root", "https://root.example.com");
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated_raw(deps, &reg, source, &mut sat_resolver).is_ok());
}

#[test]
fn test_11_dev_dependency() {
    let reg = registry(vec![pkg_dep_with(
        "burn-core",
        vec![dep_kind("burn-ndarray", DepKind::Development)],
        &[("default", &["burn-ndarray/std"])],
    )]);

    let deps = vec!["burn-core".with(&["default"])];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_ok());
}

#[test]
fn test_12_weak_dependencies() {
    let reg = registry(vec![
        pkg_dep_with("borsh", vec!["borsh-derive".opt()], &[("std", &[])]),
        pkg_dep_with(
            "rust_decimal",
            vec!["borsh".opt().with(&["borsh-derive"])],
            &[("default", &["borsh?/std"])],
        ),
    ]);

    let deps = vec!["rust_decimal".with(&["default"])];

    // Weak dependencies are not supported yet in the dependency resolver
    assert!(resolve(deps.clone(), &reg).is_err());
    assert!(SatResolver::new(&reg).sat_resolve(&deps));
}

#[test]
fn test_13_weak_dependencies() {
    let reg = registry(vec![
        pkg_dep_with("memchr", vec!["std".opt()], &[("std", &["dep:std"])]),
        pkg_dep_with(
            "winnow",
            vec!["memchr".opt()],
            &[("default", &["memchr?/std"]), ("simd", &["dep:memchr"])],
        ),
    ]);

    let deps = vec!["winnow".with(&["default"])];

    // Weak dependencies are not supported yet in the dependency resolver
    assert!(resolve(deps.clone(), &reg).is_err());
    assert!(SatResolver::new(&reg).sat_resolve(&deps));
}

#[test]
fn test_14_weak_dependencies() {
    let reg = registry(vec![
        pkg_dep("a", vec![dep("bad")]),
        pkg_dep_with("b", vec!["a".opt()], &[("perf-literal", &["dep:a"])]),
        pkg_dep_with(
            "c",
            vec!["b".opt()],
            &[
                ("perf-literal", &["b?/perf-literal"]),
                ("perf-literal-multisubstring", &["dep:b"]),
            ],
        ),
        pkg_dep_with("dep", vec![dep("c")], &[("default", &["c/perf-literal"])]),
    ]);

    let deps = vec!["dep".with(&["default"])];

    // Weak dependencies are not supported yet in the dependency resolver
    assert!(resolve(deps.clone(), &reg).is_err());
    assert!(SatResolver::new(&reg).sat_resolve(&deps));
}

#[test]
fn test_15_duplicate_sys_crate() {
    let reg = registry(vec![
        pkg_dep_link("js", "js", vec![]),
        pkg_dep_link("dep", "js", vec![dep("js")]),
    ]);

    let deps = vec![dep("dep")];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_err());
}

#[test]
fn test_16_missing_optional_dependency() {
    let reg = registry(vec![
        pkg_dep("b", vec!["c".opt()]),
        pkg_dep_with("dep", vec![dep("b")], &[("d", &["b/c"])]),
    ]);

    let deps = vec!["dep".with(&["d"])];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_err());
}

#[test]
fn test_17_feature_shadowing_missing_optional_dependency() {
    let reg = registry(vec![pkg_dep_with(
        "rustix",
        vec!["alloc".opt()],
        &[
            ("alloc", &[]),
            ("default", &["alloc"]),
            ("rustc-dep-of-std", &["dep:alloc"]),
        ],
    )]);

    let deps = vec!["rustix".with(&["default"])];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_ok());
}

#[test]
fn test_18_feature_shadowing_activated_optional_dependency() {
    let reg = registry(vec![
        pkg_dep("alloc", vec![dep("bad")]),
        pkg_dep_with(
            "rustix",
            vec!["alloc".opt()],
            &[
                ("alloc", &[]),
                ("default", &["dep:alloc"]),
                ("rustc-dep-of-std", &["alloc"]),
            ],
        ),
    ]);

    let deps = vec!["rustix".with(&["default"])];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_err());
}

#[test]
fn test_19_same_dep_twice_feature_unification() {
    let reg = registry(vec![
        pkg_dep_with(
            "iced",
            vec!["iced_wgpu".opt(), "iced_wgpu".opt().with(&["webgl"])],
            &[("default", &["iced_wgpu"])],
        ),
        pkg_dep_with("iced_wgpu", vec![], &[("webgl", &[])]),
    ]);

    let deps = vec!["iced".with(&["default"])];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_ok());
}

#[test]
fn test_20_no_implicit_feature() {
    let reg = registry(vec![
        pkg("c"),
        pkg_dep_with("ureq", vec!["c".opt()], &[("cookies", &["dep:c"])]),
        pkg_dep_with("dep", vec![dep("ureq")], &[("cookies", &["ureq/c"])]),
    ]);

    let deps = vec!["dep".with(&["cookies"])];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_err());
}

#[test]
fn test_21_implicit_feature() {
    let reg = registry(vec![
        pkg("c"),
        pkg_dep("ureq", vec!["c".opt()]),
        pkg_dep_with("dep", vec![dep("ureq")], &[("cookies", &["ureq/c"])]),
    ]);

    let deps = vec!["dep".with(&["cookies"])];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_ok());
}

#[test]
fn test_22_missing_explicit_default_feature() {
    let reg = registry(vec![
        pkg_dep_with(
            "fuel-tx",
            vec![dep("serde"), "serde_json".opt()],
            &[("default", &["serde/default"]), ("serde", &["serde_json"])],
        ),
        pkg("serde"),
    ]);

    let deps = vec!["fuel-tx".with(&["default"])];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_err());
}

#[test]
fn test_23_no_need_for_explicit_default_feature() {
    let reg = registry(vec![
        pkg("a"),
        pkg_dep_with(
            "b",
            vec!["a".with_default()],
            &[("default", &["std"]), ("std", &[])],
        ),
    ]);

    let deps = vec!["b".with(&["default"])];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_ok());
}

#[test]
fn test_24_dep_feature() {
    let reg = registry(vec![
        pkg_dep_with("proc-macro2", vec![], &[("proc-macro", &[])]),
        pkg_dep_with(
            "syn",
            vec![dep("proc-macro2")],
            &[("proc-macro", &["proc-macro2/proc-macro"])],
        ),
        pkg_dep("serde_derive", vec!["syn".with(&["proc-macro"])]),
    ]);

    let deps = vec![dep("serde_derive")];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_ok());
}

#[test]
fn test_25_dep_feature() {
    let reg = registry(vec![
        pkg_dep_with("proc-macro2", vec![], &[("proc-macro", &[])]),
        pkg_dep_with(
            "syn",
            vec![dep("proc-macro2")],
            &[("proc-macro", &["proc-macro2/proc-macro"])],
        ),
    ]);

    let deps = vec!["syn".with(&["proc-macro"])];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_ok());
}

#[test]
fn test_26_implicit_feature_with_dep_feature() {
    let reg = registry(vec![
        pkg_dep_with("quote", vec![], &[("proc-macro", &[])]),
        pkg_dep_with(
            "syn",
            vec!["quote".opt()],
            &[("default", &["quote", "quote/proc-macro"])],
        ),
    ]);

    let deps = vec!["syn".with(&["default"])];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_ok());
}

#[test]
fn test_27_dep_feature_activating_shadowing_feature() {
    let reg = registry(vec![
        pkg_dep_with(
            "a",
            vec!["b".opt(), "x".opt()],
            &[("b", &["x"]), ("default", &["b/native"])],
        ),
        pkg_dep_with("b", vec![], &[("native", &[])]),
    ]);

    let deps = vec!["a".with(&["default"])];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_err());
}

#[test]
fn test_28_dep_feature_not_activating_shadowing_feature() {
    let reg = registry(vec![
        pkg_dep_with(
            "fuel-tx",
            vec![dep("serde"), "serde_json".opt()],
            &[("default", &["serde/default"]), ("serde", &["serde_json"])],
        ),
        pkg_dep_with("serde", vec![], &[("default", &[])]),
    ]);

    let deps = vec!["fuel-tx".with(&["default"])];
    let mut sat_resolver = SatResolver::new(&reg);
    assert!(resolve_and_validated(deps, &reg, &mut sat_resolver).is_ok());
}
