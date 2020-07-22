//! Tests for the new feature resolver.

use cargo_test_support::cross_compile::{self, alternate};
use cargo_test_support::paths::CargoPathExt;
use cargo_test_support::publish::validate_crate_contents;
use cargo_test_support::registry::{Dependency, Package};
use cargo_test_support::{basic_manifest, cargo_process, project, rustc_host};
use std::fs::File;

#[cargo_test]
fn inactivate_targets() {
    // Basic test of `itarget`. A shared dependency where an inactive [target]
    // changes the features.
    Package::new("common", "1.0.0")
        .feature("f1", &[])
        .file(
            "src/lib.rs",
            r#"
            #[cfg(feature = "f1")]
            compile_error!("f1 should not activate");
            "#,
        )
        .publish();

    Package::new("bar", "1.0.0")
        .add_dep(
            Dependency::new("common", "1.0")
                .target("cfg(whatever)")
                .enable_features(&["f1"]),
        )
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            common = "1.0"
            bar = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_contains("[..]f1 should not activate[..]")
        .run();

    p.cargo("check -Zfeatures=itarget")
        .masquerade_as_nightly_cargo()
        .run();
}

#[cargo_test]
fn inactive_target_optional() {
    // Activating optional [target] dependencies for inactivate target.
    Package::new("common", "1.0.0")
        .feature("f1", &[])
        .feature("f2", &[])
        .feature("f3", &[])
        .feature("f4", &[])
        .file(
            "src/lib.rs",
            r#"
            pub fn f() {
                if cfg!(feature="f1") { println!("f1"); }
                if cfg!(feature="f2") { println!("f2"); }
                if cfg!(feature="f3") { println!("f3"); }
                if cfg!(feature="f4") { println!("f4"); }
            }
            "#,
        )
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2018"

            [dependencies]
            common = "1.0"

            [target.'cfg(whatever)'.dependencies]
            dep1 = {path='dep1', optional=true}
            dep2 = {path='dep2', optional=true, features=["f3"]}
            common = {version="1.0", optional=true, features=["f4"]}

            [features]
            foo1 = ["dep1/f2"]
            "#,
        )
        .file(
            "src/main.rs",
            r#"
            fn main() {
                if cfg!(feature="foo1") { println!("foo1"); }
                if cfg!(feature="dep1") { println!("dep1"); }
                if cfg!(feature="dep2") { println!("dep2"); }
                if cfg!(feature="common") { println!("common"); }
                common::f();
            }
            "#,
        )
        .file(
            "dep1/Cargo.toml",
            r#"
            [package]
            name = "dep1"
            version = "0.1.0"

            [dependencies]
            common = {version="1.0", features=["f1"]}

            [features]
            f2 = ["common/f2"]
            "#,
        )
        .file(
            "dep1/src/lib.rs",
            r#"compile_error!("dep1 should not build");"#,
        )
        .file(
            "dep2/Cargo.toml",
            r#"
            [package]
            name = "dep2"
            version = "0.1.0"

            [dependencies]
            common = "1.0"

            [features]
            f3 = ["common/f3"]
            "#,
        )
        .file(
            "dep2/src/lib.rs",
            r#"compile_error!("dep2 should not build");"#,
        )
        .build();

    p.cargo("run --all-features")
        .with_stdout("foo1\ndep1\ndep2\ncommon\nf1\nf2\nf3\nf4\n")
        .run();
    p.cargo("run --features dep1")
        .with_stdout("dep1\nf1\n")
        .run();
    p.cargo("run --features foo1")
        .with_stdout("foo1\ndep1\nf1\nf2\n")
        .run();
    p.cargo("run --features dep2")
        .with_stdout("dep2\nf3\n")
        .run();
    p.cargo("run --features common")
        .with_stdout("common\nf4\n")
        .run();

    p.cargo("run -Zfeatures=itarget --all-features")
        .masquerade_as_nightly_cargo()
        .with_stdout("foo1\n")
        .run();
    p.cargo("run -Zfeatures=itarget --features dep1")
        .masquerade_as_nightly_cargo()
        .with_stdout("dep1\n")
        .run();
    p.cargo("run -Zfeatures=itarget --features foo1")
        .masquerade_as_nightly_cargo()
        .with_stdout("foo1\n")
        .run();
    p.cargo("run -Zfeatures=itarget --features dep2")
        .masquerade_as_nightly_cargo()
        .with_stdout("dep2\n")
        .run();
    p.cargo("run -Zfeatures=itarget --features common")
        .masquerade_as_nightly_cargo()
        .with_stdout("common")
        .run();
}

