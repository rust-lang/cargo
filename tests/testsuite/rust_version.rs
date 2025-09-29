//! Tests for targets with `rust-version`.

use crate::prelude::*;
use crate::utils::cargo_process;
use cargo_test_support::{project, registry::Package, str};

#[cargo_test]
fn rust_version_satisfied() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2015"
            authors = []
            rust-version = "1.1.1"
            [[bin]]
            name = "foo"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check").run();
    p.cargo("check --ignore-rust-version").run();
}

#[cargo_test]
fn rust_version_error() {
    project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2015"
            authors = []
            rust-version = "^1.43"
            [[bin]]
            name = "foo"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build()
        .cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] unexpected version requirement, expected a version like "1.32"
 --> Cargo.toml:7:28
  |
7 |             rust-version = "^1.43"
  |                            ^^^^^^^

"#]])
        .run();
}

#[cargo_test]
fn rust_version_older_than_edition() {
    project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            rust-version = "1.1"
            edition = "2018"
            [[bin]]
            name = "foo"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build()
        .cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  rust-version 1.1 is imcompatible with the version (1.31.0) required by the specified edition (2018)

"#]])
        .run();
}

#[cargo_test]
fn lint_self_incompatible_with_rust_version() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2015"
            authors = []
            rust-version = "1.9876.0"
            [[bin]]
            name = "foo"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] rustc [..] is not supported by the following package:
  foo@0.0.1 requires rustc 1.9876.0


"#]])
        .run();
    p.cargo("check --ignore-rust-version").run();
}

#[cargo_test]
fn lint_dep_incompatible_with_rust_version() {
    Package::new("too_new_parent", "0.0.1")
        .dep("too_new_child", "0.0.1")
        .rust_version("1.2345.0")
        .file("src/lib.rs", "fn other_stuff() {}")
        .publish();
    Package::new("too_new_child", "0.0.1")
        .rust_version("1.2345.0")
        .file("src/lib.rs", "fn other_stuff() {}")
        .publish();
    Package::new("rustc_compatible", "0.0.1")
        .rust_version("1.60.0")
        .file("src/lib.rs", "fn other_stuff() {}")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2015"
            rust-version = "1.50"
            authors = []
            [dependencies]
            too_new_parent = "0.0.1"
            rustc_compatible = "0.0.1"
        "#,
        )
        .file("src/main.rs", "fn main(){}")
        .build();

    p.cargo("generate-lockfile")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 3 packages to latest compatible versions
[ADDING] too_new_child v0.0.1 (requires Rust 1.2345.0)
[ADDING] too_new_parent v0.0.1 (requires Rust 1.2345.0)

"#]])
        .run();
    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[DOWNLOADING] crates ...
[DOWNLOADED] too_new_parent v0.0.1 (registry `dummy-registry`)
[DOWNLOADED] too_new_child v0.0.1 (registry `dummy-registry`)
[DOWNLOADED] rustc_compatible v0.0.1 (registry `dummy-registry`)
[ERROR] rustc [..] is not supported by the following packages:
  too_new_child@0.0.1 requires rustc 1.2345.0
  too_new_parent@0.0.1 requires rustc 1.2345.0
Either upgrade rustc or select compatible dependency versions with
`cargo update <name>@<current-ver> --precise <compatible-ver>`
where `<compatible-ver>` is the latest version supporting rustc [..]


"#]])
        .run();
    p.cargo("check --ignore-rust-version").run();
}

#[cargo_test]
fn resolve_with_rust_version() {
    Package::new("only-newer", "1.6.0")
        .rust_version("1.65.0")
        .file("src/lib.rs", "fn other_stuff() {}")
        .publish();
    Package::new("newer-and-older", "1.5.0")
        .rust_version("1.55.0")
        .file("src/lib.rs", "fn other_stuff() {}")
        .publish();
    Package::new("newer-and-older", "1.6.0")
        .rust_version("1.65.0")
        .file("src/lib.rs", "fn other_stuff() {}")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2015"
            authors = []
            rust-version = "1.60.0"

            [dependencies]
            only-newer = "1.0.0"
            newer-and-older = "1.0.0"
        "#,
        )
        .file("src/main.rs", "fn main(){}")
        .build();

    p.cargo("generate-lockfile --ignore-rust-version")
        .env("CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS", "fallback")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions

