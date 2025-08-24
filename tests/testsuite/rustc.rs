//! Tests for the `cargo rustc` command.

use crate::prelude::*;
use cargo_test_support::basic_bin_manifest;
use cargo_test_support::basic_lib_manifest;
use cargo_test_support::basic_manifest;
use cargo_test_support::project;
use cargo_test_support::str;
use cargo_test_support::target_spec_json;

#[cargo_test]
fn build_lib_for_foo() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("src/lib.rs", r#" "#)
        .build();

    p.cargo("rustc --lib -v").with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo --edition=2015 src/lib.rs [..]--crate-type lib --emit=[..]link[..]-C debuginfo=2 [..]-C metadata=[..] [..]--out-dir [ROOT]/foo/target/debug/deps -L dependency=[ROOT]/foo/target/debug/deps`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]).run();
}

#[cargo_test]
fn lib() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("src/lib.rs", r#" "#)
        .build();

    p.cargo("rustc --lib -v -- -C debug-assertions=off")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo --edition=2015 src/lib.rs [..]--crate-type lib --emit=[..]link[..]-C debuginfo=2 [..]-C metadata=[..] [..]--out-dir [ROOT]/foo/target/debug/deps -L dependency=[ROOT]/foo/target/debug/deps[..]-C debug-assertions=off[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn build_main_and_allow_unstable_options() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("src/lib.rs", r#" "#)
        .build();

    p.cargo("rustc -v --bin foo -- -C debug-assertions")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo --edition=2015 src/lib.rs [..]--crate-type lib --emit=[..]link[..]-C debuginfo=2 [..]-C metadata=[..] --out-dir [ROOT]/foo/target/debug/deps -L dependency=[ROOT]/foo/target/debug/deps`
[RUNNING] `rustc --crate-name foo --edition=2015 src/main.rs [..]--crate-type bin --emit=[..]link[..]-C debuginfo=2 [..]-C metadata=[..] --out-dir [ROOT]/foo/target/debug/deps -L dependency=[ROOT]/foo/target/debug/deps --extern foo=[ROOT]/foo/target/debug/deps/libfoo-[HASH].rlib[..]-C debug-assertions[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn fails_when_trying_to_build_main_and_lib_with_args() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("src/lib.rs", r#" "#)
        .build();

    p.cargo("rustc -v -- -C debug-assertions")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] extra arguments to `rustc` can only be passed to one target, consider filtering
the package by passing, e.g., `--lib` or `--bin NAME` to specify a single target

"#]])
        .run();
}

#[cargo_test]
fn build_with_args_to_one_of_multiple_binaries() {
    let p = project()
        .file("src/bin/foo.rs", "fn main() {}")
        .file("src/bin/bar.rs", "fn main() {}")
        .file("src/bin/baz.rs", "fn main() {}")
        .file("src/lib.rs", r#" "#)
        .build();

    p.cargo("rustc -v --bin bar -- -C debug-assertions")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo --edition=2015 src/lib.rs [..]--crate-type lib --emit=[..]link[..]-C debuginfo=2 [..]-C metadata=[..] --out-dir [..]`
[RUNNING] `rustc --crate-name bar --edition=2015 src/bin/bar.rs [..]--crate-type bin --emit=[..]link[..]-C debuginfo=2 [..]-C debug-assertions[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn fails_with_args_to_all_binaries() {
    let p = project()
        .file("src/bin/foo.rs", "fn main() {}")
        .file("src/bin/bar.rs", "fn main() {}")
        .file("src/bin/baz.rs", "fn main() {}")
        .file("src/lib.rs", r#" "#)
        .build();

    p.cargo("rustc -v -- -C debug-assertions")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] extra arguments to `rustc` can only be passed to one target, consider filtering
the package by passing, e.g., `--lib` or `--bin NAME` to specify a single target

"#]])
        .run();
}

#[cargo_test]
fn fails_with_crate_type_to_multi_binaries() {
    let p = project()
        .file("src/bin/foo.rs", "fn main() {}")
        .file("src/bin/bar.rs", "fn main() {}")
        .file("src/bin/baz.rs", "fn main() {}")
        .file("src/lib.rs", r#" "#)
        .build();

    p.cargo("rustc --crate-type lib")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] crate types to rustc can only be passed to one target, consider filtering
the package by passing, e.g., `--lib` or `--example` to specify a single target

"#]])
        .run();
}

#[cargo_test]
fn fails_with_crate_type_to_multi_examples() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2015"
            authors = []

            [[example]]
            name = "ex1"
            crate-type = ["rlib"]
            [[example]]
            name = "ex2"
            crate-type = ["rlib"]
        "#,
        )
        .file("src/lib.rs", "")
        .file("examples/ex1.rs", "")
        .file("examples/ex2.rs", "")
        .build();

    p.cargo("rustc -v --example ex1 --example ex2 --crate-type lib,cdylib")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] crate types to rustc can only be passed to one target, consider filtering
the package by passing, e.g., `--lib` or `--example` to specify a single target

"#]])
        .run();
}

#[cargo_test]
fn fails_with_crate_type_to_binary() {
    let p = project().file("src/bin/foo.rs", "fn main() {}").build();

    p.cargo("rustc --crate-type lib")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] crate types can only be specified for libraries and example libraries.
Binaries, tests, and benchmarks are always the `bin` crate type

"#]])
        .run();
}

#[cargo_test]
fn build_with_crate_type_for_foo() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("rustc -v --crate-type cdylib")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo --edition=2015 src/lib.rs [..]--crate-type cdylib [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn build_with_crate_type_for_foo_with_deps() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
            extern crate a;
            pub fn foo() { a::hello(); }
            "#,
        )
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2015"
            authors = []

            [dependencies]
            a = { path = "a" }
            "#,
        )
        .file("a/Cargo.toml", &basic_manifest("a", "0.1.0"))
        .file("a/src/lib.rs", "pub fn hello() {}")
        .build();

    p.cargo("rustc -v --crate-type cdylib")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] a v0.1.0 ([ROOT]/foo/a)
[RUNNING] `rustc --crate-name a --edition=2015 a/src/lib.rs [..]--crate-type lib [..]`
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo --edition=2015 src/lib.rs [..]--crate-type cdylib [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn build_with_crate_types_for_foo() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("rustc -v --crate-type lib,cdylib")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo --edition=2015 src/lib.rs [..]--crate-type lib --crate-type cdylib [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn build_with_crate_type_to_example() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2015"
            authors = []

            [[example]]
            name = "ex"
            crate-type = ["rlib"]
        "#,
        )
        .file("src/lib.rs", "")
        .file("examples/ex.rs", "")
        .build();

    p.cargo("rustc -v --example ex --crate-type cdylib")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo --edition=2015 src/lib.rs [..]--crate-type lib [..]`
[RUNNING] `rustc --crate-name ex --edition=2015 examples/ex.rs [..]--crate-type cdylib [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn build_with_crate_types_to_example() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2015"
            authors = []

            [[example]]
            name = "ex"
            crate-type = ["rlib"]
        "#,
        )
        .file("src/lib.rs", "")
        .file("examples/ex.rs", "")
        .build();

    p.cargo("rustc -v --example ex --crate-type lib,cdylib")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo --edition=2015 src/lib.rs [..]--crate-type lib [..]`
[RUNNING] `rustc --crate-name ex --edition=2015 examples/ex.rs [..]--crate-type lib --crate-type cdylib [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn build_with_crate_types_to_one_of_multi_examples() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2015"
            authors = []

            [[example]]
            name = "ex1"
            crate-type = ["rlib"]
            [[example]]
            name = "ex2"
            crate-type = ["rlib"]
        "#,
        )
        .file("src/lib.rs", "")
        .file("examples/ex1.rs", "")
        .file("examples/ex2.rs", "")
        .build();

    p.cargo("rustc -v --example ex1 --crate-type lib,cdylib")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo --edition=2015 src/lib.rs [..]--crate-type lib [..]`
[RUNNING] `rustc --crate-name ex1 --edition=2015 examples/ex1.rs [..]--crate-type lib --crate-type cdylib [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn build_with_args_to_one_of_multiple_tests() {
    let p = project()
        .file("tests/foo.rs", r#" "#)
        .file("tests/bar.rs", r#" "#)
        .file("tests/baz.rs", r#" "#)
        .file("src/lib.rs", r#" "#)
        .build();

    p.cargo("rustc -v --test bar -- -C debug-assertions")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo --edition=2015 src/lib.rs [..]--crate-type lib --emit=[..]link[..]-C debuginfo=2 [..]-C metadata=[..] --out-dir [..]`
[RUNNING] `rustc --crate-name bar --edition=2015 tests/bar.rs [..]--emit=[..]link[..]-C debuginfo=2 [..]--test[..]-C debug-assertions[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn build_foo_with_bar_dependency() {
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
        .file("src/main.rs", "extern crate bar; fn main() { bar::baz() }")
        .build();
    let _bar = project()
        .at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("src/lib.rs", "pub fn baz() {}")
        .build();

    foo.cargo("rustc -v -- -C debug-assertions")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] bar v0.1.0 ([ROOT]/bar)
[RUNNING] `rustc --crate-name bar [..] -C debuginfo=2[..]`
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..] -C debuginfo=2 [..]-C debug-assertions[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn build_only_bar_dependency() {
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
        .file("src/main.rs", "extern crate bar; fn main() { bar::baz() }")
        .build();
    let _bar = project()
        .at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("src/lib.rs", "pub fn baz() {}")
        .build();

    foo.cargo("rustc -v -p bar -- -C debug-assertions")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] bar v0.1.0 ([ROOT]/bar)
[RUNNING] `rustc --crate-name bar [..]--crate-type lib [..] -C debug-assertions[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn targets_selected_default() {
    let p = project().file("src/main.rs", "fn main() {}").build();
    p.cargo("rustc -v")
        // bin
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name foo --edition=2015 src/main.rs [..]--crate-type bin \
             --emit=[..]link[..]",
        )
        // bench
        .with_stderr_does_not_contain(
            "[RUNNING] `rustc --crate-name foo --edition=2015 src/main.rs [..]--emit=[..]link \
             -C opt-level=3 --test [..]",
        )
        // unit test
        .with_stderr_does_not_contain(
            "[RUNNING] `rustc --crate-name foo --edition=2015 src/main.rs [..]--emit=[..]link \
             -C debuginfo=2 [..]--test [..]",
        )
        .run();
}

#[cargo_test]
fn targets_selected_all() {
    let p = project().file("src/main.rs", "fn main() {}").build();
    p.cargo("rustc -v --all-targets")
        // bin and unit test
        .with_stderr_data(
            str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo --edition=2015 src/main.rs [..]--crate-type bin --emit=[..]link[..]`
[RUNNING] `rustc --crate-name foo --edition=2015 src/main.rs [..]--emit[..]link[..] -C debuginfo=2 [..]--test [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]].unordered()
        )
        .run();
}