#[cargo_test]
fn itarget_proc_macro() {
    // itarget inside a proc-macro while cross-compiling
    if cross_compile::disabled() {
        return;
    }
    Package::new("hostdep", "1.0.0").publish();
    Package::new("pm", "1.0.0")
        .proc_macro(true)
        .target_dep("hostdep", "1.0", &rustc_host())
        .file("src/lib.rs", "extern crate hostdep;")
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            pm = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check").run();
    p.cargo("check -Zfeatures=itarget")
        .masquerade_as_nightly_cargo()
        .run();
    p.cargo("check --target").arg(alternate()).run();
    p.cargo("check -Zfeatures=itarget --target")
        .arg(alternate())
        .masquerade_as_nightly_cargo()
        .run();
    // For good measure, just make sure things don't break.
    p.cargo("check -Zfeatures=all --target")
        .arg(alternate())
        .masquerade_as_nightly_cargo()
        .run();
}

#[cargo_test]
fn decouple_host_deps() {
    // Basic test for `host_dep` decouple.
    Package::new("common", "1.0.0")
        .feature("f1", &[])
        .file(
            "src/lib.rs",
            r#"
            #[cfg(feature = "f1")]
            pub fn foo() {}
            #[cfg(not(feature = "f1"))]
            pub fn bar() {}
            "#,
        )
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2018"

            [build-dependencies]
            common = {version="1.0", features=["f1"]}

            [dependencies]
            common = "1.0"
            "#,
        )
        .file(
            "build.rs",
            r#"
            use common::foo;
            fn main() {}
            "#,
        )
        .file("src/lib.rs", "use common::bar;")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_contains("[..]unresolved import `common::bar`[..]")
        .run();

    p.cargo("check -Zfeatures=host_dep")
        .masquerade_as_nightly_cargo()
        .run();
}

#[cargo_test]
fn decouple_host_deps_nested() {
    // `host_dep` decouple of transitive dependencies.
    Package::new("common", "1.0.0")
        .feature("f1", &[])
        .file(
            "src/lib.rs",
            r#"
            #[cfg(feature = "f1")]
            pub fn foo() {}
            #[cfg(not(feature = "f1"))]
            pub fn bar() {}
            "#,
        )
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2018"

            [build-dependencies]
            bdep = {path="bdep"}

            [dependencies]
            common = "1.0"
            "#,
        )
        .file(
            "build.rs",
            r#"
            use bdep::foo;
            fn main() {}
            "#,
        )
        .file("src/lib.rs", "use common::bar;")
        .file(
            "bdep/Cargo.toml",
            r#"
            [package]
            name = "bdep"
            version = "0.1.0"
            edition = "2018"

            [dependencies]
            common = {version="1.0", features=["f1"]}
            "#,
        )
        .file("bdep/src/lib.rs", "pub use common::foo;")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_contains("[..]unresolved import `common::bar`[..]")
        .run();

    p.cargo("check -Zfeatures=host_dep")
        .masquerade_as_nightly_cargo()
        .run();
}

#[cargo_test]
fn decouple_dev_deps() {
    // Basic test for `dev_dep` decouple.
    Package::new("common", "1.0.0")
        .feature("f1", &[])
        .feature("f2", &[])
        .file(
            "src/lib.rs",
            r#"
            // const ensures it uses the correct dependency at *build time*
            // compared to *link time*.
            #[cfg(all(feature="f1", not(feature="f2")))]
            pub const X: u32 = 1;

            #[cfg(all(feature="f1", feature="f2"))]
            pub const X: u32 = 3;

            pub fn foo() -> u32 {
                let mut res = 0;
                if cfg!(feature = "f1") {
                    res |= 1;
                }
                if cfg!(feature = "f2") {
                    res |= 2;
                }
                res
            }
            "#,
        )
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2018"

            [dependencies]
            common = {version="1.0", features=["f1"]}

            [dev-dependencies]
            common = {version="1.0", features=["f2"]}
            "#,
        )
        .file(
            "src/main.rs",
            r#"
            fn main() {
                let expected: u32 = std::env::args().skip(1).next().unwrap().parse().unwrap();
                assert_eq!(foo::foo(), expected);
                assert_eq!(foo::build_time(), expected);
                assert_eq!(common::foo(), expected);
                assert_eq!(common::X, expected);
            }

            #[test]
            fn test_bin() {
                assert_eq!(foo::foo(), 3);
                assert_eq!(common::foo(), 3);
                assert_eq!(common::X, 3);
                assert_eq!(foo::build_time(), 3);
            }
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
            pub fn foo() -> u32 {
                common::foo()
            }

            pub fn build_time() -> u32 {
                common::X
            }

            #[test]
            fn test_lib() {
                assert_eq!(foo(), 3);
                assert_eq!(common::foo(), 3);
                assert_eq!(common::X, 3);
            }
            "#,
        )
        .file(
            "tests/t1.rs",
            r#"
            #[test]
            fn test_t1() {
                assert_eq!(foo::foo(), 3);
                assert_eq!(common::foo(), 3);
                assert_eq!(common::X, 3);
                assert_eq!(foo::build_time(), 3);
            }

            #[test]
            fn test_main() {
                // Features are unified for main when run with `cargo test`,
                // even with -Zfeatures=dev_dep.
                let s = std::process::Command::new("target/debug/foo")
                    .arg("3")
                    .status().unwrap();
                assert!(s.success());
            }
            "#,
        )
        .build();

    p.cargo("run 3").run();

    p.cargo("run -Zfeatures=dev_dep 1")
        .masquerade_as_nightly_cargo()
        .run();

    p.cargo("test").run();

    p.cargo("test -Zfeatures=dev_dep")
        .masquerade_as_nightly_cargo()
        .run();
}

