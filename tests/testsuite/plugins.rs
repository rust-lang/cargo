//! Tests for rustc plugins.

use cargo_test_support::{basic_manifest, project};
use cargo_test_support::{is_nightly, rustc_host};

#[cargo_test]
fn plugin_to_the_max() {
    if !is_nightly() {
        // plugins are unstable
        return;
    }

    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [lib]
                name = "foo_lib"

                [dependencies.bar]
                path = "../bar"
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                #![feature(plugin)]
                #![plugin(bar)]
                extern crate foo_lib;

                fn main() { foo_lib::foo(); }
            "#,
        )
        .file(
            "src/foo_lib.rs",
            r#"
                #![feature(plugin)]
                #![plugin(bar)]

                pub fn foo() {}
            "#,
        )
        .build();
    let _bar = project()
        .at("bar")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                authors = []

                [lib]
                name = "bar"
                plugin = true

                [dependencies.baz]
                path = "../baz"
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                #![feature(plugin_registrar, rustc_private)]

                extern crate baz;
                extern crate rustc_driver;

                use rustc_driver::plugin::Registry;

                #[plugin_registrar]
                pub fn foo(_reg: &mut Registry) {
                    println!("{}", baz::baz());
                }
            "#,
        )
        .build();
    let _baz = project()
        .at("baz")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "baz"
                version = "0.0.1"
                authors = []

                [lib]
                name = "baz"
                crate_type = ["dylib"]
            "#,
        )
        .file("src/lib.rs", "pub fn baz() -> i32 { 1 }")
        .build();

    foo.cargo("build").run();
    foo.cargo("doc").run();
}

#[cargo_test]
fn plugin_with_dynamic_native_dependency() {
    if !is_nightly() {
        // plugins are unstable
        return;
    }

    let build = project()
        .at("builder")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "builder"
                version = "0.0.1"
                authors = []

                [lib]
                name = "builder"
                crate-type = ["dylib"]
            "#,
        )
        .file("src/lib.rs", "#[no_mangle] pub extern fn foo() {}")
        .build();

    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies.bar]
                path = "bar"
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                #![feature(plugin)]
                #![plugin(bar)]

                fn main() {}
            "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                authors = []
                build = 'build.rs'

                [lib]
                name = "bar"
                plugin = true
            "#,
        )
        .file(
            "bar/build.rs",
            r#"
                use std::env;
                use std::fs;
                use std::path::PathBuf;

                fn main() {
                    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
                    let root = PathBuf::from(env::var("BUILDER_ROOT").unwrap());
                    let file = format!("{}builder{}",
                        env::consts::DLL_PREFIX,
                        env::consts::DLL_SUFFIX);
                    let src = root.join(&file);
                    let dst = out_dir.join(&file);
                    fs::copy(src, dst).unwrap();
                    if cfg!(target_env = "msvc") {
                        fs::copy(root.join("builder.dll.lib"),
                                 out_dir.join("builder.dll.lib")).unwrap();
                    }
                    println!("cargo:rustc-flags=-L {}", out_dir.display());
                }
            "#,
        )
        .file(
            "bar/src/lib.rs",
            r#"
                #![feature(plugin_registrar, rustc_private)]

                extern crate rustc_driver;
                use rustc_driver::plugin::Registry;

                #[cfg_attr(not(target_env = "msvc"), link(name = "builder"))]
                #[cfg_attr(target_env = "msvc", link(name = "builder.dll"))]
                extern { fn foo(); }

                #[plugin_registrar]
                pub fn bar(_reg: &mut Registry) {
                    unsafe { foo() }
                }
            "#,
        )
        .build();

    build.cargo("build").run();

    let root = build.root().join("target").join(rustc_host()).join("debug");
    foo.cargo("build -v").env("BUILDER_ROOT", root).run();
}

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

    p.cargo("build -v").run();
}
