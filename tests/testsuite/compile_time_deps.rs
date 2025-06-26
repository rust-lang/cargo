use crate::prelude::*;
use cargo_test_support::{project, str};

#[cargo_test]
fn gated_by_unstable_opts() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("Hello, World!") }"#)
        .build();

    p.cargo("check --compile-time-deps")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] the `--compile-time-deps` flag is unstable, and only available on the nightly channel of Cargo, but this is the `stable` channel
See https://doc.rust-lang.org/book/appendix-07-nightly-rust.html for more information about Rust release channels.
See https://github.com/rust-lang/cargo/issues/14434 for more information about the `--compile-time-deps` flag.

"#]])
        .run();
}

#[cargo_test]
fn non_comp_time_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2021"

                [dependencies]
                bar.path = "bar"
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                fn main() {
                    bar::bar();
                }
            "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                edition = "2021"
            "#,
        )
        .file("bar/src/lib.rs", r#"pub fn bar() {}"#)
        .build();

    p.cargo("-Zunstable-options check --compile-time-deps")
        .masquerade_as_nightly_cargo(&["compile-time-deps"])
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn proc_macro_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                resolver = "2"
                members = ["foo", "bar", "baz"]

                [workspace.dependencies]
                bar.path = "bar"
                baz.path = "baz"
            "#,
        )
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2021"

                [dependencies]
                bar.workspace = true
            "#,
        )
        .file(
            "foo/src/main.rs",
            r#"
                fn main() {
                    bar::bar!();
                }
            "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                edition = "2021"

                [lib]
                proc-macro = true

                [dependencies]
                baz.workspace = true
            "#,
        )
        .file(
            "bar/src/lib.rs",
            r#"
                extern crate proc_macro;

                use proc_macro::TokenStream;

                #[proc_macro]
                pub fn bar(input: TokenStream) -> TokenStream {
                    baz::baz();
                    input
                }
            "#,
        )
        .file(
            "bar/tests/simple.rs",
            r#"
                #[test]
                fn test_bar() {
                    let _x: bool = bar::bar!(true);
                }
            "#,
        )
        .file(
            "baz/Cargo.toml",
            r#"
                [package]
                name = "baz"
                version = "0.0.1"
                edition = "2021"
            "#,
        )
        .file("baz/src/lib.rs", r#"pub fn baz() {}"#)
        .build();

    p.cargo("-Zunstable-options check --package foo --compile-time-deps")
        .masquerade_as_nightly_cargo(&["compile-time-deps"])
        .with_stderr_data(str![[r#"
[COMPILING] baz v0.0.1 ([ROOT]/foo/baz)
[COMPILING] bar v0.0.1 ([ROOT]/foo/bar)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("clean").run();

    p.cargo("-Zunstable-options check --package bar --compile-time-deps")
        .masquerade_as_nightly_cargo(&["compile-time-deps"])
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("clean").run();

    p.cargo("-Zunstable-options check --package bar --all-targets --compile-time-deps")
        .masquerade_as_nightly_cargo(&["compile-time-deps"])
        .with_stderr_data(str![[r#"
[COMPILING] baz v0.0.1 ([ROOT]/foo/baz)
[COMPILING] bar v0.0.1 ([ROOT]/foo/bar)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn build_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2021"

                [build-dependencies]
                bar.path = "bar"
            "#,
        )
        .file("src/main.rs", r#"fn main() {}"#)
        .file(
            "build.rs",
            r#"
                fn main() {
                    bar::bar();
                    std::fs::write("check-script-output", "build script run").unwrap();
                }
            "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                edition = "2021"

                [dependencies]
                baz.path = "baz"
            "#,
        )
        .file(
            "bar/src/lib.rs",
            r#"
                pub fn bar() {
                    baz::baz();
                }
            "#,
        )
        .file(
            "bar/baz/Cargo.toml",
            r#"
                [package]
                name = "baz"
                version = "0.0.1"
                edition = "2021"
            "#,
        )
        .file("bar/baz/src/lib.rs", r#"pub fn baz() {}"#)
        .build();

    p.cargo("-Zunstable-options check --compile-time-deps")
        .masquerade_as_nightly_cargo(&["compile-time-deps"])
        .with_stderr_data(str![[r#"
[LOCKING] 2 packages to latest compatible versions
[COMPILING] baz v0.0.1 ([ROOT]/foo/bar/baz)
[COMPILING] bar v0.0.1 ([ROOT]/foo/bar)
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    assert_eq!(p.read_file("check-script-output"), "build script run");
}

#[cargo_test]
fn indirect_comp_time_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2021"

                [dependencies]
                bar.path = "bar"
            "#,
        )
        .file("src/main.rs", r#"fn main() {}"#)
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                edition = "2021"

                [build-dependencies]
                baz.path = "baz"
            "#,
        )
        .file("bar/src/lib.rs", r#"pub fn bar() {}"#)
        .file(
            "bar/build.rs",
            r#"
                fn main() {
                    baz::baz();
                }
            "#,
        )
        .file(
            "bar/baz/Cargo.toml",
            r#"
                [package]
                name = "baz"
                version = "0.0.1"
                edition = "2021"
            "#,
        )
        .file("bar/src/lib.rs", r#"pub fn baz() {}"#)
        .file(
            "bar/baz/Cargo.toml",
            r#"
                [package]
                name = "baz"
                version = "0.0.1"
                edition = "2021"
            "#,
        )
        .file("bar/baz/src/lib.rs", r#"pub fn baz() {}"#)
        .build();

    p.cargo("-Zunstable-options check --compile-time-deps")
        .masquerade_as_nightly_cargo(&["compile-time-deps"])
        .with_stderr_data(str![[r#"
[LOCKING] 2 packages to latest compatible versions
[COMPILING] baz v0.0.1 ([ROOT]/foo/bar/baz)
[COMPILING] bar v0.0.1 ([ROOT]/foo/bar)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn tests_target() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2021"

                [dev-dependencies]
                bar.path = "bar"
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                fn main() {}

                #[test]
                fn foo() {
                    bar::bar!();
                }
            "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                edition = "2021"

                [lib]
                proc-macro = true
            "#,
        )
        .file(
            "bar/src/lib.rs",
            r#"
                extern crate proc_macro;

                use proc_macro::TokenStream;

                #[proc_macro]
                pub fn bar(input: TokenStream) -> TokenStream {
                    input
                }
            "#,
        )
        .build();

    p.cargo("-Zunstable-options check --tests --compile-time-deps")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] bar v0.0.1 ([ROOT]/foo/bar)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .masquerade_as_nightly_cargo(&["compile-time-deps"])
        .run();

    p.cargo("clean").run();

    p.cargo("-Zunstable-options check --compile-time-deps")
        .masquerade_as_nightly_cargo(&["compile-time-deps"])
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