#[cargo_test]
fn build_script_runtime_features() {
    // Check that the CARGO_FEATURE_* environment variable is set correctly.
    //
    // This has a common dependency between build/normal/dev-deps, and it
    // queries which features it was built with in different circumstances.
    Package::new("common", "1.0.0")
        .feature("normal", &[])
        .feature("dev", &[])
        .feature("build", &[])
        .file(
            "build.rs",
            r#"
            fn is_set(name: &str) -> bool {
                std::env::var(name) == Ok("1".to_string())
            }

            fn main() {
                let mut res = 0;
                if is_set("CARGO_FEATURE_NORMAL") {
                    res |= 1;
                }
                if is_set("CARGO_FEATURE_DEV") {
                    res |= 2;
                }
                if is_set("CARGO_FEATURE_BUILD") {
                    res |= 4;
                }
                println!("cargo:rustc-cfg=RunCustomBuild=\"{}\"", res);

                let mut res = 0;
                if cfg!(feature = "normal") {
                    res |= 1;
                }
                if cfg!(feature = "dev") {
                    res |= 2;
                }
                if cfg!(feature = "build") {
                    res |= 4;
                }
                println!("cargo:rustc-cfg=CustomBuild=\"{}\"", res);
            }
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
            pub fn foo() -> u32 {
                let mut res = 0;
                if cfg!(feature = "normal") {
                    res |= 1;
                }
                if cfg!(feature = "dev") {
                    res |= 2;
                }
                if cfg!(feature = "build") {
                    res |= 4;
                }
                res
            }

            pub fn build_time() -> u32 {
                #[cfg(RunCustomBuild="1")] return 1;
                #[cfg(RunCustomBuild="3")] return 3;
                #[cfg(RunCustomBuild="4")] return 4;
                #[cfg(RunCustomBuild="5")] return 5;
                #[cfg(RunCustomBuild="7")] return 7;
            }
            "#,
        )
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2018"

            [build-dependencies]
            common = {version="1.0", features=["build"]}

            [dependencies]
            common = {version="1.0", features=["normal"]}

            [dev-dependencies]
            common = {version="1.0", features=["dev"]}
            "#,
        )
        .file(
            "build.rs",
            r#"
            fn main() {
                assert_eq!(common::foo(), common::build_time());
                println!("cargo:rustc-cfg=from_build=\"{}\"", common::foo());
            }
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
            pub fn foo() -> u32 {
                common::foo()
            }

            pub fn build_time() -> u32 {
                common::build_time()
            }

            #[test]
            fn test_lib() {
                assert_eq!(common::foo(), common::build_time());
                assert_eq!(common::foo(),
                    std::env::var("CARGO_FEATURE_EXPECT").unwrap().parse().unwrap());
            }
            "#,
        )
        .file(
            "src/main.rs",
            r#"
            fn main() {
                assert_eq!(common::foo(), common::build_time());
                assert_eq!(common::foo(),
                    std::env::var("CARGO_FEATURE_EXPECT").unwrap().parse().unwrap());
            }

            #[test]
            fn test_bin() {
                assert_eq!(common::foo(), common::build_time());
                assert_eq!(common::foo(),
                    std::env::var("CARGO_FEATURE_EXPECT").unwrap().parse().unwrap());
            }
            "#,
        )
        .file(
            "tests/t1.rs",
            r#"
            #[test]
            fn test_t1() {
                assert_eq!(common::foo(), common::build_time());
                assert_eq!(common::foo(),
                    std::env::var("CARGO_FEATURE_EXPECT").unwrap().parse().unwrap());
            }

            #[test]
            fn test_main() {
                // Features are unified for main when run with `cargo test`,
                // even with -Zfeatures=dev_dep.
                let s = std::process::Command::new("target/debug/foo")
                    .status().unwrap();
                assert!(s.success());
            }
            "#,
        )
        .build();

    // Old way, unifies all 3.
    p.cargo("run").env("CARGO_FEATURE_EXPECT", "7").run();

    // normal + build unify
    p.cargo("run -Zfeatures=dev_dep")
        .env("CARGO_FEATURE_EXPECT", "5")
        .masquerade_as_nightly_cargo()
        .run();

    // Normal only.
    p.cargo("run -Zfeatures=dev_dep,host_dep")
        .env("CARGO_FEATURE_EXPECT", "1")
        .masquerade_as_nightly_cargo()
        .run();

    p.cargo("test").env("CARGO_FEATURE_EXPECT", "7").run();

    // dev_deps are still unified with `cargo test`
    p.cargo("test -Zfeatures=dev_dep")
        .env("CARGO_FEATURE_EXPECT", "7")
        .masquerade_as_nightly_cargo()
        .run();

    // normal + dev unify
    p.cargo("test -Zfeatures=host_dep")
        .env("CARGO_FEATURE_EXPECT", "3")
        .masquerade_as_nightly_cargo()
        .run();
}

