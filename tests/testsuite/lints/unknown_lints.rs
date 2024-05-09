use cargo_test_support::project;

#[cargo_test]
fn default() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.0.1"
edition = "2015"
authors = []

[lints.cargo]
this-lint-does-not-exist = "warn"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr(
            "\
warning: unknown lint: `this-lint-does-not-exist`
 --> Cargo.toml:9:1
  |
9 | this-lint-does-not-exist = \"warn\"
  | ^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `cargo::unknown_lints` is set to `warn` by default
[CHECKING] foo v0.0.1 ([CWD])
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn inherited() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[workspace]
members = ["foo"]

[workspace.lints.cargo]
this-lint-does-not-exist = "warn"
"#,
        )
        .file(
            "foo/Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.0.1"
edition = "2015"
authors = []

[lints]
workspace = true
            "#,
        )
        .file("foo/src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr(
            "\
warning: unknown lint: `this-lint-does-not-exist`
 --> Cargo.toml:6:1
  |
6 | this-lint-does-not-exist = \"warn\"
  | ^^^^^^^^^^^^^^^^^^^^^^^^
  |
note: `cargo::this-lint-does-not-exist` was inherited
 --> foo/Cargo.toml:9:1
  |
9 | workspace = true
  | ----------------
  |
  = note: `cargo::unknown_lints` is set to `warn` by default
[CHECKING] foo v0.0.1 ([CWD]/foo)
[FINISHED] [..]
",
        )
        .run();
}
