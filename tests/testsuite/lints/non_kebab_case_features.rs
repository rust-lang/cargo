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
foo_bar = []

[lints.cargo]
non_kebab_case_features = "warn"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    foo.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints", "test-dummy-unstable"])
        .with_stderr_data(str![[r#"
[WARNING] features should have a kebab-case name
 --> Cargo.toml:9:1
  |
9 | foo_bar = []
  | ^^^^^^^
  |
  = [NOTE] `cargo::non_kebab_case_features` is set to `warn` in `[lints]`
[HELP] to change the feature name to kebab case, convert the `features` key
  |
9 - foo_bar = []
9 + foo-bar = []
  |
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn feature_name_implicit() {
    Package::new("foo_bar", "0.0.1").publish();

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
foo_bar = { version = "0.0.1", optional = true }

[lints.cargo]
non_kebab_case_features = "warn"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    foo.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints", "test-dummy-unstable"])
        .with_stderr_data(str![[r#"
[WARNING] features should have a kebab-case name
 --> Cargo.toml:9:1
  |
9 | foo_bar = { version = "0.0.1", optional = true }
  | ^^^^^^^ source of feature name --------------- cause of feature
  |
  = [NOTE] see also <https://doc.rust-lang.org/cargo/reference/features.html#optional-dependencies>
  = [NOTE] `cargo::non_kebab_case_features` is set to `warn` in `[lints]`
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
