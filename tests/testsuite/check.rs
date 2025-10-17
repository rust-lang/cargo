//! Tests for the `cargo check` command.

use std::fmt::{self, Write};

use crate::prelude::*;
use crate::utils::tools;
use cargo_test_support::compare::assert_e2e;
use cargo_test_support::install::exe;
use cargo_test_support::registry::Package;
use cargo_test_support::str;
use cargo_test_support::{basic_bin_manifest, basic_manifest, git, project};

#[cargo_test]
fn check_success() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies.bar]
                path = "../bar"
            "#,
        )
        .file(
            "src/main.rs",
            "extern crate bar; fn main() { ::bar::baz(); }",
        )
        .build();
    let _bar = project()
        .at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("src/lib.rs", "pub fn baz() {}")
        .build();

    foo.cargo("check").run();
}

#[cargo_test]
fn check_fail() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies.bar]
                path = "../bar"
            "#,
        )
        .file(
            "src/main.rs",
            "extern crate bar; fn main() { ::bar::baz(42); }",
        )
        .build();
    let _bar = project()
        .at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("src/lib.rs", "pub fn baz() {}")
        .build();

    foo.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
...
error[E0061]: this function takes 0 arguments but 1 argument was supplied
...
"#]])
        .run();
}

#[cargo_test]
fn custom_derive() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies.bar]
                path = "../bar"
            "#,
        )
        .file(
            "src/main.rs",
            r#"
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
            "#,
        )
        .build();
    let _bar = project()
        .at("bar")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
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
            extern crate proc_macro;

            use proc_macro::TokenStream;

            #[proc_macro_derive(B)]
            pub fn derive(_input: TokenStream) -> TokenStream {
                format!("impl B for A {{ fn b(&self) {{}} }}").parse().unwrap()
            }
            "#,
        )
        .build();

    foo.cargo("check").run();
}

#[cargo_test]
fn check_build() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies.bar]
                path = "../bar"
            "#,
        )
        .file(
            "src/main.rs",
            "extern crate bar; fn main() { ::bar::baz(); }",
        )
        .build();

    let _bar = project()
        .at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("src/lib.rs", "pub fn baz() {}")
        .build();

    foo.cargo("check").run();
    foo.cargo("build").run();
}

#[cargo_test]
fn build_check() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies.bar]
                path = "../bar"
            "#,
        )
        .file(
            "src/main.rs",
            "extern crate bar; fn main() { ::bar::baz(); }",
        )
        .build();

    let _bar = project()
        .at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("src/lib.rs", "pub fn baz() {}")
        .build();

    foo.cargo("build -v").run();
    foo.cargo("check -v").run();
}

// Checks that where a project has both a lib and a bin, the lib is only checked
// not built.
#[cargo_test]
fn issue_3418() {
    let foo = project()
        .file("src/lib.rs", "")
        .file("src/main.rs", "fn main() {}")
        .build();

    foo.cargo("check -v")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..] src/lib.rs [..]--emit=[..]metadata [..]`
[RUNNING] `rustc --crate-name foo [..] src/main.rs [..]--emit=[..]metadata [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

// Check on a dylib should have a different metadata hash than build.
#[cargo_test]
fn dylib_check_preserves_build_cache() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [lib]
                crate-type = ["dylib"]

                [dependencies]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check").run();

    p.cargo("build")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

// test `cargo rustc --profile check`
#[cargo_test]
fn rustc_check() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies.bar]
                path = "../bar"
            "#,
        )
        .file(
            "src/main.rs",
            "extern crate bar; fn main() { ::bar::baz(); }",
        )
        .build();
    let _bar = project()
        .at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("src/lib.rs", "pub fn baz() {}")
        .build();

    foo.cargo("rustc --profile check -- --emit=metadata").run();

    // Verify compatible usage of --profile with --release, issue #7488
    foo.cargo("rustc --profile check --release -- --emit=metadata")
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] the argument '--profile <PROFILE-NAME>' cannot be used with '--release'

Usage: cargo[EXE] rustc --profile <PROFILE-NAME> [ARGS]...

For more information, try '--help'.

"#]])
        .run();

    foo.cargo("rustc --profile test --release -- --emit=metadata")
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] the argument '--profile <PROFILE-NAME>' cannot be used with '--release'

