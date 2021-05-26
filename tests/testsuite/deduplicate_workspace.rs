//! Tests for deduplicating Cargo.toml fields with { workspace = true }
use cargo_test_support::project;

#[cargo_test]
fn permit_additional_workspace_fields() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["bar"]
            version = "1.2.3"
            authors = ["Rustaceans"]
            description = "This is a crate"
            documentation = "https://www.rust-lang.org/learn"
            readme = "README.md"
            homepage = "https://www.rust-lang.org"
            repository = "https://github.com/example/example"
            license = "MIT"
            license-file = "./LICENSE"
            keywords = ["cli"]
            categories = ["development-tools"]
            publish = false
            edition = "2018"

            [workspace.badges]
            gitlab = { repository = "https://gitlab.com/rust-lang/rusu", branch = "master" }

            [workspace.dependencies]
            dep1 = "0.1"
        "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.2.0"
            authors = []
            workspace = ".."
        "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("build")
        // Should not warn about unused fields.
        .with_stderr(
            "\
[COMPILING] bar v0.2.0 ([CWD]/bar)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    p.cargo("check").run();
    let lockfile = p.read_lockfile();
    assert!(!lockfile.contains("dep1"));
}

#[cargo_test]
fn deny_optional_dependencies() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["bar"]

            [workspace.dependencies]
            dep1 = { version = "0.1", optional = true }
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.2.0"
            authors = []
            workspace = ".."
        "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]Cargo.toml`

Caused by:
  dep1 is optional, but workspace dependencies cannot be optional
",
        )
        .run();
}
