extern crate cargotest;
extern crate hamcrest;

use cargotest::is_nightly;
use cargotest::support::{project, execs};
use hamcrest::assert_that;

#[test]
fn probe_cfg_before_crate_type_discovery() {
    if !is_nightly() {
        return;
    }

    let client = project("client")
        .file("Cargo.toml", r#"
            [package]
            name = "client"
            version = "0.0.1"
            authors = []

            [target.'cfg(not(stage300))'.dependencies.noop]
            path = "../noop"
        "#)
        .file("src/main.rs", r#"
            #![feature(proc_macro)]

            #[macro_use]
            extern crate noop;

            #[derive(Noop)]
            struct X;

            fn main() {}
        "#);
    let noop = project("noop")
        .file("Cargo.toml", r#"
            [package]
            name = "noop"
            version = "0.0.1"
            authors = []

            [lib]
            proc-macro = true
        "#)
        .file("src/lib.rs", r#"
            #![feature(proc_macro, proc_macro_lib)]

            extern crate proc_macro;
            use proc_macro::TokenStream;

            #[proc_macro_derive(Noop)]
            pub fn noop(_input: TokenStream) -> TokenStream {
                "".parse().unwrap()
            }
        "#);
    noop.build();

    assert_that(client.cargo_process("build"),
                execs().with_status(0));
}

#[test]
fn noop() {
    if !is_nightly() {
        return;
    }

    let client = project("client")
        .file("Cargo.toml", r#"
            [package]
            name = "client"
            version = "0.0.1"
            authors = []

            [dependencies.noop]
            path = "../noop"
        "#)
        .file("src/main.rs", r#"
            #![feature(proc_macro)]

            #[macro_use]
            extern crate noop;

            #[derive(Noop)]
            struct X;

            fn main() {}
        "#);
    let noop = project("noop")
        .file("Cargo.toml", r#"
            [package]
            name = "noop"
            version = "0.0.1"
            authors = []

            [lib]
            proc-macro = true
        "#)
        .file("src/lib.rs", r#"
            #![feature(proc_macro, proc_macro_lib)]

            extern crate proc_macro;
            use proc_macro::TokenStream;

            #[proc_macro_derive(Noop)]
            pub fn noop(_input: TokenStream) -> TokenStream {
                "".parse().unwrap()
            }
        "#);
    noop.build();

    assert_that(client.cargo_process("build"),
                execs().with_status(0));
    assert_that(client.cargo("build"),
                execs().with_status(0));
}

#[test]
fn impl_and_derive() {
    if !is_nightly() {
        return;
    }

    let client = project("client")
        .file("Cargo.toml", r#"
            [package]
            name = "client"
            version = "0.0.1"
            authors = []

            [dependencies.transmogrify]
            path = "../transmogrify"
        "#)
        .file("src/main.rs", r#"
            #![feature(proc_macro)]

            #[macro_use]
            extern crate transmogrify;

            trait ImplByTransmogrify {
                fn impl_by_transmogrify(&self) -> bool;
            }

            #[derive(Transmogrify, Debug)]
            struct X { success: bool }

            fn main() {
                let x = X::new();
                assert!(x.impl_by_transmogrify());
                println!("{:?}", x);
            }
        "#);
    let transmogrify = project("transmogrify")
        .file("Cargo.toml", r#"
            [package]
            name = "transmogrify"
            version = "0.0.1"
            authors = []

            [lib]
            proc-macro = true
        "#)
        .file("src/lib.rs", r#"
            #![feature(proc_macro, proc_macro_lib)]

            extern crate proc_macro;
            use proc_macro::TokenStream;

            #[proc_macro_derive(Transmogrify)]
            #[doc(hidden)]
            pub fn transmogrify(input: TokenStream) -> TokenStream {
                "
                    impl X {
                        fn new() -> Self {
                            X { success: true }
                        }
                    }

                    impl ImplByTransmogrify for X {
                        fn impl_by_transmogrify(&self) -> bool {
                            true
                        }
                    }
                ".parse().unwrap()
            }
        "#);
    transmogrify.build();

    assert_that(client.cargo_process("build"),
                execs().with_status(0));
    assert_that(client.cargo("run"),
                execs().with_status(0).with_stdout("X { success: true }"));
}

#[test]
fn plugin_and_proc_macro() {
    if !is_nightly() {
        return;
    }

    let questionable = project("questionable")
        .file("Cargo.toml", r#"
            [package]
            name = "questionable"
            version = "0.0.1"
            authors = []

            [lib]
            plugin = true
            proc-macro = true
        "#)
        .file("src/lib.rs", r#"
            #![feature(plugin_registrar, rustc_private)]
            #![feature(proc_macro, proc_macro_lib)]

            extern crate rustc_plugin;
            use rustc_plugin::Registry;

            extern crate proc_macro;
            use proc_macro::TokenStream;

            #[plugin_registrar]
            pub fn plugin_registrar(reg: &mut Registry) {}

            #[proc_macro_derive(Questionable)]
            pub fn questionable(input: TokenStream) -> TokenStream {
                input
            }
        "#);

    let msg = "  lib.plugin and lib.proc-macro cannot both be true";
    assert_that(questionable.cargo_process("build"),
                execs().with_status(101).with_stderr_contains(msg));
}

#[test]
fn proc_macro_doctest() {
    if !is_nightly() {
        return
    }
    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
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

/// ```
/// assert!(true);
/// ```
#[proc_macro_derive(Bar)]
pub fn derive(_input: TokenStream) -> TokenStream {
    "".parse().unwrap()
}

#[test]
fn a() {
  assert!(true);
}
"#);

    assert_that(foo.cargo_process("test"),
                execs().with_status(0)
                       .with_stdout_contains("test a ... ok")
                       .with_stdout_contains_n("test [..] ... ok", 2));
}
