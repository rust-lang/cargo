extern crate cargotest;
extern crate hamcrest;

use cargotest::is_nightly;
use cargotest::support::{execs, project};
use hamcrest::assert_that;

#[test]
fn check_success() {
    if !is_nightly() {
        return
    }
    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "../bar"
        "#)
        .file("src/main.rs", r#"
            extern crate bar;
            fn main() {
                ::bar::baz();
            }
        "#);
    let bar = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.1.0"
            authors = []
        "#)
        .file("src/lib.rs", r#"
            pub fn baz() {}
        "#);
    bar.build();

    assert_that(foo.cargo_process("check"),
                execs().with_status(0));
}

#[test]
fn check_fail() {
    if !is_nightly() {
        return
    }
    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "../bar"
        "#)
        .file("src/main.rs", r#"
            extern crate bar;
            fn main() {
                ::bar::baz(42);
            }
        "#);
    let bar = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.1.0"
            authors = []
        "#)
        .file("src/lib.rs", r#"
            pub fn baz() {}
        "#);
    bar.build();

    assert_that(foo.cargo_process("check"),
                execs().with_status(101));
}

#[test]
fn custom_derive() {
    if !is_nightly() {
        return
    }
    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "../bar"
        "#)
        .file("src/main.rs", r#"
#![feature(proc_macro)]

#[macro_use]
extern crate bar;

trait B {
    fn b(&self);
}

#[derive(B)]
struct A;

fn main() {
    let a = A;
    a.b();
}
"#);
    let bar = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.1.0"
            authors = []
            [lib]
            proc-macro = true
        "#)
        .file("src/lib.rs", r#"
#![feature(proc_macro, proc_macro_lib)]
#![crate_type = "proc-macro"]

extern crate proc_macro;

use proc_macro::TokenStream;

#[proc_macro_derive(B)]
pub fn derive(_input: TokenStream) -> TokenStream {
    format!("impl B for A {{ fn b(&self) {{}} }}").parse().unwrap()
}
"#);
    bar.build();

    assert_that(foo.cargo_process("check"),
                execs().with_status(0));
}

#[test]
fn check_build() {
    if !is_nightly() {
        return;
    }

    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "../bar"
        "#)
        .file("src/main.rs", r#"
            extern crate bar;
            fn main() {
                ::bar::baz();
            }
        "#);
    let bar = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.1.0"
            authors = []
        "#)
        .file("src/lib.rs", r#"
            pub fn baz() {}
        "#);
    bar.build();

    assert_that(foo.cargo_process("check"),
                execs().with_status(0));
    assert_that(foo.cargo_process("build"),
                execs().with_status(0));
}

#[test]
fn build_check() {
    if !is_nightly() {
        return;
    }

    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "../bar"
        "#)
        .file("src/main.rs", r#"
            extern crate bar;
            fn main() {
                ::bar::baz();
            }
        "#);
    let bar = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.1.0"
            authors = []
        "#)
        .file("src/lib.rs", r#"
            pub fn baz() {}
        "#);
    bar.build();

    assert_that(foo.cargo_process("build"),
                execs().with_status(0));
    assert_that(foo.cargo_process("check"),
                execs().with_status(0));
}
