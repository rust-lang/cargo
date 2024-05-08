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
