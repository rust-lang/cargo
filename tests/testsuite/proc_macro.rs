//! Tests for proc-macros.

use cargo_test_support::project;

#[cargo_test]
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
    let _noop = project()
        .at("noop")
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

    p.cargo("check").run();
}

#[cargo_test]
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
    let _noop = project()
        .at("noop")
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

    p.cargo("check").run();
    p.cargo("check").run();
}

#[cargo_test]
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
    let _transmogrify = project()
        .at("transmogrify")
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

    p.cargo("build").run();
    p.cargo("run").with_stdout("X { success: true }").run();
}

#[cargo_test(nightly, reason = "plugins are unstable")]
fn plugin_and_proc_macro() {
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
                #![feature(rustc_private)]
                #![feature(proc_macro, proc_macro_lib)]

                extern crate rustc_driver;
                use rustc_driver::plugin::Registry;

                extern crate proc_macro;
                use proc_macro::TokenStream;

                #[no_mangle]
                pub fn __rustc_plugin_registrar(reg: &mut Registry) {}

                #[proc_macro_derive(Questionable)]
                pub fn questionable(input: TokenStream) -> TokenStream {
                    input
                }
            "#,
        )
        .build();

    let msg = "  `lib.plugin` and `lib.proc-macro` cannot both be `true`";
    p.cargo("check")
        .with_status(101)
        .with_stderr_contains(msg)
        .run();
}

#[cargo_test]
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

    foo.cargo("test")
        .with_stdout_contains("test a ... ok")
        .with_stdout_contains_n("test [..] ... ok", 2)
        .run();
}

#[cargo_test]
fn proc_macro_crate_type() {
    // Verify that `crate-type = ["proc-macro"]` is the same as `proc-macro = true`
    // and that everything, including rustdoc, works correctly.
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                [dependencies]
                pm = { path = "pm" }
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                //! ```
                //! use foo::THING;
                //! assert_eq!(THING, 123);
                //! ```
                #[macro_use]
                extern crate pm;
                #[derive(MkItem)]
                pub struct S;
                #[cfg(test)]
                mod tests {
                    use super::THING;
                    #[test]
                    fn it_works() {
                        assert_eq!(THING, 123);
                    }
                }
            "#,
        )
        .file(
            "pm/Cargo.toml",
            r#"
                [package]
                name = "pm"
                version = "0.1.0"
                [lib]
                crate-type = ["proc-macro"]
            "#,
        )
        .file(
            "pm/src/lib.rs",
            r#"
                extern crate proc_macro;
                use proc_macro::TokenStream;

                #[proc_macro_derive(MkItem)]
                pub fn mk_item(_input: TokenStream) -> TokenStream {
                    "pub const THING: i32 = 123;".parse().unwrap()
                }
            "#,
        )
        .build();

    foo.cargo("test")
        .with_stdout_contains("test tests::it_works ... ok")
        .with_stdout_contains_n("test [..] ... ok", 2)
        .run();
}

#[cargo_test]
fn proc_macro_crate_type_warning() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                [lib]
                crate-type = ["proc-macro"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    foo.cargo("check")
        .with_stderr_contains(
            "[WARNING] library `foo` should only specify `proc-macro = true` instead of setting `crate-type`")
        .run();
}

#[cargo_test]
fn proc_macro_conflicting_warning() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                [lib]
                proc-macro = false
                proc_macro = true
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    foo.cargo("check")
        .with_stderr_contains(
"[WARNING] conflicting between `proc-macro` and `proc_macro` in the `foo` library target.\n
        `proc_macro` is ignored and not recommended for use in the future",
        )
        .run();
}

#[cargo_test]
fn proc_macro_crate_type_warning_plugin() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                [lib]
                crate-type = ["proc-macro"]
                plugin = true
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    foo.cargo("check")
        .with_stderr_contains(
            "[WARNING] proc-macro library `foo` should not specify `plugin = true`")
        .with_stderr_contains(
            "[WARNING] library `foo` should only specify `proc-macro = true` instead of setting `crate-type`")
        .run();
}

#[cargo_test]
fn proc_macro_crate_type_multiple() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                [lib]
                crate-type = ["proc-macro", "rlib"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    foo.cargo("check")
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]/foo/Cargo.toml`

Caused by:
  cannot mix `proc-macro` crate type with others
",
        )
        .with_status(101)
        .run();
}

#[cargo_test]
fn proc_macro_extern_prelude() {
    // Check that proc_macro is in the extern prelude.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2018"
            [lib]
            proc-macro = true
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
            use proc_macro::TokenStream;
            #[proc_macro]
            pub fn foo(input: TokenStream) -> TokenStream {
                "".parse().unwrap()
            }
            "#,
        )
        .build();
    p.cargo("test").run();
    p.cargo("doc").run();
}

#[cargo_test]
fn proc_macro_built_once() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ['a', 'b']
                resolver = "2"
            "#,
        )
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.1.0"

                [build-dependencies]
                the-macro = { path = '../the-macro' }
            "#,
        )
        .file("a/build.rs", "fn main() {}")
        .file("a/src/main.rs", "fn main() {}")
        .file(
            "b/Cargo.toml",
            r#"
                [package]
                name = "b"
                version = "0.1.0"

                [dependencies]
                the-macro = { path = '../the-macro', features = ['a'] }
            "#,
        )
        .file("b/src/main.rs", "fn main() {}")
        .file(
            "the-macro/Cargo.toml",
            r#"
                [package]
                name = "the-macro"
                version = "0.1.0"

                [lib]
                proc_macro = true

                [features]
                a = []
            "#,
        )
        .file("the-macro/src/lib.rs", "")
        .build();
    p.cargo("build --verbose")
        .with_stderr_unordered(
            "\
[COMPILING] the-macro [..]
[RUNNING] `rustc --crate-name the_macro [..]`
[COMPILING] b [..]
[RUNNING] `rustc --crate-name b [..]`
[COMPILING] a [..]
[RUNNING] `rustc --crate-name build_script_build [..]`
[RUNNING] `[..]build[..]script[..]build[..]`
[RUNNING] `rustc --crate-name a [..]`
[FINISHED] [..]
",
        )
        .run();
}
