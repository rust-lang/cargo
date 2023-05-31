//! Tests for the `cargo rustc` command.

use cargo_test_support::{basic_bin_manifest, basic_lib_manifest, basic_manifest, project};

const CARGO_RUSTC_ERROR: &str =
    "[ERROR] extra arguments to `rustc` can only be passed to one target, consider filtering
the package by passing, e.g., `--lib` or `--bin NAME` to specify a single target";

#[cargo_test]
fn build_lib_for_foo() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("src/lib.rs", r#" "#)
        .build();

    p.cargo("rustc --lib -v")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `rustc --crate-name foo src/lib.rs [..]--crate-type lib \
        --emit=[..]link[..]-C debuginfo=2 [..]\
        -C metadata=[..] \
        --out-dir [..] \
        -L dependency=[CWD]/target/debug/deps`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn lib() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("src/lib.rs", r#" "#)
        .build();

    p.cargo("rustc --lib -v -- -C debug-assertions=off")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `rustc --crate-name foo src/lib.rs [..]--crate-type lib \
        --emit=[..]link[..]-C debuginfo=2 [..]\
        -C debug-assertions=off \
        -C metadata=[..] \
        --out-dir [..] \
        -L dependency=[CWD]/target/debug/deps`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn build_main_and_allow_unstable_options() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("src/lib.rs", r#" "#)
        .build();

    p.cargo("rustc -v --bin foo -- -C debug-assertions")
        .with_stderr(format!(
            "\
[COMPILING] {name} v{version} ([CWD])
[RUNNING] `rustc --crate-name {name} src/lib.rs [..]--crate-type lib \
        --emit=[..]link[..]-C debuginfo=2 [..]\
        -C metadata=[..] \
        --out-dir [..] \
        -L dependency=[CWD]/target/debug/deps`
[RUNNING] `rustc --crate-name {name} src/main.rs [..]--crate-type bin \
        --emit=[..]link[..]-C debuginfo=2 [..]\
        -C debug-assertions \
        -C metadata=[..] \
        --out-dir [..] \
        -L dependency=[CWD]/target/debug/deps \
        --extern {name}=[CWD]/target/debug/deps/lib{name}-[..].rlib`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            name = "foo",
            version = "0.0.1"
        ))
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
        .with_stderr(CARGO_RUSTC_ERROR)
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
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `rustc --crate-name foo src/lib.rs [..]--crate-type lib --emit=[..]link[..]\
        -C debuginfo=2 [..]-C metadata=[..] \
        --out-dir [..]`
[RUNNING] `rustc --crate-name bar src/bin/bar.rs [..]--crate-type bin --emit=[..]link[..]\
        -C debuginfo=2 [..]-C debug-assertions [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
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
        .with_stderr(CARGO_RUSTC_ERROR)
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
        .with_stderr(
            "[ERROR] crate types to rustc can only be passed to one target, consider filtering
the package by passing, e.g., `--lib` or `--example` to specify a single target",
        )
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
        .with_stderr(
            "[ERROR] crate types to rustc can only be passed to one target, consider filtering
the package by passing, e.g., `--lib` or `--example` to specify a single target",
        )
        .run();
}

#[cargo_test]
fn fails_with_crate_type_to_binary() {
    let p = project().file("src/bin/foo.rs", "fn main() {}").build();

    p.cargo("rustc --crate-type lib")
        .with_status(101)
        .with_stderr(
            "[ERROR] crate types can only be specified for libraries and example libraries.
Binaries, tests, and benchmarks are always the `bin` crate type",
        )
        .run();
}

#[cargo_test]
fn build_with_crate_type_for_foo() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("rustc -v --crate-type cdylib")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `rustc --crate-name foo src/lib.rs [..]--crate-type cdylib [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
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
            authors = []

            [dependencies]
            a = { path = "a" }
            "#,
        )
        .file("a/Cargo.toml", &basic_manifest("a", "0.1.0"))
        .file("a/src/lib.rs", "pub fn hello() {}")
        .build();

    p.cargo("rustc -v --crate-type cdylib")
        .with_stderr(
            "\
[COMPILING] a v0.1.0 ([CWD]/a)
[RUNNING] `rustc --crate-name a a/src/lib.rs [..]--crate-type lib [..]
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `rustc --crate-name foo src/lib.rs [..]--crate-type cdylib [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn build_with_crate_types_for_foo() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("rustc -v --crate-type lib,cdylib")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `rustc --crate-name foo src/lib.rs [..]--crate-type lib,cdylib [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
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
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `rustc --crate-name foo src/lib.rs [..]--crate-type lib [..]
[RUNNING] `rustc --crate-name ex examples/ex.rs [..]--crate-type cdylib [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
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
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `rustc --crate-name foo src/lib.rs [..]--crate-type lib [..]
[RUNNING] `rustc --crate-name ex examples/ex.rs [..]--crate-type lib,cdylib [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
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
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `rustc --crate-name foo src/lib.rs [..]--crate-type lib [..]
[RUNNING] `rustc --crate-name ex1 examples/ex1.rs [..]--crate-type lib,cdylib [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
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
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `rustc --crate-name foo src/lib.rs [..]--crate-type lib --emit=[..]link[..]\
        -C debuginfo=2 [..]-C metadata=[..] \
        --out-dir [..]`
[RUNNING] `rustc --crate-name bar tests/bar.rs [..]--emit=[..]link[..]-C debuginfo=2 [..]\
        -C debug-assertions [..]--test[..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
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
        .with_stderr(
            "\
[COMPILING] bar v0.1.0 ([..])
[RUNNING] `[..] -C debuginfo=2 [..]`
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `[..] -C debuginfo=2 [..]-C debug-assertions [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
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
        .with_stderr(
            "\
[COMPILING] bar v0.1.0 ([..])
[RUNNING] `rustc --crate-name bar [..]--crate-type lib [..] -C debug-assertions [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn targets_selected_default() {
    let p = project().file("src/main.rs", "fn main() {}").build();
    p.cargo("rustc -v")
        // bin
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name foo src/main.rs [..]--crate-type bin \
             --emit=[..]link[..]",
        )
        // bench
        .with_stderr_does_not_contain(
            "[RUNNING] `rustc --crate-name foo src/main.rs [..]--emit=[..]link \
             -C opt-level=3 --test [..]",
        )
        // unit test
        .with_stderr_does_not_contain(
            "[RUNNING] `rustc --crate-name foo src/main.rs [..]--emit=[..]link \
             -C debuginfo=2 [..]--test [..]",
        )
        .run();
}

#[cargo_test]
fn targets_selected_all() {
    let p = project().file("src/main.rs", "fn main() {}").build();
    p.cargo("rustc -v --all-targets")
        // bin
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name foo src/main.rs [..]--crate-type bin \
             --emit=[..]link[..]",
        )
        // unit test
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name foo src/main.rs [..]--emit=[..]link[..]\
             -C debuginfo=2 [..]--test [..]",
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
        .with_stderr_contains(
            "\
error: the argument '--package [<SPEC>]' cannot be used multiple times
",
        )
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
        .with_stderr("[ERROR] Glob patterns on package selection are not supported.")
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
        .with_stderr(
            "\
[COMPILING] foo [..]
[RUNNING] `rustc [..]-C debug-assertions [..]
[FINISHED] [..]
",
        )
        .run();

    p.cargo("rustc -v -- -C debug-assertions")
        .with_stderr(
            "\
[FRESH] foo [..]
[FINISHED] [..]
",
        )
        .run();

    p.cargo("rustc -v")
        .with_stderr_does_not_contain("-C debug-assertions")
        .with_stderr(
            "\
[DIRTY] foo [..]: the profile configuration changed
[COMPILING] foo [..]
[RUNNING] `rustc [..]
[FINISHED] [..]
",
        )
        .run();

    p.cargo("rustc -v")
        .with_stderr(
            "\
[FRESH] foo [..]
[FINISHED] [..]
",
        )
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
        .with_stderr_contains(
            "\
[RUNNING] `rustc --crate-name test1 tests/test1.rs [..] --cfg foo [..]
",
        )
        .with_stderr_contains(
            "\
[RUNNING] `rustc --crate-name foo src/main.rs [..]
",
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
        .with_stdout_contains("debug_assertions")
        .with_stdout_contains("target_arch=\"x86_64\"")
        .with_stdout_contains("target_endian=\"little\"")
        .with_stdout_contains("target_env=\"msvc\"")
        .with_stdout_contains("target_family=\"windows\"")
        .with_stdout_contains("target_os=\"windows\"")
        .with_stdout_contains("target_pointer_width=\"64\"")
        .with_stdout_contains("target_vendor=\"pc\"")
        .with_stdout_contains("windows")
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
        .with_stdout_contains("debug_assertions")
        .with_stdout_contains("target_arch=\"x86_64\"")
        .with_stdout_contains("target_endian=\"little\"")
        .with_stdout_contains("target_env=\"msvc\"")
        .with_stdout_contains("target_family=\"windows\"")
        .with_stdout_contains("target_os=\"windows\"")
        .with_stdout_contains("target_pointer_width=\"64\"")
        .with_stdout_contains("target_vendor=\"pc\"")
        .with_stdout_contains("windows")
        .with_stdout_contains("target_env=\"gnu\"")
        .with_stdout_contains("target_family=\"unix\"")
        .with_stdout_contains("target_pointer_width=\"32\"")
        .with_stdout_contains("target_vendor=\"unknown\"")
        .with_stdout_contains("target_os=\"linux\"")
        .with_stdout_contains("unix")
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
        .with_stdout_contains("debug_assertions")
        .with_stdout_contains("target_arch=\"x86_64\"")
        .with_stdout_contains("target_endian=\"little\"")
        .with_stdout_contains("target_env=\"msvc\"")
        .with_stdout_contains("target_family=\"windows\"")
        .with_stdout_contains("target_feature=\"crt-static\"")
        .with_stdout_contains("target_os=\"windows\"")
        .with_stdout_contains("target_pointer_width=\"64\"")
        .with_stdout_contains("target_vendor=\"pc\"")
        .with_stdout_contains("windows")
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
        .with_stdout_contains("debug_assertions")
        .with_stdout_contains("target_arch=\"x86_64\"")
        .with_stdout_contains("target_endian=\"little\"")
        .with_stdout_contains("target_env=\"msvc\"")
        .with_stdout_contains("target_family=\"windows\"")
        .with_stdout_contains("target_feature=\"crt-static\"")
        .with_stdout_contains("target_os=\"windows\"")
        .with_stdout_contains("target_pointer_width=\"64\"")
        .with_stdout_contains("target_vendor=\"pc\"")
        .with_stdout_contains("windows")
        .run();
}