#[cargo_test]
fn cyclical_dev_dep() {
    // Check how a cyclical dev-dependency will work.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2018"

            [features]
            dev = []

            [dev-dependencies]
            foo = { path = '.', features = ["dev"] }
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
            pub fn assert_dev(enabled: bool) {
                assert_eq!(enabled, cfg!(feature="dev"));
            }

            #[test]
            fn test_in_lib() {
                assert_dev(true);
            }
            "#,
        )
        .file(
            "src/main.rs",
            r#"
            fn main() {
                let expected: bool = std::env::args().skip(1).next().unwrap().parse().unwrap();
                foo::assert_dev(expected);
            }
            "#,
        )
        .file(
            "tests/t1.rs",
            r#"
            #[test]
            fn integration_links() {
                foo::assert_dev(true);
                // The lib linked with main.rs will also be unified.
                let s = std::process::Command::new("target/debug/foo")
                    .arg("true")
                    .status().unwrap();
                assert!(s.success());
            }
            "#,
        )
        .build();

    // Old way unifies features.
    p.cargo("run true").run();

    // Should decouple main.
    p.cargo("run -Zfeatures=dev_dep false")
        .masquerade_as_nightly_cargo()
        .run();

    // dev feature should always be enabled in tests.
    p.cargo("test").run();

    // And this should be no different.
    p.cargo("test -Zfeatures=dev_dep")
        .masquerade_as_nightly_cargo()
        .run();
}

#[cargo_test]
fn all_feature_opts() {
    // All feature options at once.
    Package::new("common", "1.0.0")
        .feature("normal", &[])
        .feature("build", &[])
        .feature("dev", &[])
        .feature("itarget", &[])
        .file(
            "src/lib.rs",
            r#"
            pub fn feats() -> u32 {
                let mut res = 0;
                if cfg!(feature="normal") { res |= 1; }
                if cfg!(feature="build") { res |= 2; }
                if cfg!(feature="dev") { res |= 4; }
                if cfg!(feature="itarget") { res |= 8; }
                res
            }
            "#,
        )
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2018"

            [dependencies]
            common = {version = "1.0", features=["normal"]}

            [dev-dependencies]
            common = {version = "1.0", features=["dev"]}

            [build-dependencies]
            common = {version = "1.0", features=["build"]}

            [target.'cfg(whatever)'.dependencies]
            common = {version = "1.0", features=["itarget"]}
            "#,
        )
        .file(
            "src/main.rs",
            r#"
            fn main() {
                expect();
            }

            fn expect() {
                let expected: u32 = std::env::var("EXPECTED_FEATS").unwrap().parse().unwrap();
                assert_eq!(expected, common::feats());
            }

            #[test]
            fn from_test() {
                expect();
            }
            "#,
        )
        .build();

    p.cargo("run").env("EXPECTED_FEATS", "15").run();

    // Only normal feature.
    p.cargo("run -Zfeatures=all")
        .masquerade_as_nightly_cargo()
        .env("EXPECTED_FEATS", "1")
        .run();

    p.cargo("test").env("EXPECTED_FEATS", "15").run();

    // only normal+dev
    p.cargo("test -Zfeatures=all")
        .masquerade_as_nightly_cargo()
        .env("EXPECTED_FEATS", "5")
        .run();
}

