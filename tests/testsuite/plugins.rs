use std::fs;
use std::env;

use support::{is_nightly, rustc_host};
use support::{basic_manifest, execs, project};
use support::hamcrest::assert_that;

#[test]
fn plugin_to_the_max() {
    if !is_nightly() {
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
    let _bar = project().at("bar")
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

            extern crate rustc_plugin;
            extern crate baz;

            use rustc_plugin::Registry;

            #[plugin_registrar]
            pub fn foo(_reg: &mut Registry) {
                println!("{}", baz::baz());
            }
        "#,
        )
        .build();
    let _baz = project().at("baz")
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

    assert_that(foo.cargo("build"), execs());
    assert_that(foo.cargo("doc"), execs());
}

#[test]
fn plugin_with_dynamic_native_dependency() {
    if !is_nightly() {
        return;
    }

    let workspace = project().at("ws")
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["builder", "foo"]
        "#,
        )
        .build();

    let build = project().at("ws/builder")
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

    let foo = project().at("ws/foo")
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
            use std::path::PathBuf;
            use std::env;

            fn main() {
                let src = PathBuf::from(env::var("SRC").unwrap());
                println!("cargo:rustc-flags=-L {}/deps", src.parent().unwrap().display());
            }
        "#,
        )
        .file(
            "bar/src/lib.rs",
            r#"
            #![feature(plugin_registrar, rustc_private)]
            extern crate rustc_plugin;

            use rustc_plugin::Registry;

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

    assert_that(build.cargo("build"), execs());

    let src = workspace.root().join("target/debug");
    let lib = fs::read_dir(&src)
        .unwrap()
        .map(|s| s.unwrap().path())
        .find(|lib| {
            let lib = lib.file_name().unwrap().to_str().unwrap();
            lib.starts_with(env::consts::DLL_PREFIX) && lib.ends_with(env::consts::DLL_SUFFIX)
        })
        .unwrap();

    assert_that(
        foo.cargo("build -v").env("SRC", &lib),
        execs(),
    );
}

#[test]
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

    assert_that(p.cargo("test -v"), execs());
}

#[test]
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

    assert_that(p.cargo("test -v"), execs());
}

// See #1515
#[test]
fn native_plugin_dependency_with_custom_ar_linker() {
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

    let bar = project().at("bar")
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
            ar = "nonexistent-ar"
            linker = "nonexistent-linker"
        "#,
                target
            ),
        )
        .build();

    assert_that(
        bar.cargo("build --verbose"),
        execs().with_status(101).with_stderr_contains(
            "\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..] -C ar=nonexistent-ar -C linker=nonexistent-linker [..]`
[ERROR] [..]linker[..]
",
        ),
    );
}

#[test]
fn panic_abort_plugins() {
    if !is_nightly() {
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
            extern crate syntax;
        "#,
        )
        .build();

    assert_that(p.cargo("build"), execs());
}

#[test]
fn shared_panic_abort_plugins() {
    if !is_nightly() {
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
            extern crate syntax;
            extern crate baz;
        "#,
        )
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.0.1"))
        .file("baz/src/lib.rs", "")
        .build();

    assert_that(p.cargo("build"), execs());
}
