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

    assert_that(foo.cargo_process("cargo-build"),
                execs().with_status(0));
    assert_that(foo.process(cargo_dir().join("cargo-doc")),
                execs().with_status(0));
})
