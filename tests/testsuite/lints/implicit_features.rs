use cargo_test_support::prelude::*;
use cargo_test_support::project;
use cargo_test_support::registry::Package;
use cargo_test_support::str;

#[cargo_test]
fn default() {
    Package::new("bar", "0.1.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.1.0"
edition = "2021"

[dependencies]
bar = { version = "0.1.0", optional = true }
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn warn() {
    Package::new("bar", "0.1.0").publish();
    Package::new("baz", "0.1.0").publish();
    Package::new("target-dep", "0.1.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.1.0"
edition = "2021"

[dependencies]
bar = { version = "0.1.0", optional = true }

[build-dependencies]
baz = { version = "0.1.0", optional = true }

[target.'cfg(target_os = "linux")'.dependencies]
target-dep = { version = "0.1.0", optional = true }

[lints.cargo]
implicit_features = "warn"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] implicit features for optional dependencies is deprecated and will be unavailable in the 2024 edition
 --> Cargo.toml:8:1
  |
8 | bar = { version = "0.1.0", optional = true }
  | ---
  |
  = [NOTE] `cargo::implicit_features` is set to `warn` in `[lints]`
[WARNING] implicit features for optional dependencies is deprecated and will be unavailable in the 2024 edition
  --> Cargo.toml:11:1
   |
11 | baz = { version = "0.1.0", optional = true }
   | ---
   |
[WARNING] implicit features for optional dependencies is deprecated and will be unavailable in the 2024 edition
  --> Cargo.toml:14:1
   |
14 | target-dep = { version = "0.1.0", optional = true }
   | ----------
   |
[UPDATING] `dummy-registry` index
[LOCKING] 3 packages to latest compatible versions
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(nightly, reason = "edition2024 is not stable")]
fn implicit_features_edition_2024() {
    Package::new("bar", "0.1.0").publish();
    Package::new("baz", "0.1.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
cargo-features = ["edition2024"]
[package]
name = "foo"
version = "0.1.0"
edition = "2024"

[dependencies]
bar = { version = "0.1.0", optional = true }
baz = { version = "0.1.0", optional = true }

[features]
baz = ["dep:baz"]

[lints.cargo]
unused_optional_dependency = "allow"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints", "edition2024"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest Rust 1.82.0-nightly compatible version
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