#[cargo_test]
fn fail_with_multiple_packages() {
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

                [dependencies.baz]
                    path = "../baz"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    let _bar = project()
        .at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file(
            "src/main.rs",
            r#"
                fn main() {
                    if cfg!(flag = "1") { println!("Yeah from bar!"); }
                }
            "#,
        )
        .build();

    let _baz = project()
        .at("baz")
        .file("Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file(
            "src/main.rs",
            r#"
                fn main() {
                    if cfg!(flag = "1") { println!("Yeah from baz!"); }
                }
            "#,
        )
        .build();

    foo.cargo("rustc -v -p bar -p baz")
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] the argument '--package [<SPEC>]' cannot be used multiple times
...
"#]])
        .run();
}

#[cargo_test]
fn fail_with_bad_bin_no_package() {
    let p = project()
        .file(
            "src/main.rs",
            r#"
                fn main() { println!("hello a.rs"); }
            "#,
        )
        .build();

    p.cargo("rustc --bin main")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no bin target named `main`
[HELP] available bin targets:
    foo
...

"#]])
        .run();
}

#[cargo_test]
fn fail_with_glob() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {  break_the_build(); }")
        .build();

    p.cargo("rustc -p '*z'")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] Glob patterns on package selection are not supported.

"#]])
        .run();
}

