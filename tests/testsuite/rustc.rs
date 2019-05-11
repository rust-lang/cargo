use crate::support::{basic_bin_manifest, basic_lib_manifest, basic_manifest, project};

const CARGO_RUSTC_ERROR: &str =
    "[ERROR] extra arguments to `rustc` can only be passed to one target, consider filtering
the package by passing, e.g., `--lib` or `--bin NAME` to specify a single target";

#[test]
fn build_lib_for_foo() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("src/lib.rs", r#" "#)
        .build();

    p.cargo("rustc --lib -v")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `rustc --crate-name foo src/lib.rs --color never --crate-type lib \
        --emit=[..]link -C debuginfo=2 \
        -C metadata=[..] \
        --out-dir [..] \
        -L dependency=[CWD]/target/debug/deps`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[test]
fn lib() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("src/lib.rs", r#" "#)
        .build();

    p.cargo("rustc --lib -v -- -C debug-assertions=off")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `rustc --crate-name foo src/lib.rs --color never --crate-type lib \
        --emit=[..]link -C debuginfo=2 \
        -C debug-assertions=off \
        -C metadata=[..] \
        --out-dir [..] \
        -L dependency=[CWD]/target/debug/deps`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[test]
fn build_main_and_allow_unstable_options() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("src/lib.rs", r#" "#)
        .build();

    p.cargo("rustc -v --bin foo -- -C debug-assertions")
        .with_stderr(format!(
            "\
[COMPILING] {name} v{version} ([CWD])
[RUNNING] `rustc --crate-name {name} src/lib.rs --color never --crate-type lib \
        --emit=[..]link -C debuginfo=2 \
        -C metadata=[..] \
        --out-dir [..] \
        -L dependency=[CWD]/target/debug/deps`
[RUNNING] `rustc --crate-name {name} src/main.rs --color never --crate-type bin \
        --emit=[..]link -C debuginfo=2 \
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

#[test]
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

#[test]
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
[RUNNING] `rustc --crate-name foo src/lib.rs --color never --crate-type lib --emit=[..]link \
        -C debuginfo=2 -C metadata=[..] \
        --out-dir [..]`
[RUNNING] `rustc --crate-name bar src/bin/bar.rs --color never --crate-type bin --emit=[..]link \
        -C debuginfo=2 -C debug-assertions [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ).run();
}

#[test]
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

#[test]
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
[RUNNING] `rustc --crate-name foo src/lib.rs --color never --crate-type lib --emit=[..]link \
        -C debuginfo=2 -C metadata=[..] \
        --out-dir [..]`
[RUNNING] `rustc --crate-name bar tests/bar.rs --color never --emit=[..]link -C debuginfo=2 \
        -C debug-assertions [..]--test[..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[test]
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
[RUNNING] `[..] -C debuginfo=2 -C debug-assertions [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[test]
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
[RUNNING] `rustc --crate-name bar [..] --color never --crate-type lib [..] -C debug-assertions [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[test]
fn targets_selected_default() {
    let p = project().file("src/main.rs", "fn main() {}").build();
    p.cargo("rustc -v")
        // bin
        .with_stderr_contains(
            "\
             [RUNNING] `rustc --crate-name foo src/main.rs --color never --crate-type bin \
             --emit=[..]link[..]",
        )
        // bench
        .with_stderr_does_not_contain(
            "\
             [RUNNING] `rustc --crate-name foo src/main.rs --color never --emit=[..]link \
             -C opt-level=3 --test [..]",
        )
        // unit test
        .with_stderr_does_not_contain(
            "\
             [RUNNING] `rustc --crate-name foo src/main.rs --color never --emit=[..]link \
             -C debuginfo=2 --test [..]",
        )
        .run();
}

#[test]
fn targets_selected_all() {
    let p = project().file("src/main.rs", "fn main() {}").build();
    p.cargo("rustc -v --all-targets")
        // bin
        .with_stderr_contains(
            "\
             [RUNNING] `rustc --crate-name foo src/main.rs --color never --crate-type bin \
             --emit=[..]link[..]",
        )
        // unit test
        .with_stderr_contains(
            "\
             [RUNNING] `rustc --crate-name foo src/main.rs --color never --emit=[..]link \
             -C debuginfo=2 --test [..]",
        )
        .run();
}

#[test]
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
error: The argument '--package <SPEC>' was provided more than once, \
       but cannot be used multiple times
",
        )
        .run();
}

#[test]
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

#[test]
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

#[test]
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
            fn f() { compile_fail!("Foo should be set."); } "#,
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