#[cargo_test]
fn required_features_host_dep() {
    // Check that required-features handles build-dependencies correctly.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2018"

            [[bin]]
            name = "x"
            required-features = ["bdep/f1"]

            [build-dependencies]
            bdep = {path="bdep"}
            "#,
        )
        .file("build.rs", "fn main() {}")
        .file(
            "src/bin/x.rs",
            r#"
            fn main() {}
            "#,
        )
        .file(
            "bdep/Cargo.toml",
            r#"
            [package]
            name = "bdep"
            version = "0.1.0"

            [features]
            f1 = []
            "#,
        )
        .file("bdep/src/lib.rs", "")
        .build();

    p.cargo("run")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] target `x` in package `foo` requires the features: `bdep/f1`
Consider enabling them by passing, e.g., `--features=\"bdep/f1\"`
",
        )
        .run();

    p.cargo("run --features bdep/f1 -Zfeatures=host_dep")
        .masquerade_as_nightly_cargo()
        .run();
}

#[cargo_test]
fn disabled_shared_host_dep() {
    // Check for situation where an optional dep of a shared dep is enabled in
    // a normal dependency, but disabled in an optional one. The unit tree is:
    // foo
    // ├── foo build.rs
    // |   └── common (BUILD dependency, NO FEATURES)
    // └── common (Normal dependency, default features)
    //     └── somedep
    Package::new("somedep", "1.0.0")
        .file(
            "src/lib.rs",
            r#"
            pub fn f() { println!("hello from somedep"); }
            "#,
        )
        .publish();
    Package::new("common", "1.0.0")
        .feature("default", &["somedep"])
        .add_dep(Dependency::new("somedep", "1.0").optional(true))
        .file(
            "src/lib.rs",
            r#"
            pub fn check_somedep() -> bool {
                #[cfg(feature="somedep")]
                {
                    extern crate somedep;
                    somedep::f();
                    true
                }
                #[cfg(not(feature="somedep"))]
                {
                    println!("no somedep");
                    false
                }
            }
            "#,
        )
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "1.0.0"
            edition = "2018"

            [dependencies]
            common = "1.0"

            [build-dependencies]
            common = {version = "1.0", default-features = false}
            "#,
        )
        .file(
            "src/main.rs",
            "fn main() { assert!(common::check_somedep()); }",
        )
        .file(
            "build.rs",
            "fn main() { assert!(!common::check_somedep()); }",
        )
        .build();

    p.cargo("run -Zfeatures=host_dep -v")
        .masquerade_as_nightly_cargo()
        .with_stdout("hello from somedep")
        .run();
}

#[cargo_test]
fn required_features_inactive_dep() {
    // required-features with an inactivated dep.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [target.'cfg(whatever)'.dependencies]
            bar = {path="bar"}

            [[bin]]
            name = "foo"
            required-features = ["feat1"]

            [features]
            feat1 = []
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("check -Zfeatures=itarget")
        .masquerade_as_nightly_cargo()
        .with_stderr("[FINISHED] [..]")
        .run();

    p.cargo("check -Zfeatures=itarget --features=feat1")
        .masquerade_as_nightly_cargo()
        .with_stderr("[CHECKING] foo[..]\n[FINISHED] [..]")
        .run();
}

