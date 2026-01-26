use crate::prelude::*;
use cargo_test_support::project;
use cargo_test_support::registry::Package;
use cargo_test_support::str;

#[cargo_test]
fn feature_name_explicit() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.0.1"
edition = "2015"
authors = []

[features]
foo-bar = []

[lints.cargo]
non_snake_case_features = "warn"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    foo.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints", "test-dummy-unstable"])
        .with_stderr_data(str![[r#"
[WARNING] unknown lint: `non_snake_case_features`
  --> Cargo.toml:12:1
   |
12 | non_snake_case_features = "warn"
   | ^^^^^^^^^^^^^^^^^^^^^^^
   |
   = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn feature_name_implicit() {
    Package::new("foo-bar", "0.0.1").publish();

    let foo = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.0.1"
edition = "2015"
authors = []

[dependencies]
foo-bar = { version = "0.0.1", optional = true }

[lints.cargo]
non_snake_case_features = "warn"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    foo.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints", "test-dummy-unstable"])
        .with_stderr_data(str![[r#"
[WARNING] unknown lint: `non_snake_case_features`
  --> Cargo.toml:12:1
   |
12 | non_snake_case_features = "warn"
   | ^^^^^^^^^^^^^^^^^^^^^^^
   |
   = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
