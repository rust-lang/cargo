extern crate cargotest;
extern crate hamcrest;

use cargotest::is_nightly;
use cargotest::support::{execs, project};
use hamcrest::assert_that;

#[test]
fn check_success() {
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

    let expected = if is_nightly() { 0 } else { 101 };
    assert_that(foo.cargo_process("check"),
                execs().with_status(expected));
}

#[test]
fn check_fail() {
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

    let expected = if is_nightly() { 0 } else { 101 };
    assert_that(foo.cargo_process("check"),
                execs().with_status(expected));
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

// Checks that where a project has both a lib and a bin, the lib is only checked
// not built.
#[test]
fn issue_3418() {
    if !is_nightly() {
        return;
    }

    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []

            [dependencies]
        "#)
        .file("src/lib.rs", "")
        .file("src/main.rs", "fn main() {}");
    foo.build();

    assert_that(foo.cargo_process("check").arg("-v"),
                execs().with_status(0)
                       .with_stderr_does_not_contain("--crate-type lib"));
}
