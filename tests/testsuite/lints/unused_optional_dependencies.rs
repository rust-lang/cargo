#![allow(deprecated)]

use cargo_test_support::project;
use cargo_test_support::registry::Package;

#[cargo_test(nightly, reason = "edition2024 is not stable")]
fn default() {
    Package::new("bar", "0.1.0").publish();
    Package::new("baz", "0.1.0").publish();
    Package::new("target-dep", "0.1.0").publish();
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

[build-dependencies]
baz = { version = "0.1.0", optional = true }

[target.'cfg(target_os = "linux")'.dependencies]
target-dep = { version = "0.1.0", optional = true }
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints", "edition2024"])
        .with_stderr(
            "\
warning: unused optional dependency
 --> Cargo.toml:9:1
  |
9 | bar = { version = \"0.1.0\", optional = true }
  | ---
  |
  = note: `cargo::unused_optional_dependency` is set to `warn` by default
  = help: remove the dependency or activate it in a feature with `dep:bar`
warning: unused optional dependency
  --> Cargo.toml:12:1
   |
12 | baz = { version = \"0.1.0\", optional = true }
   | ---
   |
   = help: remove the dependency or activate it in a feature with `dep:baz`
warning: unused optional dependency
  --> Cargo.toml:15:1
   |
15 | target-dep = { version = \"0.1.0\", optional = true }
   | ----------
   |
   = help: remove the dependency or activate it in a feature with `dep:target-dep`
[CHECKING] foo v0.1.0 ([CWD])
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn edition_2021() {
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

[lints.cargo]
implicit_features = "allow"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr(
            "\
[UPDATING] [..]
[LOCKING] 2 packages to latest compatible versions
[CHECKING] foo v0.1.0 ([CWD])
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test(nightly, reason = "edition2024 is not stable")]
fn renamed_deps() {
    Package::new("bar", "0.1.0").publish();
    Package::new("bar", "0.2.0").publish();
    Package::new("target-dep", "0.1.0").publish();
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

[build-dependencies]
baz = { version = "0.2.0", package = "bar", optional = true }

[target.'cfg(target_os = "linux")'.dependencies]
target-dep = { version = "0.1.0", optional = true }
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints", "edition2024"])
        .with_stderr(
            "\
warning: unused optional dependency
 --> Cargo.toml:9:1
  |
9 | bar = { version = \"0.1.0\", optional = true }
  | ---
  |
  = note: `cargo::unused_optional_dependency` is set to `warn` by default
  = help: remove the dependency or activate it in a feature with `dep:bar`
warning: unused optional dependency
  --> Cargo.toml:12:1
   |
12 | baz = { version = \"0.2.0\", package = \"bar\", optional = true }
   | ---
   |
   = help: remove the dependency or activate it in a feature with `dep:baz`
warning: unused optional dependency
  --> Cargo.toml:15:1
   |
15 | target-dep = { version = \"0.1.0\", optional = true }
   | ----------
   |
   = help: remove the dependency or activate it in a feature with `dep:target-dep`
[CHECKING] foo v0.1.0 ([CWD])
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test(nightly, reason = "edition2024 is not stable")]
fn shadowed_optional_dep_is_unused_in_2024() {
    Package::new("optional-dep", "0.1.0").publish();
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
optional-dep = { version = "0.1.0", optional = true }

[features]
optional-dep = []
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints", "edition2024"])
        .with_stderr(
            "\
warning: unused optional dependency
 --> Cargo.toml:9:1
  |
9 | optional-dep = { version = \"0.1.0\", optional = true }
  | ------------
  |
  = note: `cargo::unused_optional_dependency` is set to `warn` by default
  = help: remove the dependency or activate it in a feature with `dep:optional-dep`
[CHECKING] foo v0.1.0 ([CWD])
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test(nightly, reason = "edition2024 is not stable")]
fn inactive_weak_optional_dep() {
    Package::new("dep_name", "0.1.0")
        .feature("dep_feature", &[])
        .publish();

    // `dep_name`` is included as a weak optional dependency throught speficying the `dep_name?/dep_feature` in feature table.
    // In edition2024, `dep_name` need to be add `dep:dep_name` to feature table to speficying activate it.

    // This test explain the conclusion mentioned above
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
        dep_name = { version = "0.1.0", optional = true }

        [features]
        foo_feature = ["dep:dep_name", "dep_name?/dep_feature"]
    "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints", "edition2024"])
        .run();

    // This test proves no regression when dep_name isn't included
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

            [features]
            foo_feature = ["dep_name?/dep_feature"]
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints", "edition2024"])
        .with_status(101)
        .with_stderr(
            "\
error: failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  feature `foo_feature` includes `dep_name?/dep_feature`, but `dep_name` is not a dependency
",
        )
        .run();

    // This test is that we need to improve in edition2024, we need to tell that a weak optioanl dependency needs specify
    // the `dep:` syntax, like `dep:dep_name`.
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
                dep_name = { version = "0.1.0", optional = true }

                [features]
                foo_feature = ["dep_name?/dep_feature"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints", "edition2024"])
        .with_status(101)
        .with_stderr(
            "\
error: failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  feature `foo_feature` includes `dep_name?/dep_feature`, but `dep_name` is not a dependency
",
        )
        .run();
}