#[cargo_test]
fn rustc_with_other_profile() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dev-dependencies]
                a = { path = "a" }
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                #[cfg(test)] extern crate a;

                #[test]
                fn foo() {}
            "#,
        )
        .file("a/Cargo.toml", &basic_manifest("a", "0.1.0"))
        .file("a/src/lib.rs", "")
        .build();

    p.cargo("rustc --profile test").run();
}

#[cargo_test]
fn rustc_fingerprint() {
    // Verify that the fingerprint includes the rustc args.
    let p = project()
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file("src/lib.rs", "")
        .build();

    p.cargo("rustc -v -- -C debug-assertions")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]-C debug-assertions[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("rustc -v -- -C debug-assertions")
        .with_stderr_data(str![[r#"
[FRESH] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("rustc -v")
        .with_stderr_does_not_contain("-C debug-assertions")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("rustc -v")
        .with_stderr_data(str![[r#"
[FRESH] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn rustc_test_with_implicit_bin() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file(
            "src/main.rs",
            r#"
                #[cfg(foo)]
                fn f() { compile_fail!("Foo shouldn't be set."); }
                fn main() {}
            "#,
        )
        .file(
            "tests/test1.rs",
            r#"
                #[cfg(not(foo))]
                fn f() { compile_fail!("Foo should be set."); }
            "#,
        )
        .build();

    p.cargo("rustc --test test1 -v -- --cfg foo")
        .with_stderr_data(
            str![[r#"
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[RUNNING] `rustc --crate-name test1 --edition=2015 tests/test1.rs [..] --cfg foo[..]`
[RUNNING] `rustc --crate-name foo --edition=2015 src/main.rs [..]`
...
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn rustc_with_print_cfg_single_target() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", r#"fn main() {} "#)
        .build();

    p.cargo("rustc -Z unstable-options --target x86_64-pc-windows-msvc --print cfg")
        .masquerade_as_nightly_cargo(&["print"])
        .with_stdout_data(
            str![[r#"
debug_assertions
target_arch="x86_64"
target_endian="little"
target_env="msvc"
target_family="windows"
target_os="windows"
target_pointer_width="64"
target_vendor="pc"
windows
...
"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn rustc_with_print_cfg_multiple_targets() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", r#"fn main() {} "#)
        .build();

    p.cargo("rustc -Z unstable-options --target x86_64-pc-windows-msvc --target i686-unknown-linux-gnu --print cfg")
        .masquerade_as_nightly_cargo(&["print"])
        .with_stdout_data(str![[r#"
debug_assertions
target_arch="x86"
target_endian="little"
target_env="gnu"
target_family="unix"
target_os="linux"
target_pointer_width="32"
target_vendor="unknown"
unix
debug_assertions
target_arch="x86_64"
target_endian="little"
target_env="msvc"
target_family="windows"
target_os="windows"
target_pointer_width="64"
target_vendor="pc"
windows
...
"#]].unordered())
        .run();
}

#[cargo_test]
fn rustc_with_print_cfg_rustflags_env_var() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", r#"fn main() {} "#)
        .build();

    p.cargo("rustc -Z unstable-options --target x86_64-pc-windows-msvc --print cfg")
        .masquerade_as_nightly_cargo(&["print"])
        .env("RUSTFLAGS", "-C target-feature=+crt-static")
        .with_stdout_data(
            str![[r#"
debug_assertions
target_arch="x86_64"
target_endian="little"
target_env="msvc"
target_family="windows"
target_feature="crt-static"
target_os="windows"
target_pointer_width="64"
target_vendor="pc"
windows
...
"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn rustc_with_print_cfg_config_toml() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file(
            ".cargo/config.toml",
            r#"
[target.x86_64-pc-windows-msvc]
rustflags = ["-C", "target-feature=+crt-static"]
"#,
        )
        .file("src/main.rs", r#"fn main() {} "#)
        .build();

    p.cargo("rustc -Z unstable-options --target x86_64-pc-windows-msvc --print cfg")
        .masquerade_as_nightly_cargo(&["print"])
        .env("RUSTFLAGS", "-C target-feature=+crt-static")
        .with_stdout_data(
            str![[r#"
debug_assertions
target_arch="x86_64"
target_endian="little"
target_env="msvc"
target_family="windows"
target_feature="crt-static"
target_os="windows"
target_pointer_width="64"
target_vendor="pc"
windows
...
"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn rustc_with_print_cfg_config_toml_env() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("targets/best-target.json", target_spec_json())
        .file(
            ".cargo/config.toml",
            r#"
[build]
target = "best-target"
[env]
RUST_TARGET_PATH = { value = "./targets", relative = true }
"#,
        )
        .file("src/main.rs", r#"fn main() {} "#)
        .build();

    p.cargo("rustc -Z unstable-options --print cfg")
        .masquerade_as_nightly_cargo(&["print"])
        .with_stdout_data(str!["..."].unordered())
        .run();
}

#[cargo_test]
fn precedence() {
    // Ensure that the precedence of cargo-rustc is only lower than RUSTFLAGS,
    // but higher than most flags set by cargo.
    //
    // See rust-lang/cargo#14346
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            edition = "2021"

            [lints.rust]
            unexpected_cfgs = "allow"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("rustc --release -v -- --cfg cargo_rustc -C strip=symbols")
        .env("RUSTFLAGS", "--cfg from_rustflags")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.0 ([ROOT]/foo)
[RUNNING] `rustc [..]-C strip=debuginfo [..]--cfg cargo_rustc -C strip=symbols --cfg from_rustflags`
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn build_with_duplicate_crate_types() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("rustc -v --crate-type staticlib --crate-type staticlib")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..] --crate-type staticlib --emit[..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("rustc -v --crate-type staticlib --crate-type staticlib")
        .with_stderr_data(str![[r#"
[FRESH] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
