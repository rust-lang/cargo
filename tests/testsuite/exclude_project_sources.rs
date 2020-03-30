//! Tests for --exclude-project-sources feature.

use cargo_test_support::registry::Package;
use cargo_test_support::{basic_manifest, git, project};

#[cargo_test]
fn exclude_project_sources_when_there_are_no_external_dependencies() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.1.0"

            [workspace]
            members = ["bar"]

            [dependencies.bar]
            path = "bar"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();

    p.cargo("build")
        .with_stderr_contains("[COMPILING] foo [..]")
        .with_stderr_contains("[COMPILING] bar [..]")
        .run();

    p.cargo("clean").run();

    p.cargo("build --exclude-project-sources -Z unstable-options")
        .masquerade_as_nightly_cargo()
        .with_stderr_does_not_contain("[COMPILING] foo [..]")
        .with_stderr_does_not_contain("[COMPILING] bar [..]")
        .run();
    p.cargo("build")
        .with_stderr_contains("[COMPILING] foo [..]")
        .with_stderr_contains("[COMPILING] bar [..]")
        .run();
}

#[cargo_test]
fn exclude_project_sources_when_there_are_external_dependencies() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.1.0"

            [workspace]
            members = ["bar"]

            [dependencies.bar]
            version = "0.1.0"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();

    Package::new("bar", "0.1.0").publish();

    p.cargo("build")
        .with_stderr_contains("[COMPILING] foo [..]")
        .with_stderr_contains("[COMPILING] bar [..]")
        .run();

    p.cargo("clean").run();

    p.cargo("build --exclude-project-sources -Z unstable-options")
        .masquerade_as_nightly_cargo()
        .with_stderr_does_not_contain("[COMPILING] foo [..]")
        .with_stderr_contains("[COMPILING] bar [..]")
        .run();

    p.cargo("build")
        .with_stderr_contains("[COMPILING] foo [..]")
        .with_stderr_does_not_contain("[COMPILING] bar [..]")
        .run();
}

#[cargo_test]
fn exclude_project_sources_when_there_are_transitive_external_dependencies() {
    let transitive = git::new("baz", |project| {
        project
            .file("Cargo.toml", &basic_manifest("baz", "0.1.0"))
            .file("src/lib.rs", "")
    });

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.1.0"
            authors = []
            [dependencies.bar]
            path = "bar"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            &format!(
                r#"
            [package]
            name = "bar"
            version = "0.1.0"

            [dependencies.baz]
            git = '{}'
        "#,
                transitive.url()
            ),
        )
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("build")
        .with_stderr_contains("[COMPILING] foo [..]")
        .with_stderr_contains("[COMPILING] bar [..]")
        .with_stderr_contains("[COMPILING] baz [..]")
        .run();

    p.cargo("clean").run();

    p.cargo("build --exclude-project-sources -Z unstable-options")
        .masquerade_as_nightly_cargo()
        .with_stderr_does_not_contain("[COMPILING] foo [..]")
        .with_stderr_does_not_contain("[COMPILING] bar [..]")
        .with_stderr_contains("[COMPILING] baz [..]")
        .run();

    p.cargo("build")
        .with_stderr_contains("[COMPILING] foo [..]")
        .with_stderr_contains("[COMPILING] bar [..]")
        .with_stderr_does_not_contain("[COMPILING] baz [..]")
        .run();
}