#[cargo_test]
fn decouple_proc_macro() {
    // proc macro features are not shared
    Package::new("common", "1.0.0")
        .feature("somefeat", &[])
        .file(
            "src/lib.rs",
            r#"
            pub const fn foo() -> bool { cfg!(feature="somefeat") }
            #[cfg(feature="somefeat")]
            pub const FEAT_ONLY_CONST: bool = true;
            "#,
        )
        .publish();
    Package::new("pm", "1.0.0")
        .proc_macro(true)
        .feature_dep("common", "1.0", &["somefeat"])
        .file(
            "src/lib.rs",
            r#"
            extern crate proc_macro;
            extern crate common;
            #[proc_macro]
            pub fn foo(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
                assert!(common::foo());
                "".parse().unwrap()
            }
            "#,
        )
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "1.0.0"
            edition = "2018"

            [dependencies]
            pm = "1.0"
            common = "1.0"
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
            //! Test with docs.
            //!
            //! ```rust
            //! pm::foo!{}
            //! fn main() {
            //!   let expected = std::env::var_os("TEST_EXPECTS_ENABLED").is_some();
            //!   assert_eq!(expected, common::foo(), "common is wrong");
            //! }
            //! ```
            "#,
        )
        .file(
            "src/main.rs",
            r#"
            pm::foo!{}
            fn main() {
                println!("it is {}", common::foo());
            }
            "#,
        )
        .build();

    p.cargo("run")
        .env("TEST_EXPECTS_ENABLED", "1")
        .with_stdout("it is true")
        .run();

    p.cargo("run -Zfeatures=host_dep")
        .masquerade_as_nightly_cargo()
        .with_stdout("it is false")
        .run();

    // Make sure the test is fallible.
    p.cargo("test --doc")
        .with_status(101)
        .with_stdout_contains("[..]common is wrong[..]")
        .run();

    p.cargo("test --doc").env("TEST_EXPECTS_ENABLED", "1").run();

    p.cargo("test --doc -Zfeatures=host_dep")
        .masquerade_as_nightly_cargo()
        .run();

    p.cargo("doc").run();
    assert!(p
        .build_dir()
        .join("doc/common/constant.FEAT_ONLY_CONST.html")
        .exists());
    // cargo doc should clean in-between runs, but it doesn't, and leaves stale files.
    // https://github.com/rust-lang/cargo/issues/6783 (same for removed items)
    p.build_dir().join("doc").rm_rf();

    p.cargo("doc -Zfeatures=host_dep")
        .masquerade_as_nightly_cargo()
        .run();
    assert!(!p
        .build_dir()
        .join("doc/common/constant.FEAT_ONLY_CONST.html")
        .exists());
}

#[cargo_test]
fn proc_macro_ws() {
    // Checks for bug with proc-macro in a workspace with dependency (shouldn't panic).
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["foo", "pm"]
            "#,
        )
        .file(
            "foo/Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [features]
            feat1 = []
            "#,
        )
        .file("foo/src/lib.rs", "")
        .file(
            "pm/Cargo.toml",
            r#"
            [package]
            name = "pm"
            version = "0.1.0"

            [lib]
            proc-macro = true

            [dependencies]
            foo = { path = "../foo", features=["feat1"] }
            "#,
        )
        .file("pm/src/lib.rs", "")
        .build();

    p.cargo("check -p pm -Zfeatures=host_dep -v")
        .masquerade_as_nightly_cargo()
        .with_stderr_contains("[RUNNING] `rustc --crate-name foo [..]--cfg[..]feat1[..]")
        .run();
    // This may be surprising that `foo` doesn't get built separately. It is
    // because pm might have other units (binaries, tests, etc.), and so the
    // feature resolver must assume that normal deps get unified with it. This
    // is related to the bigger issue where the features selected in a
    // workspace depend on which packages are selected.
    p.cargo("check --workspace -Zfeatures=host_dep -v")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[FRESH] foo v0.1.0 [..]
[FRESH] pm v0.1.0 [..]
[FINISHED] dev [..]
",
        )
        .run();
    // Selecting just foo will build without unification.
    p.cargo("check -p foo -Zfeatures=host_dep -v")
        .masquerade_as_nightly_cargo()
        // Make sure `foo` is built without feat1
        .with_stderr_line_without(&["[RUNNING] `rustc --crate-name foo"], &["--cfg[..]feat1"])
        .run();
}

