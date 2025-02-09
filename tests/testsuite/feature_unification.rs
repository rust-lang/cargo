//! Tests for workspace feature unification.

use cargo_test_support::prelude::*;
use cargo_test_support::{basic_manifest, cargo_process, project, str};

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
