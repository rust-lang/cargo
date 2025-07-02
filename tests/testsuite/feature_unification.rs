//! Tests for workspace feature unification.

use crate::prelude::*;
use crate::utils::cargo_process;
use cargo_test_support::{
    basic_manifest,
    compare::assert_e2e,
    project,
    registry::{Dependency, Package},
    str,
};

#[cargo_test]
fn workspace_feature_unification() {
    let p = project()
        .file(
            ".cargo/config.toml",
            r#"
                [resolver]
                feature-unification = "workspace"
            "#,
        )
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                resolver = "2"
                members = ["common", "a", "b"]
            "#,
        )
        .file(
            "common/Cargo.toml",
            r#"
                [package]
                name = "common"
                version = "0.1.0"
                edition = "2021"

                [features]
                a = []
                b = []
            "#,
        )
        .file(
            "common/src/lib.rs",
            r#"
                #[cfg(not(all(feature = "a", feature = "b")))]
                compile_error!("features were not unified");
            "#,
        )
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                common = { path = "../common", features = ["a"] }
            "#,
        )
        .file("a/src/lib.rs", "")
        .file(
            "b/Cargo.toml",
            r#"
                [package]
                name = "b"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                common = { path = "../common", features = ["b"] }
            "#,
        )
        .file("b/src/lib.rs", "")
        .build();

    p.cargo("check -p common")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .with_stderr_data(str![[r#"
[CHECKING] common v0.1.0 ([ROOT]/foo/common)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("check -p a")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .with_stderr_data(str![[r#"
[CHECKING] a v0.1.0 ([ROOT]/foo/a)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("check -p b")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .with_stderr_data(str![[r#"
[CHECKING] b v0.1.0 ([ROOT]/foo/b)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("check")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn package_feature_unification() {
    Package::new("outside", "0.1.0")
        .feature("a", &[])
        .feature("b", &[])
        .file(
            "src/lib.rs",
            r#"
                #[cfg(all(feature = "a", feature = "b"))]
                compile_error!("features were unified");
                #[cfg(feature = "a")]
                pub fn a() {}
                #[cfg(feature = "b")]
                pub fn b() {}
            "#,
        )
        .publish();

    let p = project()
        .file(
            ".cargo/config.toml",
            r#"
                [resolver]
                feature-unification = "package"
            "#,
        )
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                resolver = "2"
                members = ["common", "a", "b"]
            "#,
        )
        .file(
            "common/Cargo.toml",
            r#"
                [package]
                name = "common"
                version = "0.1.0"
                edition = "2021"

                [features]
                a = []
                b = []
            "#,
        )
        .file(
            "common/src/lib.rs",
            r#"
                #[cfg(all(feature = "a", feature = "b"))]
                compile_error!("features were unified");
                #[cfg(feature = "a")]
                pub fn a() {}
                #[cfg(feature = "b")]
                pub fn b() {}
            "#,
        )
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                common = { path = "../common", features = ["a"] }
                outside = { version = "0.1.0", features = ["a"] }
            "#,
        )
        .file("a/src/lib.rs", "pub use common::a;")
        .file(
            "b/Cargo.toml",
            r#"
                [package]
                name = "b"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                common = { path = "../common", features = ["b"] }
                outside = { version = "0.1.0", features = ["b"] }
            "#,
        )
        .file("b/src/lib.rs", "pub use common::b;")
        .build();

    p.cargo("check -p common")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[CHECKING] common v0.1.0 ([ROOT]/foo/common)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("check -p a")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .with_stderr_data(
            str![[r#"
[DOWNLOADING] crates ...
[DOWNLOADED] outside v0.1.0 (registry `dummy-registry`)
[CHECKING] outside v0.1.0
[CHECKING] common v0.1.0 ([ROOT]/foo/common)
[CHECKING] a v0.1.0 ([ROOT]/foo/a)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
    p.cargo("check -p b")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .with_stderr_data(
            str![[r#"
[CHECKING] outside v0.1.0
[CHECKING] common v0.1.0 ([ROOT]/foo/common)
[CHECKING] b v0.1.0 ([ROOT]/foo/b)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
    p.cargo("check -p a -p b")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .with_stderr_data(
            str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
    p.cargo("check")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    // Sanity check that compilation without package feature unification does not work
    p.cargo("check -p a -p b")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .env("CARGO_RESOLVER_FEATURE_UNIFICATION", "selected")
        .with_status(101)
        .with_stderr_contains("[ERROR] features were unified")
        .run();
}

#[cargo_test]
fn package_feature_unification_default_features() {
    let p = project()
        .file(
            ".cargo/config.toml",
            r#"
                [resolver]
                feature-unification = "package"
            "#,
        )
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                resolver = "2"
                members = ["common", "a", "b"]
            "#,
        )
        .file(
            "common/Cargo.toml",
            r#"
                [package]
                name = "common"
                version = "0.1.0"
                edition = "2021"

                [features]
                default = ["a"]
                a = []
                b = []
            "#,
        )
        .file(
            "common/src/lib.rs",
            r#"
                #[cfg(all(feature = "a", feature = "b"))]
                compile_error!("features were unified");
                #[cfg(feature = "a")]
                pub fn a() {}
                #[cfg(feature = "b")]
                pub fn b() {}
            "#,
        )
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                common = { path = "../common" }
            "#,
        )
        .file("a/src/lib.rs", "pub use common::a;")
        .file(
            "b/Cargo.toml",
            r#"
                [package]
                name = "b"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                common = { path = "../common", features = ["b"], default-features = false }
            "#,
        )
        .file("b/src/lib.rs", "pub use common::b;")
        .build();

    p.cargo("check -p common")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .with_stderr_data(str![[r#"
[CHECKING] common v0.1.0 ([ROOT]/foo/common)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("check -p a")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .with_stderr_data(str![[r#"
[CHECKING] a v0.1.0 ([ROOT]/foo/a)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("check -p b")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .with_stderr_data(str![[r#"
[CHECKING] common v0.1.0 ([ROOT]/foo/common)
[CHECKING] b v0.1.0 ([ROOT]/foo/b)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("check")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .with_stderr_data(
            str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn package_feature_unification_cli_features() {
    Package::new("outside", "0.1.0")
        .feature("a", &[])
        .feature("b", &[])
        .file(
            "src/lib.rs",
            r#"
                #[cfg(all(feature = "a", feature = "b"))]
                compile_error!("features were unified");
                #[cfg(feature = "a")]
                pub fn a() {}
                #[cfg(feature = "b")]
                pub fn b() {}
            "#,
        )
        .publish();

    let p = project()
        .file(
            ".cargo/config.toml",
            r#"
                [resolver]
                feature-unification = "package"
            "#,
        )
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                resolver = "2"
                members = ["common", "a", "b"]
            "#,
        )
        .file(
            "common/Cargo.toml",
            r#"
                [package]
                name = "common"
                version = "0.1.0"
                edition = "2021"

                [features]
                a = []
                b = []
            "#,
        )
        .file(
            "common/src/lib.rs",
            r#"
                #[cfg(all(feature = "a", feature = "b"))]
                compile_error!("features were unified");
                #[cfg(feature = "a")]
                pub fn a() {}
                #[cfg(feature = "b")]
                pub fn b() {}
            "#,
        )
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                common = { path = "../common" }
                outside = "0.1.0"

                [features]
                a = ["common/a", "outside/a"]
            "#,
        )
        .file("a/src/lib.rs", "pub use common::a;")
        .file(
            "b/Cargo.toml",
            r#"
                [package]
                name = "b"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                common = { path = "../common", features = ["b"] }
                outside = "0.1.0"

                [features]
                b = ["common/b", "outside/b"]
            "#,
        )
        .file("b/src/lib.rs", "pub use common::b;")
        .build();

    p.cargo("check -p a -p b -F a,b")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .with_stderr_data(
            str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[CHECKING] common v0.1.0 ([ROOT]/foo/common)
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] outside v0.1.0 (registry `dummy-registry`)
[CHECKING] outside v0.1.0
[CHECKING] b v0.1.0 ([ROOT]/foo/b)
[CHECKING] a v0.1.0 ([ROOT]/foo/a)

"#]]
            .unordered(),
        )
        .run();
    p.cargo("check --workspace --exclude common -F a,b")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .with_stderr_data(
            str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();

    p.cargo("check -p a -p b -F a/a,b/b")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .with_stderr_data(
            str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
    p.cargo("check -p a -p b -F a,b,c")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] none of the selected packages contains this feature: c
selected packages: a, b

"#]])
        .run();
    p.cargo("check -p a -F b")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] the package 'a' does not contain this feature: b
[HELP] packages with the missing feature: common, b

"#]])
        .run();
    p.cargo("check -p a -F a/a,common/b")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .with_status(101)
        .with_stderr_contains("[ERROR] features were unified")
        .run();

    p.cargo("check -p a -F a/a,outside/b")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .with_status(101)
        .with_stderr_contains("[ERROR] features were unified")
        .run();

    // Sanity check that compilation without package feature unification does not work
    p.cargo("check -p a -p b -F a,b")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .env("CARGO_RESOLVER_FEATURE_UNIFICATION", "selected")
        .with_status(101)
        .with_stderr_contains("[ERROR] features were unified")
        .run();
}

#[cargo_test]
fn package_feature_unification_weak_dependencies() {
    let p = project()
        .file(
            ".cargo/config.toml",
            r#"
                [resolver]
                feature-unification = "package"
            "#,
        )
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                resolver = "2"
                members = ["common", "a", "b"]
            "#,
        )
        .file(
            "common/Cargo.toml",
            r#"
                [package]
                name = "common"
                version = "0.1.0"
                edition = "2021"

                [features]
                a = []
                b = []
            "#,
        )
        .file(
            "common/src/lib.rs",
            r#"
                #[cfg(all(feature = "a", feature = "b"))]
                compile_error!("features were unified");
                #[cfg(feature = "a")]
                pub fn a() {}
                #[cfg(feature = "b")]
                pub fn b() {}
            "#,
        )
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                common = { path = "../common", optional = true }

                [features]
                default = ["dep:common", "common?/a"]
            "#,
        )
        .file("a/src/lib.rs", "pub use common::a;")
        .file(
            "b/Cargo.toml",
            r#"
                [package]
                name = "b"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                common = { path = "../common", optional = true }

                [features]
                default = ["dep:common", "common?/b"]
            "#,
        )
        .file("b/src/lib.rs", "pub use common::b;")
        .build();

    p.cargo("check -p a -p b")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .with_stderr_data(
            str![[r#"
[CHECKING] common v0.1.0 ([ROOT]/foo/common)
[CHECKING] a v0.1.0 ([ROOT]/foo/a)
[CHECKING] b v0.1.0 ([ROOT]/foo/b)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
    p.cargo("check")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .with_stderr_data(str![[r#"
[CHECKING] common v0.1.0 ([ROOT]/foo/common)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    // Sanity check that compilation without package feature unification does not work
    p.cargo("check -p a -p b")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .env("CARGO_RESOLVER_FEATURE_UNIFICATION", "selected")
        .with_status(101)
        .with_stderr_contains("[ERROR] features were unified")
        .run();
}

#[cargo_test]
fn feature_unification_cargo_tree() {
    Package::new("outside", "0.1.0")
        .feature("a", &[])
        .feature("b", &[])
        .file(
            "src/lib.rs",
            r#"
                #[cfg(all(feature = "a", feature = "b"))]
                compile_error!("features were unified");
                #[cfg(feature = "a")]
                pub fn a() {}
                #[cfg(feature = "b")]
                pub fn b() {}
            "#,
        )
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                resolver = "2"
                members = ["common", "a", "b"]
            "#,
        )
        .file(
            "common/Cargo.toml",
            r#"
                [package]
                name = "common"
                version = "0.1.0"
                edition = "2021"

                [features]
                a = []
                b = []
            "#,
        )
        .file(
            "common/src/lib.rs",
            r#"
                #[cfg(all(feature = "a", feature = "b"))]
                compile_error!("features were unified");
                #[cfg(feature = "a")]
                pub fn a() {}
                #[cfg(feature = "b")]
                pub fn b() {}
            "#,
        )
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                common = { path = "../common", features = ["a"] }
                outside = { version = "0.1.0", features = ["a"] }
            "#,
        )
        .file("a/src/lib.rs", "pub use common::a;")
        .file(
            "b/Cargo.toml",
            r#"
                [package]
                name = "b"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                common = { path = "../common", features = ["b"] }
                outside = { version = "0.1.0", features = ["b"] }
            "#,
        )
        .file("b/src/lib.rs", "pub use common::b;")
        .build();

    p.cargo("tree -e features")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .env("CARGO_RESOLVER_FEATURE_UNIFICATION", "selected")
        .with_stdout_data(str![[r#"
a v0.1.0 ([ROOT]/foo/a)
├── common feature "a"
│   └── common v0.1.0 ([ROOT]/foo/common)
├── common feature "default" (command-line)
│   └── common v0.1.0 ([ROOT]/foo/common)
├── outside feature "a"
│   └── outside v0.1.0
└── outside feature "default"
    └── outside v0.1.0

b v0.1.0 ([ROOT]/foo/b)
├── common feature "b"
│   └── common v0.1.0 ([ROOT]/foo/common)
├── common feature "default" (command-line) (*)
├── outside feature "b"
│   └── outside v0.1.0
└── outside feature "default" (*)

common v0.1.0 ([ROOT]/foo/common)

"#]])
        .run();

    p.cargo("tree -e features")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .env("CARGO_RESOLVER_FEATURE_UNIFICATION", "workspace")
        .with_stdout_data(str![[r#"
a v0.1.0 ([ROOT]/foo/a)
├── common feature "a"
│   └── common v0.1.0 ([ROOT]/foo/common)
├── common feature "default" (command-line)
│   └── common v0.1.0 ([ROOT]/foo/common)
├── outside feature "a"
│   └── outside v0.1.0
└── outside feature "default"
    └── outside v0.1.0

b v0.1.0 ([ROOT]/foo/b)
├── common feature "b"
│   └── common v0.1.0 ([ROOT]/foo/common)
├── common feature "default" (command-line) (*)
├── outside feature "b"
│   └── outside v0.1.0
└── outside feature "default" (*)

common v0.1.0 ([ROOT]/foo/common)

"#]])
        .run();

    p.cargo("tree -e features")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .env("CARGO_RESOLVER_FEATURE_UNIFICATION", "package")
        .with_stdout_data(str![[r#"
common v0.1.0 ([ROOT]/foo/common)
a v0.1.0 ([ROOT]/foo/a)
├── common feature "a"
│   └── common v0.1.0 ([ROOT]/foo/common)
├── common feature "default"
│   └── common v0.1.0 ([ROOT]/foo/common)
├── outside feature "a"
│   └── outside v0.1.0
└── outside feature "default"
    └── outside v0.1.0
b v0.1.0 ([ROOT]/foo/b)
├── common feature "b"
│   └── common v0.1.0 ([ROOT]/foo/common)
├── common feature "default"
│   └── common v0.1.0 ([ROOT]/foo/common)
├── outside feature "b"
│   └── outside v0.1.0
└── outside feature "default"
    └── outside v0.1.0

"#]])
        .run();
}

#[cargo_test]
fn cargo_install_ignores_config() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                common = { path = "common", features = ["a"] }

                [workspace]
                members = ["common", "b"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "common/Cargo.toml",
            r#"
                [package]
                name = "common"
                version = "0.1.0"
                edition = "2021"

                [features]
                a = []
                b = []
            "#,
        )
        .file(
            "common/src/lib.rs",
            r#"
                #[cfg(all(feature = "a", feature = "b"))]
                compile_error!("features should not be unified");
            "#,
        )
        .file(
            "b/Cargo.toml",
            r#"
                [package]
                name = "b"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                common = { path = "../common", features = ["b"] }
            "#,
        )
        .file("b/src/lib.rs", "")
        .build();

    cargo_process("install --path")
        .arg(p.root())
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .env("CARGO_RESOLVER_FEATURE_UNIFICATION", "workspace")
        .with_stderr_data(str![[r#"
[INSTALLING] a v0.1.0 ([ROOT]/foo)
[COMPILING] common v0.1.0 ([ROOT]/foo/common)
[COMPILING] a v0.1.0 ([ROOT]/foo)
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[INSTALLING] [ROOT]/home/.cargo/bin/a[EXE]
[INSTALLED] package `a v0.1.0 ([ROOT]/foo)` (executable `a[EXE]`)
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]])
        .run();
    cargo_process("install --path")
        .arg(p.root())
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .env("CARGO_RESOLVER_FEATURE_UNIFICATION", "package")
        .with_stderr_data(str![[r#"
[INSTALLING] a v0.1.0 ([ROOT]/foo)
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[REPLACING] [ROOT]/home/.cargo/bin/a[EXE]
[REPLACED] package `a v0.1.0 ([ROOT]/foo)` with `a v0.1.0 ([ROOT]/foo)` (executable `a[EXE]`)
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]])
        .run();
}

#[cargo_test]
fn unstable_config_on_stable() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                resolver = "2"
                members = ["bar"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("check")
        .env("CARGO_RESOLVER_FEATURE_UNIFICATION", "workspace")
        .with_stderr_data(str![[r#"
[WARNING] ignoring `resolver.feature-unification` without `-Zfeature-unification`
[CHECKING] bar v0.1.0 ([ROOT]/foo/bar)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn cargo_fix_works() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
# Before project
[ project ] # After project header
# After project header line
name = "foo"
edition = "2021"
# After project table
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("fix --edition --allow-no-vcs")
        .env("CARGO_RESOLVER_FEATURE_UNIFICATION", "package")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .with_stderr_data(str![[r#"
[MIGRATING] Cargo.toml from 2021 edition to 2024
[FIXED] Cargo.toml (1 fix)
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[MIGRATING] src/lib.rs from 2021 edition to 2024
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    assert_e2e().eq(
        p.read_file("Cargo.toml"),
        str![[r#"

# Before project
[ package ] # After project header
# After project header line
name = "foo"
edition = "2021"
# After project table

"#]],
    );
}

#[cargo_test]
fn edition_v2_resolver_report() {
    // Show a report if the V2 resolver shows differences.
    Package::new("common", "1.0.0")
        .feature("f1", &[])
        .feature("dev-feat", &[])
        .add_dep(Dependency::new("opt_dep", "1.0").optional(true))
        .publish();
    Package::new("opt_dep", "1.0.0").publish();

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
                [workspace]
                members = ["bar"]

                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2018"

                [dependencies]
                common = "1.0"
                bar = "1.0"

                [build-dependencies]
                common = { version = "1.0", features = ["opt_dep"] }

                [dev-dependencies]
                common = { version="1.0", features=["dev-feat"] }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                edition = "2018"
            "#,
        )
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("fix --edition --allow-no-vcs --workspace")
        .env("CARGO_RESOLVER_FEATURE_UNIFICATION", "package")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .with_status(101)
        .with_stderr_data(
            str![[r#"
[MIGRATING] Cargo.toml from 2018 edition to 2021
[UPDATING] `dummy-registry` index
[LOCKING] 3 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] common v1.0.0 (registry `dummy-registry`)
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[DOWNLOADED] opt_dep v1.0.0 (registry `dummy-registry`)
[MIGRATING] bar/Cargo.toml from 2018 edition to 2021
[ERROR] cannot fix edition when using `feature-unification = "package"`.

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn feature_unification_of_cli_features_within_workspace() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                resolver = "2"
                members = ["parent", "child", "grandchild"]
            "#,
        )
        .file(
            "grandchild/Cargo.toml",
            r#"
                [package]
                name = "grandchild"
                version = "0.1.0"
                edition = "2021"

                [features]
                a = []
            "#,
        )
        .file(
            "grandchild/src/lib.rs",
            r#"
                #[cfg(feature = "a")]
                pub fn a() {}
            "#,
        )
        .file(
            "child/Cargo.toml",
            r#"
                [package]
                name = "child"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                grandchild = { path = "../grandchild" }
            "#,
        )
        .file("child/src/lib.rs", "pub use grandchild::*;")
        .file(
            "parent/Cargo.toml",
            r#"
                [package]
                name = "parent"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                child = { path = "../child" }
            "#,
        )
        .file("parent/src/lib.rs", "pub use child::a;")
        .build();

    p.cargo("check -p parent -F grandchild/a")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .env("CARGO_RESOLVER_FEATURE_UNIFICATION", "package")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] the package 'parent' does not contain this feature: grandchild/a

"#]])
        .run();

    p.cargo("check -p parent -F grandchild/a")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .env("CARGO_RESOLVER_FEATURE_UNIFICATION", "workspace")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] the package 'parent' does not contain this feature: grandchild/a

"#]])
        .run();

    p.cargo("check -p parent -F grandchild/a")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .env("CARGO_RESOLVER_FEATURE_UNIFICATION", "selected")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] the package 'parent' does not contain this feature: grandchild/a

"#]])
        .run();

    p.cargo("check -p child -F grandchild/a")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .env("CARGO_RESOLVER_FEATURE_UNIFICATION", "package")
        .with_stderr_data(str![[r#"
[CHECKING] grandchild v0.1.0 ([ROOT]/foo/grandchild)
[CHECKING] child v0.1.0 ([ROOT]/foo/child)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check -p child -F grandchild/a")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .env("CARGO_RESOLVER_FEATURE_UNIFICATION", "workspace")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check -p child -F grandchild/a")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .env("CARGO_RESOLVER_FEATURE_UNIFICATION", "selected")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check -F grandchild/a")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .env("CARGO_RESOLVER_FEATURE_UNIFICATION", "package")
        .with_stderr_data(str![[r#"
[CHECKING] parent v0.1.0 ([ROOT]/foo/parent)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check -F grandchild/a")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .env("CARGO_RESOLVER_FEATURE_UNIFICATION", "workspace")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check -F grandchild/a")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .env("CARGO_RESOLVER_FEATURE_UNIFICATION", "selected")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check -F grandchild/a --workspace --exclude grandchild")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .env("CARGO_RESOLVER_FEATURE_UNIFICATION", "package")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check -F grandchild/a --workspace --exclude grandchild")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .env("CARGO_RESOLVER_FEATURE_UNIFICATION", "workspace")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check -F grandchild/a --workspace --exclude grandchild")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .env("CARGO_RESOLVER_FEATURE_UNIFICATION", "selected")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check -F grandchild/a --workspace --exclude grandchild --exclude child")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .env("CARGO_RESOLVER_FEATURE_UNIFICATION", "package")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] the package 'parent' does not contain this feature: grandchild/a

"#]])
        .run();

    p.cargo("check -F grandchild/a --workspace --exclude grandchild --exclude child")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .env("CARGO_RESOLVER_FEATURE_UNIFICATION", "workspace")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] the package 'parent' does not contain this feature: grandchild/a

"#]])
        .run();

    p.cargo("check -F grandchild/a --workspace --exclude grandchild --exclude child")
        .arg("-Zfeature-unification")
        .masquerade_as_nightly_cargo(&["feature-unification"])
        .env("CARGO_RESOLVER_FEATURE_UNIFICATION", "selected")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] the package 'parent' does not contain this feature: grandchild/a

"#]])
        .run();
}