Usage: cargo[EXE] rustc --profile <PROFILE-NAME> [ARGS]...

For more information, try '--help'.

"#]])
        .run();
}

#[cargo_test]
fn rustc_check_err() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies.bar]
                path = "../bar"
            "#,
        )
        .file(
            "src/main.rs",
            "extern crate bar; fn main() { ::bar::qux(); }",
        )
        .build();
    let _bar = project()
        .at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("src/lib.rs", "pub fn baz() {}")
        .build();

    foo.cargo("rustc --profile check -- --emit=metadata")
        .with_status(101)
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[CHECKING] bar v0.1.0 ([ROOT]/bar)
[CHECKING] foo v0.0.1 ([ROOT]/foo)
error[E0425]: [..]
...
"#]])
        .run();
}

#[cargo_test]
fn check_all() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [workspace]
                [dependencies]
                b = { path = "b" }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("examples/a.rs", "fn main() {}")
        .file("tests/a.rs", "")
        .file("src/lib.rs", "")
        .file("b/Cargo.toml", &basic_manifest("b", "0.0.1"))
        .file("b/src/main.rs", "fn main() {}")
        .file("b/src/lib.rs", "")
        .build();

    p.cargo("check --workspace -v")
        .with_stderr_data(
            str![[r#"
[CHECKING] b v0.0.1 ([ROOT]/foo/b)
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..] src/lib.rs [..]`
[RUNNING] `rustc --crate-name foo [..] src/main.rs [..]`
[RUNNING] `rustc --crate-name b [..] b/src/lib.rs [..]`
[RUNNING] `rustc --crate-name b [..] b/src/main.rs [..]`
...
"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn check_all_exclude() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar", "baz"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "pub fn baz() { break_the_build(); }")
        .build();

    p.cargo("check --workspace --exclude baz")
        .with_stderr_does_not_contain("[CHECKING] baz v0.1.0 [..]")
        .with_stderr_data(str![[r#"
[CHECKING] bar v0.1.0 ([ROOT]/foo/bar)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn check_all_exclude_glob() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar", "baz"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "pub fn baz() { break_the_build(); }")
        .build();

    p.cargo("check --workspace --exclude '*z'")
        .with_stderr_does_not_contain("[CHECKING] baz v0.1.0 [..]")
        .with_stderr_data(str![[r#"
[CHECKING] bar v0.1.0 ([ROOT]/foo/bar)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn check_virtual_all_implied() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar", "baz"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "pub fn baz() {}")
        .build();

    p.cargo("check -v")
        .with_stderr_data(
            str![[r#"
[CHECKING] baz v0.1.0 ([ROOT]/foo/baz)
[CHECKING] bar v0.1.0 ([ROOT]/foo/bar)
[RUNNING] `rustc --crate-name baz [..] baz/src/lib.rs [..]`
[RUNNING] `rustc --crate-name bar [..] bar/src/lib.rs [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn check_virtual_manifest_one_project() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar", "baz"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "pub fn baz() { break_the_build(); }")
        .build();

    p.cargo("check -p bar")
        .with_stderr_does_not_contain("[CHECKING] baz v0.1.0 [..]")
        .with_stderr_data(str![[r#"
[CHECKING] bar v0.1.0 ([ROOT]/foo/bar)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn check_virtual_manifest_one_bin_project_not_in_default_members() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar"]
                default-members = []
                resolver = "3"
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/main.rs", "fn main() { let _ = (1); }")
        .build();

    p.cargo("check -p bar")
        .with_stderr_contains("[..]run `cargo fix --bin \"bar\" -p bar` to apply[..]")
        .run();
}

#[cargo_test]
fn check_virtual_manifest_glob() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar", "baz"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {  break_the_build(); }")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "pub fn baz() {}")
        .build();

    p.cargo("check -p '*z'")
        .with_stderr_does_not_contain("[CHECKING] bar v0.1.0 [..]")
        .with_stderr_data(str![[r#"
[CHECKING] baz v0.1.0 ([ROOT]/foo/baz)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn exclude_warns_on_non_existing_package() {
    let p = project().file("src/lib.rs", "").build();
    p.cargo("check --workspace --exclude bar")
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[WARNING] excluded package(s) `bar` not found in workspace `[ROOT]/foo`
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn targets_selected_default() {
    let foo = project()
        .file("src/main.rs", "fn main() {}")
        .file("src/lib.rs", "pub fn smth() {}")
        .file("examples/example1.rs", "fn main() {}")
        .file("tests/test2.rs", "#[test] fn t() {}")
        .file("benches/bench3.rs", "")
        .build();

    foo.cargo("check -v")
        .with_stderr_contains("[..] --crate-name foo [..] src/lib.rs [..]")
        .with_stderr_contains("[..] --crate-name foo [..] src/main.rs [..]")
        .with_stderr_does_not_contain("[..] --crate-name example1 [..] examples/example1.rs [..]")
        .with_stderr_does_not_contain("[..] --crate-name test2 [..] tests/test2.rs [..]")
        .with_stderr_does_not_contain("[..] --crate-name bench3 [..] benches/bench3.rs [..]")
        .run();
}

#[cargo_test]
fn targets_selected_all() {
    let foo = project()
        .file("src/main.rs", "fn main() {}")
        .file("src/lib.rs", "pub fn smth() {}")
        .file("examples/example1.rs", "fn main() {}")
        .file("tests/test2.rs", "#[test] fn t() {}")
        .file("benches/bench3.rs", "")
        .build();

    foo.cargo("check --all-targets -v")
        .with_stderr_data(
            str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..] src/lib.rs [..]`
[RUNNING] `rustc --crate-name foo [..] src/main.rs [..]`
[RUNNING] `rustc --crate-name example1 [..] examples/example1.rs [..]`
[RUNNING] `rustc --crate-name test2 [..] tests/test2.rs [..]`
[RUNNING] `rustc --crate-name bench3 [..] benches/bench3.rs [..]`
...
"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn check_unit_test_profile() {
    let foo = project()
        .file(
            "src/lib.rs",
            r#"
                #[cfg(test)]
                mod tests {
                    #[test]
                    fn it_works() {
                        badtext
                    }
                }
            "#,
        )
        .build();

    foo.cargo("check").run();
    foo.cargo("check --profile test")
        .with_status(101)
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
error[E0425]: cannot find value `badtext` in this scope
...
"#]])
        .run();
}

// Verify what is checked with various command-line filters.
#[cargo_test]
fn check_filters() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
                fn unused_normal_lib() {}
                #[cfg(test)]
                mod tests {
                    fn unused_unit_lib() {}
                }
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                fn main() {}
                fn unused_normal_bin() {}
                #[cfg(test)]
                mod tests {
                    fn unused_unit_bin() {}
                }
            "#,
        )
        .file(
            "tests/t1.rs",
            r#"
                fn unused_normal_t1() {}
                #[cfg(test)]
                mod tests {
                    fn unused_unit_t1() {}
                }
            "#,
        )
        .file(
            "examples/ex1.rs",
            r#"
                fn main() {}
                fn unused_normal_ex1() {}
                #[cfg(test)]
                mod tests {
                    fn unused_unit_ex1() {}
                }
            "#,
        )
        .file(
            "benches/b1.rs",
            r#"
                fn unused_normal_b1() {}
                #[cfg(test)]
                mod tests {
                    fn unused_unit_b1() {}
                }
            "#,
        )
        .build();

    p.cargo("check")
        .with_stderr_contains("[..]unused_normal_lib[..]")
        .with_stderr_contains("[..]unused_normal_bin[..]")
        .with_stderr_does_not_contain("[..]unused_normal_t1[..]")
        .with_stderr_does_not_contain("[..]unused_normal_ex1[..]")
        .with_stderr_does_not_contain("[..]unused_normal_b1[..]")
        .with_stderr_does_not_contain("[..]unused_unit_[..]")
        .run();
    p.root().join("target").rm_rf();
    p.cargo("check --tests -v")
        .with_stderr_contains("[..] --crate-name foo [..] src/lib.rs [..] --test [..]")
        .with_stderr_contains("[..] --crate-name foo [..] src/lib.rs [..] --crate-type lib [..]")
        .with_stderr_contains("[..] --crate-name foo [..] src/main.rs [..] --test [..]")
        .with_stderr_contains("[..]unused_unit_lib[..]")
        .with_stderr_contains("[..]unused_unit_bin[..]")
        .with_stderr_contains("[..]unused_normal_lib[..]")
        .with_stderr_contains("[..]unused_normal_bin[..]")
        .with_stderr_contains("[..]unused_unit_t1[..]")
        .with_stderr_does_not_contain("[..]unused_normal_ex1[..]")
        .with_stderr_does_not_contain("[..]unused_unit_ex1[..]")
        .with_stderr_does_not_contain("[..]unused_normal_b1[..]")
        .with_stderr_does_not_contain("[..]unused_unit_b1[..]")
        .with_stderr_does_not_contain("[..]--crate-type bin[..]")
        .run();
    p.root().join("target").rm_rf();
    p.cargo("check --test t1 -v")
        .with_stderr_contains("[..]unused_normal_lib[..]")
        .with_stderr_contains("[..]unused_unit_t1[..]")
        .with_stderr_does_not_contain("[..]unused_unit_lib[..]")
        .with_stderr_does_not_contain("[..]unused_normal_bin[..]")
        .with_stderr_does_not_contain("[..]unused_unit_bin[..]")
        .with_stderr_does_not_contain("[..]unused_normal_ex1[..]")
        .with_stderr_does_not_contain("[..]unused_normal_b1[..]")
        .with_stderr_does_not_contain("[..]unused_unit_ex1[..]")
        .with_stderr_does_not_contain("[..]unused_unit_b1[..]")
        .run();
    p.root().join("target").rm_rf();
    p.cargo("check --all-targets -v")
        .with_stderr_contains("[..]unused_normal_lib[..]")
        .with_stderr_contains("[..]unused_normal_bin[..]")
        .with_stderr_contains("[..]unused_normal_t1[..]")
        .with_stderr_contains("[..]unused_normal_ex1[..]")
        .with_stderr_contains("[..]unused_normal_b1[..]")
        .with_stderr_contains("[..]unused_unit_b1[..]")
        .with_stderr_contains("[..]unused_unit_t1[..]")
        .with_stderr_contains("[..]unused_unit_lib[..]")
        .with_stderr_contains("[..]unused_unit_bin[..]")
        .with_stderr_does_not_contain("[..]unused_unit_ex1[..]")
        .run();
}

#[cargo_test]
fn check_artifacts() {
    // Verify which artifacts are created when running check (#4059).
    let p = project()
        .file("src/lib.rs", "")
        .file("src/main.rs", "fn main() {}")
        .file("tests/t1.rs", "")
        .file("examples/ex1.rs", "fn main() {}")
        .file("benches/b1.rs", "")
        .build();

    p.cargo("check").run();
    assert!(!p.root().join("target/debug/libfoo.rmeta").is_file());
    assert!(!p.root().join("target/debug/libfoo.rlib").is_file());
    assert!(!p.root().join("target/debug").join(exe("foo")).is_file());
    assert_eq!(p.glob("target/debug/deps/libfoo-*.rmeta").count(), 2);

    p.root().join("target").rm_rf();
    p.cargo("check --lib").run();
    assert!(!p.root().join("target/debug/libfoo.rmeta").is_file());
    assert!(!p.root().join("target/debug/libfoo.rlib").is_file());
    assert!(!p.root().join("target/debug").join(exe("foo")).is_file());
    assert_eq!(p.glob("target/debug/deps/libfoo-*.rmeta").count(), 1);

    p.root().join("target").rm_rf();
    p.cargo("check --bin foo").run();
    assert!(!p.root().join("target/debug/libfoo.rmeta").is_file());
    assert!(!p.root().join("target/debug/libfoo.rlib").is_file());
    assert!(!p.root().join("target/debug").join(exe("foo")).is_file());
    assert_eq!(p.glob("target/debug/deps/libfoo-*.rmeta").count(), 2);

    p.root().join("target").rm_rf();
    p.cargo("check --test t1").run();
    assert!(!p.root().join("target/debug/libfoo.rmeta").is_file());
    assert!(!p.root().join("target/debug/libfoo.rlib").is_file());
    assert!(!p.root().join("target/debug").join(exe("foo")).is_file());
    assert_eq!(p.glob("target/debug/t1-*").count(), 0);
    assert_eq!(p.glob("target/debug/deps/libfoo-*.rmeta").count(), 1);
    assert_eq!(p.glob("target/debug/deps/libt1-*.rmeta").count(), 1);

    p.root().join("target").rm_rf();
    p.cargo("check --example ex1").run();
    assert!(!p.root().join("target/debug/libfoo.rmeta").is_file());
    assert!(!p.root().join("target/debug/libfoo.rlib").is_file());
    assert!(
        !p.root()
            .join("target/debug/examples")
            .join(exe("ex1"))
            .is_file()
    );
    assert_eq!(p.glob("target/debug/deps/libfoo-*.rmeta").count(), 1);
    assert_eq!(p.glob("target/debug/examples/libex1-*.rmeta").count(), 1);

    p.root().join("target").rm_rf();
    p.cargo("check --bench b1").run();
    assert!(!p.root().join("target/debug/libfoo.rmeta").is_file());
    assert!(!p.root().join("target/debug/libfoo.rlib").is_file());
    assert!(!p.root().join("target/debug").join(exe("foo")).is_file());
    assert_eq!(p.glob("target/debug/b1-*").count(), 0);
    assert_eq!(p.glob("target/debug/deps/libfoo-*.rmeta").count(), 1);
    assert_eq!(p.glob("target/debug/deps/libb1-*.rmeta").count(), 1);
}

#[cargo_test]
fn short_message_format() {
    let foo = project()
        .file("src/lib.rs", "fn foo() { let _x: bool = 'a'; }")
        .build();
    foo.cargo("check --message-format=short")
        .with_status(101)
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
src/lib.rs:1:27: error[E0308]: mismatched types[..]
[ERROR] could not compile `foo` (lib) due to 1 previous error

"#]])
        .run();
}

#[cargo_test]
fn proc_macro() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "demo"
                version = "0.0.1"
                edition = "2015"

                [lib]
                proc-macro = true
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                extern crate proc_macro;

                use proc_macro::TokenStream;

                #[proc_macro_derive(Foo)]
                pub fn demo(_input: TokenStream) -> TokenStream {
                    "".parse().unwrap()
                }
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                #[macro_use]
                extern crate demo;

                #[derive(Foo)]
                struct A;

                fn main() {}
            "#,
        )
        .build();
    p.cargo("check -v").env("CARGO_LOG", "cargo=trace").run();
}

#[cargo_test]
fn check_keep_going() {
    let foo = project()
        .file("src/bin/one.rs", "compile_error!(\"ONE\"); fn main() {}")
        .file("src/bin/two.rs", "compile_error!(\"TWO\"); fn main() {}")
        .build();

    // Due to -j1, without --keep-going only one of the two bins would be built.
    foo.cargo("check -j1 --keep-going")
        .with_status(101)
        .with_stderr_data(
            str![[r#"
[ERROR] ONE
[ERROR] TWO
...
"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn does_not_use_empty_rustc_wrapper() {
    // An empty RUSTC_WRAPPER environment variable won't be used.
    // The env var will also override the config, essentially unsetting it.
    let p = project()
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
                [build]
                rustc-wrapper = "do-not-execute-me"
            "#,
        )
        .build();
    p.cargo("check").env("RUSTC_WRAPPER", "").run();
}

#[cargo_test]
fn does_not_use_empty_rustc_workspace_wrapper() {
    let p = project().file("src/lib.rs", "").build();
    p.cargo("check").env("RUSTC_WORKSPACE_WRAPPER", "").run();
}

#[cargo_test]
fn error_from_deep_recursion() -> Result<(), fmt::Error> {
    let mut big_macro = String::new();
    writeln!(big_macro, "macro_rules! m {{")?;
    for i in 0..130 {
        writeln!(big_macro, "({}) => {{ m!({}); }};", i, i + 1)?;
    }
    writeln!(big_macro, "}}")?;
    writeln!(big_macro, "m!(0);")?;

    let p = project().file("src/lib.rs", &big_macro).build();
    p.cargo("check --message-format=json")
        .with_status(101)
        .with_stdout_data(str![[r#"
{"reason":"compiler-message",[..]"message":"recursion limit reached while expanding `m!`",[..]rendered":"[..]recursion limit reached while expanding `m!`[..]"}}
...
"#]])
        .run();

    Ok(())
}

#[cargo_test]
fn rustc_workspace_wrapper_affects_all_workspace_members() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar", "baz"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "pub fn baz() {}")
        .build();

    p.cargo("check")
        .env("RUSTC_WORKSPACE_WRAPPER", tools::echo_wrapper())
        .with_stderr_data(
            str![[r#"
WRAPPER CALLED: rustc --crate-name bar [..]
WRAPPER CALLED: rustc --crate-name baz [..]
...
"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn rustc_workspace_wrapper_includes_path_deps() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [workspace]
                members = ["bar"]

                [dependencies]
                baz = { path = "baz" }
            "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "pub fn baz() {}")
        .build();

    p.cargo("check --workspace")
        .env("RUSTC_WORKSPACE_WRAPPER", tools::echo_wrapper())
        .with_stderr_data(
            str![[r#"
WRAPPER CALLED: rustc --crate-name bar [..]
WRAPPER CALLED: rustc --crate-name baz [..]
WRAPPER CALLED: rustc --crate-name foo [..]
...
"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn rustc_workspace_wrapper_respects_primary_units() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar", "baz"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "pub fn baz() {}")
        .build();

    p.cargo("check -p bar")
        .env("RUSTC_WORKSPACE_WRAPPER", tools::echo_wrapper())
        .with_stderr_contains("WRAPPER CALLED: rustc --crate-name bar [..]")
        .with_stdout_does_not_contain("WRAPPER CALLED: rustc --crate-name baz [..]")
        .run();
}

#[cargo_test]
fn rustc_workspace_wrapper_excludes_published_deps() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [workspace]
                members = ["bar"]

                [dependencies]
                baz = "1.0.0"
            "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();

    Package::new("baz", "1.0.0").publish();

    p.cargo("check --workspace -v")
        .env("RUSTC_WORKSPACE_WRAPPER", tools::echo_wrapper())
        .with_stderr_contains("WRAPPER CALLED: rustc --crate-name foo [..]")
        .with_stderr_contains("WRAPPER CALLED: rustc --crate-name bar [..]")
        .with_stderr_contains("[CHECKING] baz [..]")
        .with_stdout_does_not_contain("WRAPPER CALLED: rustc --crate-name baz [..]")
        .run();
}

#[cargo_test]
fn warn_manifest_with_project() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[WARNING] `[project]` is deprecated in favor of `[package]`
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn error_manifest_with_project_on_2024() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                edition = "2024"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  `[project]` is not supported as of the 2024 Edition, please use `[package]`

"#]])
        .run();
}

#[cargo_test]
fn warn_manifest_package_and_project() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [project]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[WARNING] `[project]` is deprecated in favor of `[package]`
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn git_manifest_package_and_project() {
    let p = project();
    let git_project = git::new("bar", |p| {
        p.file(
            "Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.0.1"
            edition = "2015"

            [project]
            name = "bar"
            version = "0.0.1"
            edition = "2015"
            "#,
        )
        .file("src/lib.rs", "")
    });

    let p = p
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [dependencies.bar]
                version = "0.0.1"
                git  = '{}'

            "#,
                git_project.url()
            ),
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/bar`
[LOCKING] 1 package to latest compatible version
[CHECKING] bar v0.0.1 ([ROOTURL]/bar#[..])
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn git_manifest_with_project() {
    let p = project();
    let git_project = git::new("bar", |p| {
        p.file(
            "Cargo.toml",
            r#"
            [project]
            name = "bar"
            version = "0.0.1"
            edition = "2015"
            "#,
        )
        .file("src/lib.rs", "")
    });

    let p = p
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [dependencies.bar]
                version = "0.0.1"
                git  = '{}'

            "#,
                git_project.url()
            ),
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/bar`
[LOCKING] 1 package to latest compatible version
[CHECKING] bar v0.0.1 ([ROOTURL]/bar#[..])
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn check_fixable_warning() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
            "#,
        )
        .file("src/lib.rs", "use std::io;")
        .build();

    foo.cargo("check")
        .with_stderr_data(str![[r#"
...
[WARNING] `foo` (lib) generated 1 warning (run `cargo fix --lib -p foo` to apply 1 suggestion)
...
"#]])
        .run();
}

#[cargo_test]
fn check_fixable_test_warning() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
            "#,
        )
        .file(
            "src/lib.rs",
            "\
mod tests {
    #[test]
    fn t1() {
        use std::io;
    }
}
            ",
        )
        .build();

    foo.cargo("check --all-targets")
        .with_stderr_data(str![[r#"
...
[WARNING] `foo` (lib test) generated 1 warning (run `cargo fix --lib -p foo --tests` to apply 1 suggestion)
...
"#]])
        .run();
    foo.cargo("fix --lib -p foo --tests --allow-no-vcs").run();
    assert!(!foo.read_file("src/lib.rs").contains("use std::io;"));
}

#[cargo_test]
fn check_fixable_error_no_fix() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
            "#,
        )
        .file(
            "src/lib.rs",
            "use std::io;\n#[derive(Debug(x))]\nstruct Foo;",
        )
        .build();

    foo.cargo("check")
        .with_status(101)
        .with_stderr_data(
            str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[ERROR] traits in `#[derive(...)]` don't accept arguments
[WARNING] unused import: `std::io`
[WARNING] `foo` (lib) generated 1 warning
[ERROR] could not compile `foo` (lib) due to 1 previous error; 1 warning emitted
...
"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn check_fixable_warning_workspace() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["foo", "bar"]
            "#,
        )
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
            "#,
        )
        .file("foo/src/lib.rs", "use std::io;")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                edition = "2015"

                [dependencies]
                foo = { path = "../foo" }
            "#,
        )
        .file("bar/src/lib.rs", "use std::io;")
        .build();

    p.cargo("check")
        .with_stderr_data(
            str![[r#"
[WARNING] `foo` (lib) generated 1 warning (run `cargo fix --lib -p foo` to apply 1 suggestion)
[WARNING] `bar` (lib) generated 1 warning (run `cargo fix --lib -p bar` to apply 1 suggestion)
...
"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn check_fixable_example() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file(
            "src/main.rs",
            r#"
            fn hello() -> &'static str {
                "hello"
            }

            pub fn main() {
                println!("{}", hello())
            }
            "#,
        )
        .file("examples/ex1.rs", "use std::fmt; fn main() {}")
        .build();
    p.cargo("check --all-targets")
        .with_stderr_data(str![[r#"
...
[WARNING] `foo` (example "ex1") generated 1 warning (run `cargo fix --example "ex1" -p foo` to apply 1 suggestion)
...
"#]])
        .run();
}

#[cargo_test(nightly, reason = "bench")]
fn check_fixable_bench() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file(
            "src/main.rs",
            r#"
            #![feature(test)]
            #[cfg(test)]
            extern crate test;

            fn hello() -> &'static str {
                "hello"
            }

            pub fn main() {
                println!("{}", hello())
            }

            #[bench]
            fn bench_hello(_b: &mut test::Bencher) {
                use std::io;
                assert_eq!(hello(), "hello")
            }
            "#,
        )
        .file(
            "benches/bench.rs",
            "
            #![feature(test)]
            extern crate test;

            #[bench]
            fn bench(_b: &mut test::Bencher) { use std::fmt; }
        ",
        )
        .build();
    p.cargo("check --all-targets")
        .with_stderr_data(str![[r#"
...
[WARNING] `foo` (bench "bench") generated 1 warning (run `cargo fix --bench "bench" -p foo` to apply 1 suggestion)
...
"#]])
        .run();
}

#[cargo_test(nightly, reason = "bench")]
fn check_fixable_mixed() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file(
            "src/main.rs",
            r#"
            #![feature(test)]
            #[cfg(test)]
            extern crate test;

            fn hello() -> &'static str {
                "hello"
            }

            pub fn main() {
                println!("{}", hello())
            }

            #[bench]
            fn bench_hello(_b: &mut test::Bencher) {
                use std::io;
                assert_eq!(hello(), "hello")
            }
            #[test]
            fn t1() {
                use std::fmt;
            }
            "#,
        )
        .file("examples/ex1.rs", "use std::fmt; fn main() {}")
        .file(
            "benches/bench.rs",
            "
            #![feature(test)]
            extern crate test;

            #[bench]
            fn bench(_b: &mut test::Bencher) { use std::fmt; }
        ",
        )
        .build();
    p.cargo("check --all-targets")
        .with_stderr_data(str![[r#"
[WARNING] `foo` (example "ex1") generated 1 warning (run `cargo fix --example "ex1" -p foo` to apply 1 suggestion)
[WARNING] `foo` (bench "bench") generated 1 warning (run `cargo fix --bench "bench" -p foo` to apply 1 suggestion)
[WARNING] `foo` (bin "foo" test) generated 2 warnings (run `cargo fix --bin "foo" -p foo --tests` to apply 2 suggestions)
...
"#]].unordered())
        .run();
}

#[cargo_test]
fn check_fixable_warning_for_clippy() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
            "#,
        )
        // We don't want to show a warning that is `clippy`
        // specific since we are using a `rustc` wrapper
        // inplace of `clippy`
        .file("src/lib.rs", "use std::io;")
        .build();

    foo.cargo("check")
        // We can't use `clippy` so we use a `rustc` workspace wrapper instead
        .env("RUSTC_WORKSPACE_WRAPPER", tools::wrapped_clippy_driver())
        .with_stderr_data(str![[r#"
...
[WARNING] `foo` (lib) generated 1 warning (run `cargo clippy --fix --lib -p foo` to apply 1 suggestion)
...
"#]])
        .run();
}

#[cargo_test]
fn check_unused_manifest_keys() {
    Package::new("dep", "0.1.0").publish();
    Package::new("foo", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.2.0"
            edition = "2015"
            authors = []

            [dependencies]
            dep = { version = "0.1.0", wxz = "wxz" }
            foo = { version = "0.1.0", abc = "abc" }

            [dev-dependencies]
            foo = { version = "0.1.0", wxz = "wxz" }

            [build-dependencies]
            foo = { version = "0.1.0", wxz = "wxz" }

            [target.'cfg(windows)'.dependencies]
            foo = { version = "0.1.0", wxz = "wxz" }

            [target.wasm32-wasip1.dev-dependencies]
            foo = { version = "0.1.0", wxz = "wxz" }

            [target.bar.build-dependencies]
            foo = { version = "0.1.0", wxz = "wxz" }
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_stderr_data(
            str![[r#"
[WARNING] unused manifest key: dependencies.dep.wxz
[WARNING] unused manifest key: dependencies.foo.abc
[WARNING] unused manifest key: dev-dependencies.foo.wxz
[WARNING] unused manifest key: build-dependencies.foo.wxz
[WARNING] unused manifest key: target.bar.build-dependencies.foo.wxz
[WARNING] unused manifest key: target.cfg(windows).dependencies.foo.wxz
[WARNING] unused manifest key: target.wasm32-wasip1.dev-dependencies.foo.wxz
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] foo v0.1.0 (registry `dummy-registry`)
[DOWNLOADED] dep v0.1.0 (registry `dummy-registry`)
[CHECKING] foo v0.1.0
[CHECKING] dep v0.1.0
[CHECKING] bar v0.2.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn versionless_package() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                description = "foo"
                edition = "2015"
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("check")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn pkgid_querystring_works() {
    let git_project = git::new("gitdep", |p| {
        p.file("Cargo.toml", &basic_manifest("gitdep", "1.0.0"))
            .file("src/lib.rs", "")
    });
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                edition = "2015"

                [dependencies]
                gitdep = {{ git = "{}", branch = "master" }}
                "#,
                git_project.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("generate-lockfile").run();

    let output = p.cargo("pkgid").arg("gitdep").run();
    let gitdep_pkgid = String::from_utf8(output.stdout).unwrap();
    let gitdep_pkgid = gitdep_pkgid.trim();
    assert_e2e().eq(
        gitdep_pkgid,
        str!["git+[ROOTURL]/gitdep?branch=master#1.0.0"],
    );

    p.cargo("build -p")
        .arg(gitdep_pkgid)
        .with_stderr_data(str![[r#"
[COMPILING] gitdep v1.0.0 ([ROOTURL]/gitdep?branch=master#[..])
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
