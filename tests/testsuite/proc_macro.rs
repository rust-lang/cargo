use support::is_nightly;
use support::{execs, project};
use support::hamcrest::assert_that;

#[test]
fn probe_cfg_before_crate_type_discovery() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [target.'cfg(not(stage300))'.dependencies.noop]
            path = "../noop"
        "#,
        )
        .file(
            "src/main.rs",
            r#"
            #[macro_use]
            extern crate noop;

            #[derive(Noop)]
            struct X;

            fn main() {}
        "#,
        )
        .build();
    let _noop = project().at("noop")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "noop"
            version = "0.0.1"
            authors = []

            [lib]
            proc-macro = true
        "#,
        )
        .file(
            "src/lib.rs",
            r#"
            extern crate proc_macro;
            use proc_macro::TokenStream;

            #[proc_macro_derive(Noop)]
            pub fn noop(_input: TokenStream) -> TokenStream {
                "".parse().unwrap()
            }
        "#,
        )
        .build();

    assert_that(p.cargo("build"), execs());
}

#[test]
fn noop() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.noop]
            path = "../noop"
        "#,
        )
        .file(
            "src/main.rs",
            r#"
            #[macro_use]
            extern crate noop;

            #[derive(Noop)]
            struct X;

            fn main() {}
        "#,
        )
        .build();
    let _noop = project().at("noop")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "noop"
            version = "0.0.1"
            authors = []

            [lib]
            proc-macro = true
        "#,
        )
        .file(
            "src/lib.rs",
            r#"
            extern crate proc_macro;
            use proc_macro::TokenStream;

            #[proc_macro_derive(Noop)]
            pub fn noop(_input: TokenStream) -> TokenStream {
                "".parse().unwrap()
            }
        "#,
        )
        .build();

    assert_that(p.cargo("build"), execs());
    assert_that(p.cargo("build"), execs());
}

#[test]
fn impl_and_derive() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.transmogrify]
            path = "../transmogrify"
        "#,
        )
        .file(
            "src/main.rs",
            r#"
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
        "#,
        )
        .build();
    let _transmogrify = project().at("transmogrify")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "transmogrify"
            version = "0.0.1"
            authors = []

            [lib]
            proc-macro = true
        "#,
        )
        .file(
            "src/lib.rs",
            r#"
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
        "#,
        )
        .build();

    assert_that(p.cargo("build"), execs());
    assert_that(
        p.cargo("run"),
        execs().with_stdout("X { success: true }"),
    );
}

#[test]
fn plugin_and_proc_macro() {
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

            [lib]
            plugin = true
            proc-macro = true
        "#,
        )
        .file(
            "src/lib.rs",
            r#"
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
        "#,
        )
        .build();

    let msg = "  lib.plugin and lib.proc-macro cannot both be true";
    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr_contains(msg),
    );
}

#[test]
fn proc_macro_doctest() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
            [lib]
            proc-macro = true
        "#,
        )
        .file(
            "src/lib.rs",
            r#"
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
"#,
        )
        .build();

    assert_that(
        foo.cargo("test"),
        execs()
            .with_stdout_contains("test a ... ok")
            .with_stdout_contains_n("test [..] ... ok", 2),
    );
}
