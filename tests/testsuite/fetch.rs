//! Tests for the `cargo fetch` command.

use crate::prelude::*;
use crate::utils::cross_compile::disabled as cross_compile_disabled;
use cargo_test_support::registry::Package;
use cargo_test_support::rustc_host;
use cargo_test_support::{basic_manifest, cross_compile, project, str};

#[cargo_test]
fn no_deps() {
    let p = project()
        .file("src/main.rs", "mod a; fn main() {}")
        .file("src/a.rs", "")
        .build();

    p.cargo("fetch").with_stderr_data("").run();
}

#[cargo_test]
fn fetch_all_platform_dependencies_when_no_target_is_given() {
    if cross_compile_disabled() {
        return;
    }

    Package::new("d1", "1.2.3")
        .file("Cargo.toml", &basic_manifest("d1", "1.2.3"))
        .file("src/lib.rs", "")
        .publish();

    Package::new("d2", "0.1.2")
        .file("Cargo.toml", &basic_manifest("d2", "0.1.2"))
        .file("src/lib.rs", "")
        .publish();

    let target = cross_compile::alternate();
    let host = rustc_host();
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.1"
                    edition = "2015"
                    authors = []

                    [target.{host}.dependencies]
                    d1 = "1.2.3"

                    [target.{target}.dependencies]
                    d2 = "0.1.2"
                "#,
                host = host,
                target = target
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("fetch")
        .with_stderr_data(str![[r#"
...
[DOWNLOADED] d1 v1.2.3 (registry `dummy-registry`)
[DOWNLOADED] d2 v0.1.2 (registry `dummy-registry`)
...
"#]])
        .run();
}

#[cargo_test]
fn fetch_platform_specific_dependencies() {
    if cross_compile_disabled() {
        return;
    }

    Package::new("d1", "1.2.3")
        .file("Cargo.toml", &basic_manifest("d1", "1.2.3"))
        .file("src/lib.rs", "")
        .publish();

    Package::new("d2", "0.1.2")
        .file("Cargo.toml", &basic_manifest("d2", "0.1.2"))
        .file("src/lib.rs", "")
        .publish();

    let target = cross_compile::alternate();
    let host = rustc_host();
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.1"
                    edition = "2015"
                    authors = []

                    [target.{host}.dependencies]
                    d1 = "1.2.3"

                    [target.{target}.dependencies]
                    d2 = "0.1.2"
                "#,
                host = host,
                target = target
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("fetch --target")
        .arg(&host)
        .with_stderr_contains("[DOWNLOADED] d1 v1.2.3 [..]")
        .with_stderr_does_not_contain("[DOWNLOADED] d2 v0.1.2 [..]")
        .run();

    p.cargo("fetch --target")
        .arg(&target)
        .with_stderr_contains("[DOWNLOADED] d2 v0.1.2[..]")
        .with_stderr_does_not_contain("[DOWNLOADED] d1 v1.2.3 [..]")
        .run();
}

#[cargo_test]
fn fetch_warning() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "1.0.0"
            edition = "2015"
            misspelled = "wut"
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("fetch")
        .with_stderr_data(str![[r#"
[WARNING] unused manifest key: package.misspelled

"#]])
        .run();
}
