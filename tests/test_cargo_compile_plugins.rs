use std::io::fs;
use std::os;

use support::{project, execs, cargo_dir};
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
            #![feature(phase)]
            #[phase(plugin)] extern crate bar;
            extern crate foo_lib;

            fn main() { foo_lib::foo(); }
        "#)
        .file("src/foo_lib.rs", r#"
            #![feature(phase)]
            #[phase(plugin)] extern crate bar;

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
            #![feature(plugin_registrar)]

            extern crate rustc;
            extern crate baz;

            use rustc::plugin::Registry;

            #[plugin_registrar]
            pub fn foo(reg: &mut Registry) {
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
        .file("src/lib.rs", "pub fn baz() -> int { 1 }");
    bar.build();
    baz.build();

    assert_that(foo.cargo_process("build"),
                execs().with_status(0));
    assert_that(foo.process(cargo_dir().join("cargo")).arg("doc"),
                execs().with_status(0));
})

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
        .file("src/main.rs", r#"
            use std::io::fs;
            use std::os;

            fn main() {
                let src = Path::new(os::getenv("SRC").unwrap());
                let dst = Path::new(os::getenv("OUT_DIR").unwrap());
                let dst = dst.join(src.filename().unwrap());
                fs::rename(&src, &dst).unwrap();
            }
        "#)
        .file("src/lib.rs", r#"
            #[no_mangle]
            pub extern fn foo() {}
        "#);
    assert_that(build.cargo_process("build"),
                execs().with_status(0).with_stderr(""));
    let src = build.root().join("target");
    let lib = fs::readdir(&src).unwrap().into_iter().find(|lib| {
        let lib = lib.filename_str().unwrap();
        lib.starts_with(os::consts::DLL_PREFIX) &&
            lib.ends_with(os::consts::DLL_SUFFIX)
    }).unwrap();
    let libname = lib.filename_str().unwrap();
    let libname = libname.slice(os::consts::DLL_PREFIX.len(),
                                libname.len() - os::consts::DLL_SUFFIX.len());

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
            #![feature(phase)]
            #[phase(plugin)] extern crate bar;

            fn main() {}
        "#)
        .file("bar/Cargo.toml", format!(r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []
            build = '{}'

            [lib]
            name = "bar"
            plugin = true
        "#, build.bin("builder").display()))
        .file("bar/src/lib.rs", format!(r#"
            #![feature(plugin_registrar)]

            extern crate rustc;

            use rustc::plugin::Registry;

            #[link(name = "{}")]
            extern {{ fn foo(); }}

            #[plugin_registrar]
            pub fn bar(_reg: &mut Registry) {{
                unsafe {{ foo() }}
            }}
        "#, libname));

    assert_that(foo.cargo_process("build").env("SRC", Some(lib.as_vec())),
                execs().with_status(0));
})