"#]])
        .run();
    p.cargo("tree")
        .with_stdout_data(str![[r#"
foo v0.0.1 ([ROOT]/foo)
├── newer-and-older v1.6.0
└── only-newer v1.6.0

"#]])
        .run();

    p.cargo("generate-lockfile")
        .env("CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS", "fallback")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest Rust 1.60.0 compatible versions
[ADDING] newer-and-older v1.5.0 (available: v1.6.0, requires Rust 1.65.0)
[ADDING] only-newer v1.6.0 (requires Rust 1.65.0)

"#]])
        .run();
    p.cargo("tree")
        .with_stdout_data(str![[r#"
foo v0.0.1 ([ROOT]/foo)
├── newer-and-older v1.5.0
└── only-newer v1.6.0

"#]])
        .run();
}

#[cargo_test]
fn resolve_with_rustc() {
    Package::new("only-newer", "1.6.0")
        .rust_version("1.2345")
        .file("src/lib.rs", "fn other_stuff() {}")
        .publish();
    Package::new("newer-and-older", "1.5.0")
        .rust_version("1.55.0")
        .file("src/lib.rs", "fn other_stuff() {}")
        .publish();
    Package::new("newer-and-older", "1.6.0")
        .rust_version("1.2345")
        .file("src/lib.rs", "fn other_stuff() {}")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2015"
            authors = []
            rust-version = "1.60.0"

            [dependencies]
            only-newer = "1.0.0"
            newer-and-older = "1.0.0"
        "#,
        )
        .file("src/main.rs", "fn main(){}")
        .build();

    p.cargo("generate-lockfile --ignore-rust-version")
        .env("CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS", "fallback")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[ADDING] newer-and-older v1.6.0 (requires Rust 1.2345)
[ADDING] only-newer v1.6.0 (requires Rust 1.2345)

"#]])
        .run();
    p.cargo("tree")
        .with_stdout_data(str![[r#"
foo v0.0.1 ([ROOT]/foo)
├── newer-and-older v1.6.0
└── only-newer v1.6.0

"#]])
        .run();

    p.cargo("generate-lockfile")
        .env("CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS", "fallback")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest Rust 1.60.0 compatible versions
[ADDING] newer-and-older v1.5.0 (available: v1.6.0, requires Rust 1.2345)
[ADDING] only-newer v1.6.0 (requires Rust 1.2345)

"#]])
        .run();
    p.cargo("tree")
        .with_stdout_data(str![[r#"
foo v0.0.1 ([ROOT]/foo)
├── newer-and-older v1.5.0
└── only-newer v1.6.0

"#]])
        .run();
}

#[cargo_test]
fn resolve_with_backtracking() {
    Package::new("has-rust-version", "1.6.0")
        .rust_version("1.65.0")
        .file("src/lib.rs", "fn other_stuff() {}")
        .publish();
    Package::new("no-rust-version", "2.1.0")
        .file("src/lib.rs", "fn other_stuff() {}")
        .publish();
    Package::new("no-rust-version", "2.2.0")
        .file("src/lib.rs", "fn other_stuff() {}")
        .dep("has-rust-version", "1.6.0")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2015"
            authors = []
            rust-version = "1.60.0"

            [dependencies]
            no-rust-version = "2"
        "#,
        )
        .file("src/main.rs", "fn main(){}")
        .build();

    p.cargo("generate-lockfile --ignore-rust-version")
        .env("CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS", "fallback")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions

"#]])
        .run();
    p.cargo("tree")
        .with_stdout_data(str![[r#"
foo v0.0.1 ([ROOT]/foo)
└── no-rust-version v2.2.0
    └── has-rust-version v1.6.0

"#]])
        .run();

    // Ideally we'd pick `has-rust-version` 1.6.0 which requires backtracking
    p.cargo("generate-lockfile")
        .env("CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS", "fallback")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest Rust 1.60.0 compatible versions
[ADDING] has-rust-version v1.6.0 (requires Rust 1.65.0)

"#]])
        .run();
    p.cargo("tree")
        .with_stdout_data(str![[r#"
foo v0.0.1 ([ROOT]/foo)
└── no-rust-version v2.2.0
    └── has-rust-version v1.6.0

"#]])
        .run();
}

#[cargo_test]
fn resolve_with_multiple_rust_versions() {
    Package::new(&format!("shared-only-newer"), "1.65.0")
        .rust_version("1.65.0")
        .file("src/lib.rs", "fn other_stuff() {}")
        .publish();
    for ver in ["1.45.0", "1.55.0", "1.65.0"] {
        Package::new(&format!("shared-newer-and-older"), ver)
            .rust_version(ver)
            .file("src/lib.rs", "fn other_stuff() {}")
            .publish();
    }
    Package::new(&format!("lower-only-newer"), "1.65.0")
        .rust_version("1.65.0")
        .file("src/lib.rs", "fn other_stuff() {}")
        .publish();
    for ver in ["1.45.0", "1.55.0"] {
        Package::new(&format!("lower-newer-and-older"), ver)
            .rust_version(ver)
            .file("src/lib.rs", "fn other_stuff() {}")
            .publish();
    }
    Package::new(&format!("higher-only-newer"), "1.65.0")
        .rust_version("1.65.0")
        .file("src/lib.rs", "fn other_stuff() {}")
        .publish();
    for ver in ["1.55.0", "1.65.0"] {
        Package::new(&format!("higher-newer-and-older"), ver)
            .rust_version(ver)
            .file("src/lib.rs", "fn other_stuff() {}")
            .publish();
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["lower"]

            [package]
            name = "higher"
            version = "0.0.1"
            edition = "2015"
            authors = []
            rust-version = "1.60.0"

            [dependencies]
            higher-only-newer = "1"
            higher-newer-and-older = "1"
            shared-only-newer = "1"
            shared-newer-and-older = "1"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "lower/Cargo.toml",
            r#"
            [package]
            name = "lower"
            version = "0.0.1"
            edition = "2015"
            authors = []
            rust-version = "1.50.0"

            [dependencies]
            lower-only-newer = "1"
            lower-newer-and-older = "1"
            shared-only-newer = "1"
            shared-newer-and-older = "1"
        "#,
        )
        .file("lower/src/main.rs", "fn main() {}")
        .build();

    p.cargo("generate-lockfile --ignore-rust-version")
        .env("CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS", "fallback")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 6 packages to latest compatible versions

"#]])
        .run();
    p.cargo("tree")
        .with_stdout_data(str![[r#"
higher v0.0.1 ([ROOT]/foo)
├── higher-newer-and-older v1.65.0
├── higher-only-newer v1.65.0
├── shared-newer-and-older v1.65.0
└── shared-only-newer v1.65.0

"#]])
        .run();

    p.cargo("generate-lockfile")
        .env("CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS", "fallback")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 6 packages to latest Rust 1.50.0 compatible versions
[ADDING] higher-newer-and-older v1.55.0 (available: v1.65.0, requires Rust 1.65.0)
[ADDING] higher-only-newer v1.65.0 (requires Rust 1.65.0)
[ADDING] lower-newer-and-older v1.45.0 (available: v1.55.0, requires Rust 1.55.0)
[ADDING] lower-only-newer v1.65.0 (requires Rust 1.65.0)
[ADDING] shared-newer-and-older v1.45.0 (available: v1.65.0, requires Rust 1.65.0)
[ADDING] shared-only-newer v1.65.0 (requires Rust 1.65.0)

"#]])
        .run();
    p.cargo("tree")
        .with_stdout_data(str![[r#"
higher v0.0.1 ([ROOT]/foo)
├── higher-newer-and-older v1.55.0
├── higher-only-newer v1.65.0
├── shared-newer-and-older v1.45.0
└── shared-only-newer v1.65.0

"#]])
        .run();
}

#[cargo_test]
fn resolve_edition2024() {
    Package::new("only-newer", "1.6.0")
        .rust_version("1.999.0")
        .file("src/lib.rs", "fn other_stuff() {}")
        .publish();
    Package::new("newer-and-older", "1.5.0")
        .rust_version("1.80.0")
        .file("src/lib.rs", "fn other_stuff() {}")
        .publish();
    Package::new("newer-and-older", "1.6.0")
        .rust_version("1.999.0")
        .file("src/lib.rs", "fn other_stuff() {}")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2024"
            authors = []
            rust-version = "1.85.0"

            [dependencies]
            only-newer = "1.0.0"
            newer-and-older = "1.0.0"
        "#,
        )
        .file("src/main.rs", "fn main(){}")
        .build();

    // Edition2024 should resolve for MSRV
    p.cargo("generate-lockfile")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest Rust 1.85.0 compatible versions
[ADDING] newer-and-older v1.5.0 (available: v1.6.0, requires Rust 1.999.0)
[ADDING] only-newer v1.6.0 (requires Rust 1.999.0)

"#]])
        .run();
    p.cargo("tree")
        .with_stdout_data(str![[r#"
foo v0.0.1 ([ROOT]/foo)
├── newer-and-older v1.5.0
└── only-newer v1.6.0

"#]])
        .run();

    // `--ignore-rust-version` has precedence over Edition2024
    p.cargo("generate-lockfile --ignore-rust-version")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[ADDING] newer-and-older v1.6.0 (requires Rust 1.999.0)
[ADDING] only-newer v1.6.0 (requires Rust 1.999.0)

"#]])
        .run();
    p.cargo("tree")
        .with_stdout_data(str![[r#"
foo v0.0.1 ([ROOT]/foo)
├── newer-and-older v1.6.0
└── only-newer v1.6.0

"#]])
        .run();

    // config has precedence over Edition2024
    p.cargo("generate-lockfile")
        .env("CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS", "allow")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[ADDING] newer-and-older v1.6.0 (requires Rust 1.999.0)
[ADDING] only-newer v1.6.0 (requires Rust 1.999.0)

"#]])
        .run();
    p.cargo("tree")
        .with_stdout_data(str![[r#"
foo v0.0.1 ([ROOT]/foo)
├── newer-and-older v1.6.0
└── only-newer v1.6.0

"#]])
        .run();
}

#[cargo_test]
fn resolve_v3() {
    Package::new("only-newer", "1.6.0")
        .rust_version("1.999.0")
        .file("src/lib.rs", "fn other_stuff() {}")
        .publish();
    Package::new("newer-and-older", "1.5.0")
        .rust_version("1.80.0")
        .file("src/lib.rs", "fn other_stuff() {}")
        .publish();
    Package::new("newer-and-older", "1.6.0")
        .rust_version("1.999.0")
        .file("src/lib.rs", "fn other_stuff() {}")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2015"
            authors = []
            rust-version = "1.85.0"
            resolver = "3"

            [dependencies]
            only-newer = "1.0.0"
            newer-and-older = "1.0.0"
        "#,
        )
        .file("src/main.rs", "fn main(){}")
        .build();

    // v3 should resolve for MSRV
    p.cargo("generate-lockfile")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest Rust 1.85.0 compatible versions
[ADDING] newer-and-older v1.5.0 (available: v1.6.0, requires Rust 1.999.0)
[ADDING] only-newer v1.6.0 (requires Rust 1.999.0)

"#]])
        .run();
    p.cargo("tree")
        .with_stdout_data(str![[r#"
foo v0.0.1 ([ROOT]/foo)
├── newer-and-older v1.5.0
└── only-newer v1.6.0

"#]])
        .run();

    // `--ignore-rust-version` has precedence over v3
    p.cargo("generate-lockfile --ignore-rust-version")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[ADDING] newer-and-older v1.6.0 (requires Rust 1.999.0)
[ADDING] only-newer v1.6.0 (requires Rust 1.999.0)

"#]])
        .run();
    p.cargo("tree")
        .with_stdout_data(str![[r#"
foo v0.0.1 ([ROOT]/foo)
├── newer-and-older v1.6.0
└── only-newer v1.6.0

"#]])
        .run();

    // config has precedence over v3
    p.cargo("generate-lockfile")
        .env("CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS", "allow")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[ADDING] newer-and-older v1.6.0 (requires Rust 1.999.0)
[ADDING] only-newer v1.6.0 (requires Rust 1.999.0)

"#]])
        .run();
    p.cargo("tree")
        .with_stdout_data(str![[r#"
foo v0.0.1 ([ROOT]/foo)
├── newer-and-older v1.6.0
└── only-newer v1.6.0

"#]])
        .run();
}

#[cargo_test]
fn update_msrv_resolve() {
    Package::new("bar", "1.5.0")
        .rust_version("1.55.0")
        .file("src/lib.rs", "fn other_stuff() {}")
        .publish();
    Package::new("bar", "1.6.0")
        .rust_version("1.65.0")
        .file("src/lib.rs", "fn other_stuff() {}")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2015"
            authors = []
            rust-version = "1.60.0"
            [dependencies]
            bar = "1.0.0"
        "#,
        )
        .file("src/main.rs", "fn main(){}")
        .build();

    p.cargo("update")
        .env("CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS", "fallback")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest Rust 1.60.0 compatible version
[ADDING] bar v1.5.0 (available: v1.6.0, requires Rust 1.65.0)

"#]])
        .run();
    p.cargo("update --ignore-rust-version")
        .env("CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS", "fallback")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[UPDATING] bar v1.5.0 -> v1.6.0

"#]])
        .run();
}

#[cargo_test]
fn update_precise_overrides_msrv_resolver() {
    Package::new("bar", "1.5.0")
        .rust_version("1.55.0")
        .file("src/lib.rs", "fn other_stuff() {}")
        .publish();
    Package::new("bar", "1.6.0")
        .rust_version("1.65.0")
        .file("src/lib.rs", "fn other_stuff() {}")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2015"
            authors = []
            rust-version = "1.60.0"
            [dependencies]
            bar = "1.0.0"
        "#,
        )
        .file("src/main.rs", "fn main(){}")
        .build();

    p.cargo("update")
        .env("CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS", "fallback")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest Rust 1.60.0 compatible version
[ADDING] bar v1.5.0 (available: v1.6.0, requires Rust 1.65.0)

"#]])
        .run();
    p.cargo("update --precise 1.6.0 bar")
        .env("CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS", "fallback")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[UPDATING] bar v1.5.0 -> v1.6.0 (requires Rust 1.65.0)

"#]])
        .run();
}

#[cargo_test]
fn check_msrv_resolve() {
    Package::new("only-newer", "1.6.0")
        .rust_version("1.65.0")
        .file("src/lib.rs", "fn other_stuff() {}")
        .publish();
    Package::new("newer-and-older", "1.5.0")
        .rust_version("1.55.0")
        .file("src/lib.rs", "fn other_stuff() {}")
        .publish();
    Package::new("newer-and-older", "1.6.0")
        .rust_version("1.65.0")
        .file("src/lib.rs", "fn other_stuff() {}")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2015"
            authors = []
            rust-version = "1.60.0"

            [dependencies]
            only-newer = "1.0.0"
            newer-and-older = "1.0.0"
        "#,
        )
        .file("src/main.rs", "fn main(){}")
        .build();

    p.cargo("check --ignore-rust-version")
        .env("CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS", "fallback")
        .with_stderr_data(
            str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] only-newer v1.6.0 (registry `dummy-registry`)
[DOWNLOADED] newer-and-older v1.6.0 (registry `dummy-registry`)
[CHECKING] only-newer v1.6.0
[CHECKING] newer-and-older v1.6.0
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
    p.cargo("tree")
        .with_stdout_data(str![[r#"
foo v0.0.1 ([ROOT]/foo)
├── newer-and-older v1.6.0
└── only-newer v1.6.0

"#]])
        .run();

    std::fs::remove_file(p.root().join("Cargo.lock")).unwrap();
    p.cargo("check")
        .env("CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS", "fallback")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest Rust 1.60.0 compatible versions
[ADDING] newer-and-older v1.5.0 (available: v1.6.0, requires Rust 1.65.0)
[ADDING] only-newer v1.6.0 (requires Rust 1.65.0)
[DOWNLOADING] crates ...
[DOWNLOADED] newer-and-older v1.5.0 (registry `dummy-registry`)
[CHECKING] newer-and-older v1.5.0
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("tree")
        .with_stdout_data(str![[r#"
foo v0.0.1 ([ROOT]/foo)
├── newer-and-older v1.5.0
└── only-newer v1.6.0

"#]])
        .run();
}

#[cargo_test]
fn cargo_install_ignores_msrv_config() {
    Package::new("dep", "1.0.0")
        .rust_version("1.50")
        .file("src/lib.rs", "fn hello() {}")
        .publish();
    Package::new("dep", "1.1.0")
        .rust_version("1.70")
        .file("src/lib.rs", "fn hello() {}")
        .publish();
    Package::new("foo", "0.0.1")
        .rust_version("1.60")
        .file("src/main.rs", "fn main() {}")
        .dep("dep", "1")
        .publish();

    cargo_process("install foo")
        .env(
            "CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS",
            "fallback",
        )
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] foo v0.0.1 (registry `dummy-registry`)
[INSTALLING] foo v0.0.1
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] dep v1.1.0 (registry `dummy-registry`)
[COMPILING] dep v1.1.0
[COMPILING] foo v0.0.1
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[INSTALLING] [ROOT]/home/.cargo/bin/foo[EXE]
[INSTALLED] package `foo v0.0.1` (executable `foo[EXE]`)
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]])
        .run();
}

#[cargo_test]
fn cargo_install_ignores_resolver_v3_msrv_change() {
    Package::new("dep", "1.0.0")
        .rust_version("1.50")
        .file("src/lib.rs", "fn hello() {}")
        .publish();
    Package::new("dep", "1.1.0")
        .rust_version("1.70")
        .file("src/lib.rs", "fn hello() {}")
        .publish();
    Package::new("foo", "0.0.1")
        .rust_version("1.60")
        .resolver("3")
        .file("src/main.rs", "fn main() {}")
        .dep("dep", "1")
        .publish();

    cargo_process("install foo")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] foo v0.0.1 (registry `dummy-registry`)
[INSTALLING] foo v0.0.1
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] dep v1.1.0 (registry `dummy-registry`)
[COMPILING] dep v1.1.0
[COMPILING] foo v0.0.1
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[INSTALLING] [ROOT]/home/.cargo/bin/foo[EXE]
[INSTALLED] package `foo v0.0.1` (executable `foo[EXE]`)
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]])
        .run();
}

#[cargo_test]
fn report_rust_versions() {
    Package::new("dep-only-low-compatible", "1.55.0")
        .rust_version("1.55.0")
        .file("src/lib.rs", "fn other_stuff() {}")
        .publish();
    Package::new("dep-only-low-incompatible", "1.75.0")
        .rust_version("1.75.0")
        .file("src/lib.rs", "fn other_stuff() {}")
        .publish();
    Package::new("dep-only-high-compatible", "1.65.0")
        .rust_version("1.65.0")
        .file("src/lib.rs", "fn other_stuff() {}")
        .publish();
    Package::new("dep-only-high-incompatible", "1.75.0")
        .rust_version("1.75.0")
        .file("src/lib.rs", "fn other_stuff() {}")
        .publish();
    Package::new("dep-only-unset-unset", "1.0.0")
        .file("src/lib.rs", "fn other_stuff() {}")
        .publish();
    Package::new("dep-only-unset-compatible", "1.75.0")
        .rust_version("1.75.0")
        .file("src/lib.rs", "fn other_stuff() {}")
        .publish();
    Package::new("dep-only-unset-incompatible", "1.2345.0")
        .rust_version("1.2345.0")
        .file("src/lib.rs", "fn other_stuff() {}")
        .publish();
    Package::new("dep-shared-compatible", "1.55.0")
        .rust_version("1.55.0")
        .file("src/lib.rs", "fn other_stuff() {}")
        .publish();
    Package::new("dep-shared-incompatible", "1.75.0")
        .rust_version("1.75.0")
        .file("src/lib.rs", "fn other_stuff() {}")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["high", "low", "unset"]
            "#,
        )
        .file(
            "high/Cargo.toml",
            r#"
                [package]
                name = "high"
                edition = "2015"
                rust-version = "1.70.0"

                [dependencies]
                dep-only-high-compatible = "1"
                dep-only-high-incompatible = "1"
                dep-shared-compatible = "1"
                dep-shared-incompatible = "1"
            "#,
        )
        .file("high/src/main.rs", "fn main(){}")
        .file(
            "low/Cargo.toml",
            r#"
                [package]
                name = "low"
                edition = "2015"
                rust-version = "1.60.0"

                [dependencies]
                dep-only-low-compatible = "1"
                dep-only-low-incompatible = "1"
                dep-shared-compatible = "1"
                dep-shared-incompatible = "1"
            "#,
        )
        .file("low/src/main.rs", "fn main(){}")
        .file(
            "unset/Cargo.toml",
            r#"
                [package]
                name = "unset"
                edition = "2015"

                [dependencies]
                dep-only-unset-unset = "1"
                dep-only-unset-compatible = "1"
                dep-only-unset-incompatible = "1"
                dep-shared-compatible = "1"
                dep-shared-incompatible = "1"
            "#,
        )
        .file("unset/src/main.rs", "fn main(){}")
        .build();

    p.cargo("update")
        .env("CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS", "fallback")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 9 packages to latest Rust 1.60.0 compatible versions
[ADDING] dep-only-high-incompatible v1.75.0 (requires Rust 1.75.0)
[ADDING] dep-only-low-incompatible v1.75.0 (requires Rust 1.75.0)
[ADDING] dep-only-unset-incompatible v1.2345.0 (requires Rust 1.2345.0)
[ADDING] dep-shared-incompatible v1.75.0 (requires Rust 1.75.0)

"#]])
        .run();
}
