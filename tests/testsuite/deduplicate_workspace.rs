//! Tests for deduplicating Cargo.toml fields with { workspace = true }
use cargo_test_support::{basic_workspace_manifest, git, paths, project, publish, registry};

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
            gitlab = { repository = "https://gitlab.com/rust-lang/rust", branch = "master" }

            [workspace.dependencies]
            dep1 = "0.1"
        "#,
        )
        .file("bar/Cargo.toml", &basic_workspace_manifest("bar", ".."))
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("build")
        // Should not warn about unused fields.
        .with_stderr(
            "\
[COMPILING] bar v0.1.0 ([CWD]/bar)
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
        .file("bar/Cargo.toml", &basic_workspace_manifest("bar", ".."))
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]foo/Cargo.toml`

Caused by:
  dep1 is optional, but workspace dependencies cannot be optional
",
        )
        .run();
}

#[cargo_test]
fn inherit_workspace_fields() {
    registry::init();

    let p = project().build();

    let _ = git::repo(&paths::root().join("foo"))
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
            publish = true
            edition = "2018"

            [workspace.badges]
            gitlab = { repository = "https://gitlab.com/rust-lang/rust", branch = "master" }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
            badges = { workspace = true }

            [package]
            name = "bar"
            workspace = ".."
            version = { workspace = true }
            authors = { workspace = true }
            description = { workspace = true }
            documentation = { workspace = true }
            readme = { workspace = true }
            homepage = { workspace = true }
            repository = { workspace = true }
            license = { workspace = true }
            license-file = { workspace = true }
            keywords = { workspace = true }
            categories = { workspace = true }
            publish = { workspace = true }
            edition = { workspace = true }
        "#,
        )
        .file("LICENSE", "license")
        .file("README.md", "README.md")
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish --token sekrit").cwd("bar").run();
    publish::validate_upload(
        r#"
        {
          "authors": ["Rustaceans"],
          "badges": {
            "gitlab": { "branch": "master", "repository": "https://gitlab.com/rust-lang/rust" }
          },
          "categories": ["development-tools"],
          "deps": [],
          "description": "This is a crate",
          "documentation": "https://www.rust-lang.org/learn",
          "features": {},
          "homepage": "https://www.rust-lang.org",
          "keywords": ["cli"],
          "license": "MIT",
          "license_file": "../LICENSE",
          "links": null,
          "name": "bar",
          "readme": "README.md",
          "readme_file": "../README.md",
          "repository": "https://github.com/example/example",
          "vers": "1.2.3"
          }
        "#,
        "bar-1.2.3.crate",
        &[
            "Cargo.lock",
            "Cargo.toml",
            "Cargo.toml.orig",
            "src/main.rs",
            "README.md",
            "LICENSE",
            ".cargo_vcs_info.json",
        ],
    );
}

#[cargo_test]
fn inherit_own_workspace_fields() {
    registry::init();

    let p = project().build();

    let _ = git::repo(&paths::root().join("foo"))
        .file(
            "Cargo.toml",
            r#"
            badges = { workspace = true }

            [package]
            name = "foo"
            version = { workspace = true }
            authors = { workspace = true }
            description = { workspace = true }
            documentation = { workspace = true }
            readme = { workspace = true }
            homepage = { workspace = true }
            repository = { workspace = true }
            license = { workspace = true }
            license-file = { workspace = true }
            keywords = { workspace = true }
            categories = { workspace = true }
            publish = { workspace = true }
            edition = { workspace = true }

            [workspace]
            members = []
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
            publish = true
            edition = "2018"

            [workspace.badges]
            gitlab = { repository = "https://gitlab.com/rust-lang/rust", branch = "master" }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("LICENSE", "license")
        .file("README.md", "README.md")
        .build();

    p.cargo("publish --token sekrit").run();
    publish::validate_upload(
        r#"
        {
          "authors": ["Rustaceans"],
          "badges": {
            "gitlab": { "branch": "master", "repository": "https://gitlab.com/rust-lang/rust" }
          },
          "categories": ["development-tools"],
          "deps": [],
          "description": "This is a crate",
          "documentation": "https://www.rust-lang.org/learn",
          "features": {},
          "homepage": "https://www.rust-lang.org",
          "keywords": ["cli"],
          "license": "MIT",
          "license_file": "./LICENSE",
          "links": null,
          "name": "foo",
          "readme": "README.md",
          "readme_file": "README.md",
          "repository": "https://github.com/example/example",
          "vers": "1.2.3"
          }
        "#,
        "foo-1.2.3.crate",
        &[
            "Cargo.lock",
            "Cargo.toml",
            "Cargo.toml.orig",
            "src/main.rs",
            "README.md",
            "LICENSE",
            ".cargo_vcs_info.json",
        ],
    );
}

#[cargo_test]
fn error_inherit_from_undefined_field() {
    registry::init();

    let p = project().build();

    let _ = git::repo(&paths::root().join("foo"))
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["bar"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
            [package]
            name = "bar"
            workspace = ".."
            version = "1.2.3"
            authors = ["rustaceans"]
            description = { workspace = true }
        "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("build")
        .cwd("bar")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[CWD]/Cargo.toml`

Caused by:
  error reading description: workspace root does not define [workspace.description]
",
        )
        .run();
}

#[cargo_test]
fn error_workspace_false() {
    registry::init();

    let p = project().build();

    let _ = git::repo(&paths::root().join("foo"))
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["bar"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
            [package]
            name = "bar"
            workspace = ".."
            version = "1.2.3"
            authors = ["rustaceans"]
            description = { workspace = false }
        "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("build")
        .cwd("bar")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[CWD]/Cargo.toml`

Caused by:
  workspace cannot be false for key `package.description`
",
        )
        .run();
}

#[cargo_test]
fn error_no_root_workspace() {
    registry::init();

    let p = project().build();

    let _ = git::repo(&paths::root().join("foo"))
        .file(
            "bar/Cargo.toml",
            r#"
            [package]
            name = "bar"
            workspace = ".."
            version = "1.2.3"
            authors = ["rustaceans"]
            description = { workspace = true }
        "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("build")
        .cwd("bar")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[CWD]/Cargo.toml`

Caused by:
  error reading description: could not read workspace root
",
        )
        .run();
}

#[cargo_test]
fn error_badges_wrapping() {
    registry::init();

    let p = project().build();

    let _ = git::repo(&paths::root().join("foo"))
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "1.2.3"
            authors = ["rustaceans"]

            [badges]
            gitlab = "1.2.3"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[CWD]/Cargo.toml`

Caused by:
  expected a table of badges or { workspace = true } for key `badges`
",
        )
        .run();
}
