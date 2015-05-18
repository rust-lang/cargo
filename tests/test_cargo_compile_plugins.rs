use std::fs;
use std::env;

use support::{project, execs};
use support::{COMPILING, RUNNING};
use hamcrest::assert_that;

fn setup() {
}

test!(plugin_to_the_max {
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

            extern crate rustc;
            extern crate baz;

            use rustc::plugin::Registry;

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
});

test!(plugin_with_dynamic_native_dependency {
    let build = project("builder")
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
    assert_that(build.cargo_process("build"),
                execs().with_status(0).with_stderr(""));
    let src = build.root().join("target/debug");
    let lib = fs::read_dir(&src).unwrap().map(|s| s.unwrap().path()).find(|lib| {
        let lib = lib.file_name().unwrap().to_str().unwrap();
        lib.starts_with(env::consts::DLL_PREFIX) &&
            lib.ends_with(env::consts::DLL_SUFFIX)
    }).unwrap();
    let libname = lib.file_name().unwrap().to_str().unwrap();
    let libname = &libname[env::consts::DLL_PREFIX.len()..
                           libname.len() - env::consts::DLL_SUFFIX.len()];

    let foo = project("foo")
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
            #![feature(convert)]
            use std::path::PathBuf;
            use std::env;

            fn main() {
                let src = PathBuf::from(env::var("SRC").unwrap());
                println!("cargo:rustc-flags=-L {}", src.parent().unwrap()
                                                       .display());
            }
        "#)
        .file("bar/src/lib.rs", &format!(r#"
            #![feature(plugin_registrar, rustc_private)]
            extern crate rustc;

            use rustc::plugin::Registry;

            #[link(name = "{}")]
            extern {{ fn foo(); }}

            #[plugin_registrar]
            pub fn bar(_reg: &mut Registry) {{
                unsafe {{ foo() }}
            }}
        "#, libname));

    assert_that(foo.cargo_process("build").env("SRC", &lib),
                execs().with_status(0));
});

test!(plugin_integration {
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

    assert_that(p.cargo_process("test"),
                execs().with_status(0));
});

test!(doctest_a_plugin {
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
        .file("bar/src/lib.rs", "");

    assert_that(p.cargo_process("test").arg("-v"),
                execs().with_status(0));
});

// See #1515
test!(native_plugin_dependency_with_custom_ar_linker {
    let target = ::rustc_host();

    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [lib]
            name = "foo"
            plugin = true
        "#)
        .file("src/lib.rs", "");

    let bar = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [lib]
            name = "bar"

            [dependencies.foo]
            path = "../foo"
        "#)
        .file("src/lib", "")
        .file(".cargo/config", &format!(r#"
            [target.{}]
            ar = "nonexistent-ar"
            linker = "nonexistent-linker"
        "#, target));

    foo.build();
    assert_that(bar.cargo_process("build").arg("--verbose"),
                execs().with_stdout(&format!("\
{compiling} foo v0.0.1 ({url})
{running} `rustc [..] -C ar=nonexistent-ar -C linker=nonexistent-linker [..]`
", compiling = COMPILING, running = RUNNING, url = bar.url())))
});