#[cargo_test]
fn has_dev_dep_for_test() {
    // Check for a bug where the decision on whether or not "dev dependencies"
    // should be used did not consider `check --profile=test`.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dev-dependencies]
            dep = { path = 'dep', features = ['f1'] }
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
            #[test]
            fn t1() {
                dep::f();
            }
            "#,
        )
        .file(
            "dep/Cargo.toml",
            r#"
            [package]
            name = "dep"
            version = "0.1.0"

            [features]
            f1 = []
            "#,
        )
        .file(
            "dep/src/lib.rs",
            r#"
            #[cfg(feature = "f1")]
            pub fn f() {}
            "#,
        )
        .build();

    p.cargo("check -v")
        .with_stderr(
            "\
[CHECKING] foo v0.1.0 [..]
[RUNNING] `rustc --crate-name foo [..]
[FINISHED] [..]
",
        )
        .run();
    p.cargo("check -v --profile=test -Zfeatures=dev_dep")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[CHECKING] dep v0.1.0 [..]
[RUNNING] `rustc --crate-name dep [..]
[CHECKING] foo v0.1.0 [..]
[RUNNING] `rustc --crate-name foo [..]
[FINISHED] [..]
",
        )
        .run();
    p.cargo("check -v --profile=test")
        .with_stderr(
            "\
[FRESH] dep [..]
[FRESH] foo [..]
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn build_dep_activated() {
    // Build dependencies always match the host for [target.*.build-dependencies].
    if cross_compile::disabled() {
        return;
    }
    Package::new("somedep", "1.0.0")
        .file("src/lib.rs", "")
        .publish();
    Package::new("targetdep", "1.0.0").publish();
    Package::new("hostdep", "1.0.0")
        // Check that "for_host" is sticky.
        .target_dep("somedep", "1.0", &rustc_host())
        .feature("feat1", &[])
        .file(
            "src/lib.rs",
            r#"
            extern crate somedep;

            #[cfg(not(feature="feat1"))]
            compile_error!{"feat1 missing"}
            "#,
        )
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
            [package]
            name = "foo"
            version = "0.1.0"

            # This should never be selected.
            [target.'{}'.build-dependencies]
            targetdep = "1.0"

            [target.'{}'.build-dependencies]
            hostdep = {{version="1.0", features=["feat1"]}}
            "#,
                alternate(),
                rustc_host()
            ),
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .build();

    p.cargo("check").run();
    p.cargo("check -Zfeatures=all")
        .masquerade_as_nightly_cargo()
        .run();
    p.cargo("check --target").arg(alternate()).run();
    p.cargo("check -Zfeatures=all --target")
        .arg(alternate())
        .masquerade_as_nightly_cargo()
        .run();
}

#[cargo_test]
fn resolver_gated() {
    // Check that `resolver` field is feature gated.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            resolver = "2"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
error: failed to parse manifest at `[..]/foo/Cargo.toml`

Caused by:
  feature `resolver` is required

  consider adding `cargo-features = [\"resolver\"]` to the manifest
",
        )
        .run();

    // Test with virtual ws.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["a"]
            resolver = "2"
            "#,
        )
        .file("a/Cargo.toml", &basic_manifest("a", "0.1.0"))
        .file("a/src/lib.rs", "")
        .build();

    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
error: failed to parse manifest at `[..]/foo/Cargo.toml`

Caused by:
  feature `resolver` is required

  consider adding `cargo-features = [\"resolver\"]` to the manifest
",
        )
        .run();
}

#[cargo_test]
fn resolver_bad_setting() {
    // Unknown setting in `resolver`
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["resolver"]
            [package]
            name = "foo"
            version = "0.1.0"
            resolver = "1"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
error: failed to parse manifest at `[..]/foo/Cargo.toml`

Caused by:
  `resolver` setting `1` is not valid, only valid option is \"2\"
",
        )
        .run();
}

#[cargo_test]
fn resolver_not_both() {
    // Can't specify resolver in both workspace and package.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["resolver"]
            [workspace]
            resolver = "2"
            [package]
            name = "foo"
            version = "0.1.0"
            resolver = "2"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
error: failed to parse manifest at `[..]/foo/Cargo.toml`

Caused by:
  cannot specify `resolver` field in both `[workspace]` and `[package]`
",
        )
        .run();
}

#[cargo_test]
fn resolver_ws_member() {
    // Can't specify `resolver` in a ws member.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["a"]
            "#,
        )
        .file(
            "a/Cargo.toml",
            r#"
            cargo-features = ["resolver"]
            [package]
            name = "a"
            version = "0.1.0"
            resolver = "2"
            "#,
        )
        .file("a/src/lib.rs", "")
        .build();

    p.cargo("check")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
