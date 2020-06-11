//! Tests for rustc plugins.

use cargo_test_support::{basic_manifest, project};
use cargo_test_support::{is_nightly, rustc_host};

#[cargo_test]
fn plugin_integration() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            build = "build.rs"

            [lib]
            name = "foo"
            plugin = true
            doctest = false
        "#,
        )
        .file("build.rs", "fn main() {}")
        .file("src/lib.rs", "")
        .file("tests/it_works.rs", "")
        .build();

    p.cargo("test -v").run();
}

#[cargo_test]
fn doctest_a_plugin() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = { path = "bar" }
        "#,
        )
        .file("src/lib.rs", "#[macro_use] extern crate bar;")
        .file(
            "bar/Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [lib]
            name = "bar"
            plugin = true
        "#,
        )
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();

    p.cargo("test -v").run();
}

// See #1515
#[cargo_test]
fn native_plugin_dependency_with_custom_linker() {
    let target = rustc_host();

    let _foo = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [lib]
            plugin = true
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    let bar = project()
        .at("bar")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [dependencies.foo]
            path = "../foo"
        "#,
        )
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            &format!(
                r#"
            [target.{}]
            linker = "nonexistent-linker"
        "#,
                target
            ),
        )
        .build();

    bar.cargo("build --verbose")
        .with_status(101)
        .with_stderr_contains(
            "\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..] -C linker=nonexistent-linker [..]`
[ERROR] [..]linker[..]
",
        )
        .run();
}

#[cargo_test]
fn panic_abort_plugins() {
    if !is_nightly() {
        // requires rustc_private
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [profile.dev]
            panic = 'abort'

            [dependencies]
            bar = { path = "bar" }
        "#,
        )
        .file("src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [lib]
            plugin = true
        "#,
        )
        .file(
            "bar/src/lib.rs",
            r#"
            #![feature(rustc_private)]
            extern crate rustc_ast;
        "#,
        )
        .build();

    p.cargo("build").run();
}

#[cargo_test]
fn shared_panic_abort_plugins() {
    if !is_nightly() {
        // requires rustc_private
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [profile.dev]
            panic = 'abort'

            [dependencies]
            bar = { path = "bar" }
            baz = { path = "baz" }
        "#,
        )
        .file("src/lib.rs", "extern crate baz;")
        .file(
            "bar/Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [lib]
            plugin = true

            [dependencies]
            baz = { path = "../baz" }
        "#,
        )
        .file(
            "bar/src/lib.rs",
            r#"
            #![feature(rustc_private)]
            extern crate rustc_ast;
            extern crate baz;
        "#,
        )
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.0.1"))
        .file("baz/src/lib.rs", "")
        .build();

    p.cargo("build").run();
}
