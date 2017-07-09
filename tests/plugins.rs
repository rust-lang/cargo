extern crate cargotest;
extern crate hamcrest;

use std::fs;
use std::env;

use cargotest::{is_nightly, rustc_host};
use cargotest::support::{project, execs};
use hamcrest::assert_that;

#[test]
fn plugin_to_the_max() {
    if !is_nightly() { return }

    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [lib]
            name = "foo_lib"

            [dependencies.bar]
            path = "../bar"
        "#)
        .file("src/main.rs", r#"
            #![feature(plugin)]
            #![plugin(bar)]
            extern crate foo_lib;

            fn main() { foo_lib::foo(); }
        "#)
        .file("src/foo_lib.rs", r#"
            #![feature(plugin)]
            #![plugin(bar)]

            pub fn foo() {}
        "#);
    let bar = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [lib]
            name = "bar"
            plugin = true

            [dependencies.baz]
            path = "../baz"
        "#)
        .file("src/lib.rs", r#"
            #![feature(plugin_registrar, rustc_private)]

            extern crate rustc_plugin;
            extern crate baz;

            use rustc_plugin::Registry;

            #[plugin_registrar]
            pub fn foo(_reg: &mut Registry) {
                println!("{}", baz::baz());
            }
        "#);
    let baz = project("baz")
        .file("Cargo.toml", r#"
            [package]
            name = "baz"
            version = "0.0.1"
            authors = []

            [lib]
            name = "baz"
            crate_type = ["dylib"]
        "#)
        .file("src/lib.rs", "pub fn baz() -> i32 { 1 }");
    bar.build();
    baz.build();

    assert_that(foo.cargo_process("build"),
                execs().with_status(0));
    assert_that(foo.cargo("doc"),
                execs().with_status(0));
}

#[test]
fn plugin_with_dynamic_native_dependency() {
    if !is_nightly() { return }

    let workspace = project("ws")
        .file("Cargo.toml", r#"
            [workspace]
            members = ["builder", "foo"]
        "#);
    workspace.build();

    let build = project("ws/builder")
        .file("Cargo.toml", r#"
            [package]
            name = "builder"
            version = "0.0.1"
            authors = []

            [lib]
            name = "builder"
            crate-type = ["dylib"]
        "#)
        .file("src/lib.rs", r#"
            #[no_mangle]
            pub extern fn foo() {}
        "#);
    build.build();

    let foo = project("ws/foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "bar"
        "#)
        .file("src/main.rs", r#"
            #![feature(plugin)]
            #![plugin(bar)]

            fn main() {}
        "#)
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []
            build = 'build.rs'

            [lib]
            name = "bar"
            plugin = true
        "#)
        .file("bar/build.rs", r#"
            use std::path::PathBuf;
            use std::env;

            fn main() {
                let src = PathBuf::from(env::var("SRC").unwrap());
                println!("cargo:rustc-flags=-L {}/deps", src.parent().unwrap().display());
            }
        "#)
        .file("bar/src/lib.rs", r#"
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
        "#);
    foo.build();

    assert_that(build.cargo("build"),
                execs().with_status(0));

    let src = workspace.root().join("target/debug");
    let lib = fs::read_dir(&src).unwrap().map(|s| s.unwrap().path()).find(|lib| {
        let lib = lib.file_name().unwrap().to_str().unwrap();
        lib.starts_with(env::consts::DLL_PREFIX) &&
            lib.ends_with(env::consts::DLL_SUFFIX)
    }).unwrap();

    assert_that(foo.cargo("build").env("SRC", &lib).arg("-v"),
                execs().with_status(0));
}

#[test]
fn plugin_integration() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            build = "build.rs"

            [lib]
            name = "foo"
            plugin = true
            doctest = false
        "#)
        .file("build.rs", "fn main() {}")
        .file("src/lib.rs", "")
        .file("tests/it_works.rs", "");

    assert_that(p.cargo_process("test").arg("-v"),
                execs().with_status(0));
}

#[test]
fn doctest_a_plugin() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = { path = "bar" }
        "#)
        .file("src/lib.rs", r#"
            #[macro_use]
            extern crate bar;
        "#)
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [lib]
            name = "bar"
            plugin = true
        "#)
        .file("bar/src/lib.rs", r#"
            pub fn bar() {}
        "#);

    assert_that(p.cargo_process("test").arg("-v"),
                execs().with_status(0));
}

// See #1515
#[test]
fn native_plugin_dependency_with_custom_ar_linker() {
    let target = rustc_host();

    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [lib]
            plugin = true
        "#)
        .file("src/lib.rs", "");

    let bar = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [dependencies.foo]
            path = "../foo"
        "#)
        .file("src/lib.rs", "")
        .file(".cargo/config", &format!(r#"
            [target.{}]
            ar = "nonexistent-ar"
            linker = "nonexistent-linker"
        "#, target));

    foo.build();
    assert_that(bar.cargo_process("build").arg("--verbose"),
                execs().with_stderr_contains("\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..] -C ar=nonexistent-ar -C linker=nonexistent-linker [..]`
[ERROR] could not exec the linker [..]
"));
}

#[test]
fn panic_abort_plugins() {
    if !is_nightly() {
        return
    }

    let bar = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [profile.dev]
            panic = 'abort'

            [dependencies]
            foo = { path = "foo" }
        "#)
        .file("src/lib.rs", "")
        .file("foo/Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [lib]
            plugin = true
        "#)
        .file("foo/src/lib.rs", r#"
            #![feature(rustc_private)]
            extern crate syntax;
        "#);

    assert_that(bar.cargo_process("build"),
                execs().with_status(0));
}

#[test]
fn shared_panic_abort_plugins() {
    if !is_nightly() {
        return
    }

    let bar = project("top")
        .file("Cargo.toml", r#"
            [package]
            name = "top"
            version = "0.0.1"
            authors = []

            [profile.dev]
            panic = 'abort'

            [dependencies]
            foo = { path = "foo" }
            bar = { path = "bar" }
        "#)
        .file("src/lib.rs", "
            extern crate bar;
        ")
        .file("foo/Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [lib]
            plugin = true

            [dependencies]
            bar = { path = "../bar" }
        "#)
        .file("foo/src/lib.rs", r#"
            #![feature(rustc_private)]
            extern crate syntax;
            extern crate bar;
        "#)
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []
        "#)
        .file("bar/src/lib.rs", "");

    assert_that(bar.cargo_process("build"),
                execs().with_status(0));
}