warning: resolver for the non root package will be ignored, specify resolver at the workspace root:
package:   [..]/foo/a/Cargo.toml
workspace: [..]/foo/Cargo.toml
[CHECKING] a v0.1.0 [..]
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn resolver_ws_root_and_member() {
    // Check when specified in both ws root and member.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["resolver"]
            [workspace]
            members = ["a"]
            resolver = "2"
            "#,
        )
        .file(
            "a/Cargo.toml",
            r#"
            cargo-features = ["resolver"]
            [package]
            name = "a"
            version = "0.1.0"
            resolver = "2"
            "#,
        )
        .file("a/src/lib.rs", "")
        .build();

    // Ignores if they are the same.
    p.cargo("check")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[CHECKING] a v0.1.0 [..]
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn resolver_enables_new_features() {
    // resolver="2" enables all the things.
    Package::new("common", "1.0.0")
        .feature("normal", &[])
        .feature("build", &[])
        .feature("dev", &[])
        .feature("itarget", &[])
        .file(
            "src/lib.rs",
            r#"
            pub fn feats() -> u32 {
                let mut res = 0;
                if cfg!(feature="normal") { res |= 1; }
                if cfg!(feature="build") { res |= 2; }
                if cfg!(feature="dev") { res |= 4; }
                if cfg!(feature="itarget") { res |= 8; }
                res
            }
            "#,
        )
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["resolver"]
            [workspace]
            members = ["a", "b"]
            resolver = "2"
            "#,
        )
        .file(
            "a/Cargo.toml",
            r#"
            [package]
            name = "a"
            version = "0.1.0"
            edition = "2018"

            [dependencies]
            common = {version = "1.0", features=["normal"]}

            [dev-dependencies]
            common = {version = "1.0", features=["dev"]}

            [build-dependencies]
            common = {version = "1.0", features=["build"]}

            [target.'cfg(whatever)'.dependencies]
            common = {version = "1.0", features=["itarget"]}
            "#,
        )
        .file(
            "a/src/main.rs",
            r#"
            fn main() {
                expect();
            }

            fn expect() {
                let expected: u32 = std::env::var("EXPECTED_FEATS").unwrap().parse().unwrap();
                assert_eq!(expected, common::feats());
            }

            #[test]
            fn from_test() {
                expect();
            }
            "#,
        )
        .file(
            "b/Cargo.toml",
            r#"
            [package]
            name = "b"
            version = "0.1.0"

            [features]
            ping = []
            "#,
        )
        .file(
            "b/src/main.rs",
            r#"
            fn main() {
                if cfg!(feature="ping") {
                    println!("pong");
                }
            }
            "#,
        )
        .build();

    // Only normal.
    p.cargo("run --bin a")
        .masquerade_as_nightly_cargo()
        .env("EXPECTED_FEATS", "1")
        .with_stderr(
            "\
[UPDATING] [..]
[DOWNLOADING] crates ...
[DOWNLOADED] common [..]
[COMPILING] common v1.0.0
[COMPILING] a v0.1.0 [..]
[FINISHED] [..]
[RUNNING] `target/debug/a[EXE]`
",
        )
        .run();

    // only normal+dev
    p.cargo("test")
        .cwd("a")
        .masquerade_as_nightly_cargo()
        .env("EXPECTED_FEATS", "5")
        .run();

    // -Zpackage-features is enabled.
    p.cargo("run -p b --features=ping")
        .cwd("a")
        .masquerade_as_nightly_cargo()
        .with_stdout("pong")
        .run();
}

#[cargo_test]
fn install_resolve_behavior() {
    // install honors the resolver behavior.
    Package::new("common", "1.0.0")
        .feature("f1", &[])
        .file(
            "src/lib.rs",
            r#"
            #[cfg(feature = "f1")]
            compile_error!("f1 should not activate");
            "#,
        )
        .publish();

    Package::new("bar", "1.0.0").dep("common", "1.0").publish();

    Package::new("foo", "1.0.0")
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["resolver"]

            [package]
            name = "foo"
            version = "1.0.0"
            resolver = "2"

            [target.'cfg(whatever)'.dependencies]
            common = {version="1.0", features=["f1"]}

            [dependencies]
            bar = "1.0"

            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .publish();

    cargo_process("install foo")
        .masquerade_as_nightly_cargo()
        .run();
}

#[cargo_test]
fn package_includes_resolve_behavior() {
    // `cargo package` will inherit the correct resolve behavior.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["resolver"]
            [workspace]
            members = ["a"]
            resolver = "2"
            "#,
        )
        .file(
            "a/Cargo.toml",
            r#"
            [package]
            name = "a"
            version = "0.1.0"
            authors = ["Zzz"]
            description = "foo"
            license = "MIT"
            homepage = "https://example.com/"
            "#,
        )
        .file("a/src/lib.rs", "")
        .build();

    p.cargo("package")
        .cwd("a")
        .masquerade_as_nightly_cargo()
        .run();

    let rewritten_toml = format!(
        r#"{}
cargo-features = ["resolver"]

[package]
name = "a"
version = "0.1.0"
authors = ["Zzz"]
description = "foo"
homepage = "https://example.com/"
license = "MIT"
resolver = "2"
"#,
        cargo::core::package::MANIFEST_PREAMBLE
    );

    let f = File::open(&p.root().join("target/package/a-0.1.0.crate")).unwrap();
    validate_crate_contents(
        f,
        "a-0.1.0.crate",
        &["Cargo.toml", "Cargo.toml.orig", "src/lib.rs"],
        &[("Cargo.toml", &rewritten_toml)],
    );
}

#[cargo_test]
fn tree_all() {
    // `cargo tree` with the new feature resolver.
    Package::new("log", "0.4.8").feature("serde", &[]).publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [target.'cfg(whatever)'.dependencies]
                log = {version="*", features=["serde"]}
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("tree --target=all -Zfeatures=all")
        .masquerade_as_nightly_cargo()
        .with_stdout(
            "\
foo v0.1.0 ([..]/foo)
└── log v0.4.8
",
        )
        .run();
}
