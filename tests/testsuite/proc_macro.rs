//! Tests for proc-macros.

use crate::prelude::*;
use cargo_test_support::project;
use cargo_test_support::str;

#[cargo_test]
fn probe_cfg_before_crate_type_discovery() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
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
                edition = "2015"
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
                edition = "2015"
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
                edition = "2015"
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
                edition = "2015"
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
                edition = "2015"
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
    p.cargo("run")
        .with_stdout_data(str![[r#"
X { success: true }

"#]])
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
                edition = "2015"
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
        .with_stdout_data(str![[r#"

running 1 test
test a ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s


running 1 test
test src/lib.rs - derive (line 8) ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
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
                edition = "2015"
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
                edition = "2015"
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
        .with_stdout_data(str![[r#"

running 1 test
test tests::it_works ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s


running 1 test
test src/lib.rs - (line 2) ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
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
                edition = "2015"
                [lib]
                crate-type = ["proc-macro"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    foo.cargo("check")
        .with_stderr_data(str![[r#"
[WARNING] library `foo` should only specify `proc-macro = true` instead of setting `crate-type`
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn lib_plugin_unused_key_warning() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
                [lib]
                plugin = true
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    foo.cargo("check")
        .with_stderr_data(str![[r#"
[WARNING] unused manifest key: lib.plugin
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
                edition = "2015"
                [lib]
                crate-type = ["proc-macro"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    foo.cargo("check")
        .with_stderr_data(str![[r#"
[WARNING] library `foo` should only specify `proc-macro = true` instead of setting `crate-type`
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
                edition = "2015"
                [lib]
                crate-type = ["proc-macro", "rlib"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    foo.cargo("check")
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  cannot mix `proc-macro` crate type with others

"#]])
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
                edition = "2015"

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
                edition = "2015"

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
                edition = "2015"

                [lib]
                proc-macro = true

                [features]
                a = []
            "#,
        )
        .file("the-macro/src/lib.rs", "")
        .build();
    p.cargo("build --verbose")
        .with_stderr_data(
            str![[r#"
[COMPILING] the-macro v0.1.0 ([ROOT]/foo/the-macro)
[RUNNING] `rustc --crate-name the_macro [..]`
[COMPILING] b v0.1.0 ([ROOT]/foo/b)
[RUNNING] `rustc --crate-name b [..]`
[COMPILING] a v0.1.0 ([ROOT]/foo/a)
[RUNNING] `rustc --crate-name build_script_build [..]`
[RUNNING] `[ROOT]/foo/target/debug/build/a-[HASH]/build-script-build`
[RUNNING] `rustc --crate-name a [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}
