//! Tests for the `cargo test` command.

use cargo_test_support::paths::CargoPathExt;
use cargo_test_support::registry::Package;
use cargo_test_support::{
    basic_bin_manifest, basic_lib_manifest, basic_manifest, cargo_exe, project,
};
use cargo_test_support::{cross_compile, is_nightly, paths};
use cargo_test_support::{rustc_host, sleep_ms};
use std::fs;

#[cargo_test]
fn cargo_test_simple() {
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

            #[test]
            fn test_hello() {
                assert_eq!(hello(), "hello")
            }
            "#,
        )
        .build();

    p.cargo("build").run();
    assert!(p.bin("foo").is_file());

    p.process(&p.bin("foo")).with_stdout("hello\n").run();

    p.cargo("test")
        .with_stderr(&format!(
            "\
[COMPILING] foo v0.5.0 ([CWD])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/{}/debug/deps/foo-[..][EXE])",
            rustc_host()
        ))
        .with_stdout_contains("test test_hello ... ok")
        .run();
}

#[cargo_test]
fn cargo_test_release() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                authors = []
                version = "0.1.0"

                [dependencies]
                bar = { path = "bar" }
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                extern crate bar;
                pub fn foo() { bar::bar(); }

                #[test]
                fn test() { foo(); }
            "#,
        )
        .file(
            "tests/test.rs",
            r#"
                extern crate foo;

                #[test]
                fn test() { foo::foo(); }
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();

    p.cargo("test -v --release")
        .with_stderr(&format!(
            "\
[COMPILING] bar v0.0.1 ([CWD]/bar)
[RUNNING] [..] -C opt-level=3 [..]
[COMPILING] foo v0.1.0 ([CWD])
[RUNNING] [..] -C opt-level=3 [..]
[RUNNING] [..] -C opt-level=3 [..]
[RUNNING] [..] -C opt-level=3 [..]
[FINISHED] release [optimized] target(s) in [..]
[RUNNING] `[..]target/{target}/release/deps/foo-[..][EXE]`
[RUNNING] `[..]target/{target}/release/deps/test-[..][EXE]`
[DOCTEST] foo
[RUNNING] `rustdoc [..]--test [..]lib.rs[..]`",
            target = rustc_host()
        ))
        .with_stdout_contains_n("test test ... ok", 2)
        .with_stdout_contains("running 0 tests")
        .run();
}

#[cargo_test]
fn cargo_test_overflow_checks() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.5.0"
            authors = []

            [[bin]]
            name = "foo"

            [profile.release]
            overflow-checks = true
            "#,
        )
        .file(
            "src/foo.rs",
            r#"
            use std::panic;
            pub fn main() {
                let r = panic::catch_unwind(|| {
                    [1, i32::MAX].iter().sum::<i32>();
                });
                assert!(r.is_err());
            }
            "#,
        )
        .build();

    p.cargo("build --release").run();
    assert!(p.release_bin("foo").is_file());

    p.process(&p.release_bin("foo")).with_stdout("").run();
}

#[cargo_test]
fn cargo_test_quiet_with_harness() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [[test]]
                name = "foo"
                path = "src/foo.rs"
                harness = true
            "#,
        )
        .file(
            "src/foo.rs",
            r#"
                fn main() {}
                #[test] fn test_hello() {}
            "#,
        )
        .build();

    p.cargo("test -q")
        .with_stdout(
            "
running 1 test
.
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out[..]

",
        )
        .with_stderr("")
        .run();
}

#[cargo_test]
fn cargo_test_quiet_no_harness() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [[bin]]
                name = "foo"
                test = false

                [[test]]
                name = "foo"
                path = "src/main.rs"
                harness = false
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                fn main() {}
                #[test] fn test_hello() {}
            "#,
        )
        .build();

    p.cargo("test -q").with_stdout("").with_stderr("").run();
}

#[cargo_test]
fn cargo_test_verbose() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file(
            "src/main.rs",
            r#"
                fn main() {}
                #[test] fn test_hello() {}
            "#,
        )
        .build();

    p.cargo("test -v hello")
        .with_stderr(&format!(
            "\
[COMPILING] foo v0.5.0 ([CWD])
[RUNNING] `rustc [..] src/main.rs [..]`
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[CWD]/target/{}/debug/deps/foo-[..] hello`
",
            rustc_host()
        ))
        .with_stdout_contains("test test_hello ... ok")
        .run();
}

#[cargo_test]
fn many_similar_names() {
    let p = project()
        .file(
            "src/lib.rs",
            "
            pub fn foo() {}
            #[test] fn lib_test() {}
        ",
        )
        .file(
            "src/main.rs",
            "
            extern crate foo;
            fn main() {}
            #[test] fn bin_test() { foo::foo() }
        ",
        )
        .file(
            "tests/foo.rs",
            r#"
                extern crate foo;
                #[test] fn test_test() { foo::foo() }
            "#,
        )
        .build();

    p.cargo("test -v")
        .with_stdout_contains("test bin_test ... ok")
        .with_stdout_contains("test lib_test ... ok")
        .with_stdout_contains("test test_test ... ok")
        .run();
}

#[cargo_test]
fn cargo_test_failing_test_in_bin() {
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

            #[test]
            fn test_hello() {
                assert_eq!(hello(), "nope")
            }
            "#,
        )
        .build();

    p.cargo("build").run();
    assert!(p.bin("foo").is_file());

    p.process(&p.bin("foo")).with_stdout("hello\n").run();

    p.cargo("test")
        .with_stderr(&format!(
            "\
[COMPILING] foo v0.5.0 ([CWD])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/{}/debug/deps/foo-[..][EXE])
[ERROR] test failed, to rerun pass '--bin foo'",
            rustc_host()
        ))
        .with_stdout_contains(
            "
running 1 test
test test_hello ... FAILED

failures:

---- test_hello stdout ----
[..]thread '[..]' panicked at 'assertion failed:[..]",
        )
        .with_stdout_contains("[..]`(left == right)`[..]")
        .with_stdout_contains("[..]left: `\"hello\"`,[..]")
        .with_stdout_contains("[..]right: `\"nope\"`[..]")
        .with_stdout_contains("[..]src/main.rs:12[..]")
        .with_stdout_contains(
            "\
failures:
    test_hello
",
        )
        .with_status(101)
        .run();
}

#[cargo_test]
fn cargo_test_failing_test_in_test() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", r#"pub fn main() { println!("hello"); }"#)
        .file(
            "tests/footest.rs",
            "#[test] fn test_hello() { assert!(false) }",
        )
        .build();

    p.cargo("build").run();
    assert!(p.bin("foo").is_file());

    p.process(&p.bin("foo")).with_stdout("hello\n").run();

    p.cargo("test")
        .with_stderr(&format!(
            "\
[COMPILING] foo v0.5.0 ([CWD])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/{target}/debug/deps/foo-[..][EXE])
[RUNNING] [..] (target/{target}/debug/deps/footest-[..][EXE])
[ERROR] test failed, to rerun pass '--test footest'",
            target = rustc_host()
        ))
        .with_stdout_contains("running 0 tests")
        .with_stdout_contains(
            "\
running 1 test
test test_hello ... FAILED

failures:

---- test_hello stdout ----
[..]thread '[..]' panicked at 'assertion failed: false', \
      tests/footest.rs:1[..]
",
        )
        .with_stdout_contains(
            "\
failures:
    test_hello
",
        )
        .with_status(101)
        .run();
}

#[cargo_test]
fn cargo_test_failing_test_in_lib() {
    let p = project()
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file("src/lib.rs", "#[test] fn test_hello() { assert!(false) }")
        .build();

    p.cargo("test")
        .with_stderr(&format!(
            "\
[COMPILING] foo v0.5.0 ([CWD])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/{}/debug/deps/foo-[..][EXE])
[ERROR] test failed, to rerun pass '--lib'",
            rustc_host()
        ))
        .with_stdout_contains(
            "\
test test_hello ... FAILED

failures:

---- test_hello stdout ----
[..]thread '[..]' panicked at 'assertion failed: false', \
      src/lib.rs:1[..]
",
        )
        .with_stdout_contains(
            "\
failures:
    test_hello
",
        )
        .with_status(101)
        .run();
}

#[cargo_test]
fn test_with_lib_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [[bin]]
                name = "baz"
                path = "src/main.rs"
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                ///
                /// ```rust
                /// extern crate foo;
                /// fn main() {
                ///     println!("{:?}", foo::foo());
                /// }
                /// ```
                ///
                pub fn foo(){}
                #[test] fn lib_test() {}
            "#,
        )
        .file(
            "src/main.rs",
            "
            #[allow(unused_extern_crates)]
            extern crate foo;

            fn main() {}

            #[test]
            fn bin_test() {}
        ",
        )
        .build();

    p.cargo("test")
        .with_stderr(format!(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/{target}/debug/deps/foo-[..][EXE])
[RUNNING] [..] (target/{target}/debug/deps/baz-[..][EXE])
[DOCTEST] foo",
            target = rustc_host()
        ))
        .with_stdout_contains("test lib_test ... ok")
        .with_stdout_contains("test bin_test ... ok")
        .with_stdout_contains_n("test [..] ... ok", 3)
        .run();
}

#[cargo_test]
fn test_with_deep_lib_dep() {
    let p = project()
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
        .file(
            "src/lib.rs",
            "
            #[cfg(test)]
            extern crate bar;
            /// ```
            /// foo::foo();
            /// ```
            pub fn foo() {}

            #[test]
            fn bar_test() {
                bar::bar();
            }
        ",
        )
        .build();
    let _p2 = project()
        .at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("src/lib.rs", "pub fn bar() {} #[test] fn foo_test() {}")
        .build();

    p.cargo("test")
        .with_stderr(
            "\
[COMPILING] bar v0.0.1 ([..])
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target[..])
[DOCTEST] foo",
        )
        .with_stdout_contains("test bar_test ... ok")
        .with_stdout_contains_n("test [..] ... ok", 2)
        .run();
}

#[cargo_test]
fn external_test_explicit() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [[test]]
                name = "test"
                path = "src/test.rs"
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                pub fn get_hello() -> &'static str { "Hello" }

                #[test]
                fn internal_test() {}
            "#,
        )
        .file(
            "src/test.rs",
            r#"
                extern crate foo;

                #[test]
                fn external_test() { assert_eq!(foo::get_hello(), "Hello") }
            "#,
        )
        .build();

    p.cargo("test")
        .with_stderr(&format!(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/{target}/debug/deps/foo-[..][EXE])
[RUNNING] [..] (target/{target}/debug/deps/test-[..][EXE])
[DOCTEST] foo",
            target = rustc_host()
        ))
        .with_stdout_contains("test internal_test ... ok")
        .with_stdout_contains("test external_test ... ok")
        .with_stdout_contains("running 0 tests")
        .run();
}

#[cargo_test]
fn external_test_named_test() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [[test]]
                name = "test"
            "#,
        )
        .file("src/lib.rs", "")
        .file("tests/test.rs", "#[test] fn foo() {}")
        .build();

    p.cargo("test").run();
}

#[cargo_test]
fn external_test_implicit() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
                pub fn get_hello() -> &'static str { "Hello" }

                #[test]
                fn internal_test() {}
            "#,
        )
        .file(
            "tests/external.rs",
            r#"
                extern crate foo;

                #[test]
                fn external_test() { assert_eq!(foo::get_hello(), "Hello") }
            "#,
        )
        .build();

    p.cargo("test")
        .with_stderr(&format!(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/{target}/debug/deps/foo-[..][EXE])
[RUNNING] [..] (target/{target}/debug/deps/external-[..][EXE])
[DOCTEST] foo",
            target = rustc_host()
        ))
        .with_stdout_contains("test internal_test ... ok")
        .with_stdout_contains("test external_test ... ok")
        .with_stdout_contains("running 0 tests")
        .run();
}

#[cargo_test]
fn dont_run_examples() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            "examples/dont-run-me-i-will-fail.rs",
            r#"
                fn main() { panic!("Examples should not be run by 'cargo test'"); }
            "#,
        )
        .build();
    p.cargo("test").run();
}

#[cargo_test]
fn pass_through_command_line() {
    let p = project()
        .file(
            "src/lib.rs",
            "
            #[test] fn foo() {}
            #[test] fn bar() {}
        ",
        )
        .build();

    p.cargo("test bar")
        .with_stderr(format!(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/{}/debug/deps/foo-[..][EXE])
",
            rustc_host()
        ))
        .with_stdout_contains("running 1 test")
        .with_stdout_contains("test bar ... ok")
        .run();

    p.cargo("test foo")
        .with_stderr(format!(
            "\
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/{}/debug/deps/foo-[..][EXE])
",
            rustc_host()
        ))
        .with_stdout_contains("running 1 test")
        .with_stdout_contains("test foo ... ok")
        .run();
}

// Regression test for running cargo-test twice with
// tests in an rlib
#[cargo_test]
fn cargo_test_twice() {
    let p = project()
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file(
            "src/foo.rs",
            r#"
            #![crate_type = "rlib"]

            #[test]
            fn dummy_test() { }
            "#,
        )
        .build();

    for _ in 0..2 {
        p.cargo("test").run();
    }
}

#[cargo_test]
fn lib_bin_same_name() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [lib]
                name = "foo"
                [[bin]]
                name = "foo"
            "#,
        )
        .file("src/lib.rs", "#[test] fn lib_test() {}")
        .file(
            "src/main.rs",
            "
            #[allow(unused_extern_crates)]
            extern crate foo;

            #[test]
            fn bin_test() {}
        ",
        )
        .build();

    p.cargo("test")
        .with_stderr(format!(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/{target}/debug/deps/foo-[..][EXE])
[RUNNING] [..] (target/{target}/debug/deps/foo-[..][EXE])
[DOCTEST] foo",
            target = rustc_host()
        ))
        .with_stdout_contains_n("test [..] ... ok", 2)
        .with_stdout_contains("running 0 tests")
        .run();
}

#[cargo_test]
fn lib_with_standard_name() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("syntax", "0.0.1"))
        .file(
            "src/lib.rs",
            "
            /// ```
            /// syntax::foo();
            /// ```
            pub fn foo() {}

            #[test]
            fn foo_test() {}
        ",
        )
        .file(
            "tests/test.rs",
            "
            extern crate syntax;

            #[test]
            fn test() { syntax::foo() }
        ",
        )
        .build();

    p.cargo("test")
        .with_stderr(format!(
            "\
[COMPILING] syntax v0.0.1 ([CWD])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/{target}/debug/deps/syntax-[..][EXE])
[RUNNING] [..] (target/{target}/debug/deps/test-[..][EXE])
[DOCTEST] syntax",
            target = rustc_host()
        ))
        .with_stdout_contains("test foo_test ... ok")
        .with_stdout_contains("test test ... ok")
        .with_stdout_contains_n("test [..] ... ok", 3)
        .run();
}

#[cargo_test]
fn lib_with_standard_name2() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "syntax"
                version = "0.0.1"
                authors = []

                [lib]
                name = "syntax"
                test = false
                doctest = false
            "#,
        )
        .file("src/lib.rs", "pub fn foo() {}")
        .file(
            "src/main.rs",
            "
            extern crate syntax;

            fn main() {}

            #[test]
            fn test() { syntax::foo() }
        ",
        )
        .build();

    p.cargo("test")
        .with_stderr(format!(
            "\
[COMPILING] syntax v0.0.1 ([CWD])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/{}/debug/deps/syntax-[..][EXE])",
            rustc_host()
        ))
        .with_stdout_contains("test test ... ok")
        .run();
}

#[cargo_test]
fn lib_without_name() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "syntax"
                version = "0.0.1"
                authors = []

                [lib]
                test = false
                doctest = false
            "#,
        )
        .file("src/lib.rs", "pub fn foo() {}")
        .file(
            "src/main.rs",
            "
            extern crate syntax;

            fn main() {}

            #[test]
            fn test() { syntax::foo() }
        ",
        )
        .build();

    p.cargo("test")
        .with_stderr(format!(
            "\
[COMPILING] syntax v0.0.1 ([CWD])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/{}/debug/deps/syntax-[..][EXE])",
            rustc_host()
        ))
        .with_stdout_contains("test test ... ok")
        .run();
}

#[cargo_test]
fn bin_without_name() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "syntax"
                version = "0.0.1"
                authors = []

                [lib]
                test = false
                doctest = false

                [[bin]]
                path = "src/main.rs"
            "#,
        )
        .file("src/lib.rs", "pub fn foo() {}")
        .file(
            "src/main.rs",
            "
            extern crate syntax;

            fn main() {}

            #[test]
            fn test() { syntax::foo() }
        ",
        )
        .build();

    p.cargo("test")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  binary target bin.name is required",
        )
        .run();
}

#[cargo_test]
fn bench_without_name() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "syntax"
                version = "0.0.1"
                authors = []

                [lib]
                test = false
                doctest = false

                [[bench]]
                path = "src/bench.rs"
            "#,
        )
        .file("src/lib.rs", "pub fn foo() {}")
        .file(
            "src/main.rs",
            "
            extern crate syntax;

            fn main() {}

            #[test]
            fn test() { syntax::foo() }
        ",
        )
        .file(
            "src/bench.rs",
            "
            #![feature(test)]
            extern crate syntax;
            extern crate test;

            #[bench]
            fn external_bench(_b: &mut test::Bencher) {}
        ",
        )
        .build();

    p.cargo("test")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  benchmark target bench.name is required",
        )
        .run();
}

#[cargo_test]
fn test_without_name() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "syntax"
                version = "0.0.1"
                authors = []

                [lib]
                test = false
                doctest = false

                [[test]]
                path = "src/test.rs"
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                pub fn foo() {}
                pub fn get_hello() -> &'static str { "Hello" }
            "#,
        )
        .file(
            "src/main.rs",
            "
            extern crate syntax;

            fn main() {}

            #[test]
            fn test() { syntax::foo() }
        ",
        )
        .file(
            "src/test.rs",
            r#"
                extern crate syntax;

                #[test]
                fn external_test() { assert_eq!(syntax::get_hello(), "Hello") }
            "#,
        )
        .build();

    p.cargo("test")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  test target test.name is required",
        )
        .run();
}

#[cargo_test]
fn example_without_name() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "syntax"
                version = "0.0.1"
                authors = []

                [lib]
                test = false
                doctest = false

                [[example]]
                path = "examples/example.rs"
            "#,
        )
        .file("src/lib.rs", "pub fn foo() {}")
        .file(
            "src/main.rs",
            "
            extern crate syntax;

            fn main() {}

            #[test]
            fn test() { syntax::foo() }
        ",
        )
        .file(
            "examples/example.rs",
            r#"
                extern crate syntax;

                fn main() {
                    println!("example1");
                }
            "#,
        )
        .build();

    p.cargo("test")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  example target example.name is required",
        )
        .run();
}

#[cargo_test]
fn bin_there_for_integration() {
    let p = project()
        .file(
            "src/main.rs",
            "
            fn main() { std::process::exit(101); }
            #[test] fn main_test() {}
        ",
        )
        .file(
            "tests/foo.rs",
            &format!(
                r#"
                    use std::process::Command;
                    #[test]
                    fn test_test() {{
                        let status = Command::new("target/{}/debug/foo").status().unwrap();
                        assert_eq!(status.code(), Some(101));
                    }}
                "#,
                rustc_host()
            ),
        )
        .build();

    p.cargo("test -v")
        .with_stdout_contains("test main_test ... ok")
        .with_stdout_contains("test test_test ... ok")
        .run();
}

#[cargo_test]
fn test_dylib() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [lib]
                name = "foo"
                crate_type = ["dylib"]

                [dependencies.bar]
                path = "bar"
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                extern crate bar as the_bar;

                pub fn bar() { the_bar::baz(); }

                #[test]
                fn foo() { bar(); }
            "#,
        )
        .file(
            "tests/test.rs",
            r#"
                extern crate foo as the_foo;

                #[test]
                fn foo() { the_foo::bar(); }
            "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                authors = []

                [lib]
                name = "bar"
                crate_type = ["dylib"]
            "#,
        )
        .file("bar/src/lib.rs", "pub fn baz() {}")
        .build();

    p.cargo("test")
        .with_stderr(format!(
            "\
[COMPILING] bar v0.0.1 ([CWD]/bar)
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/{target}/debug/deps/foo-[..][EXE])
[RUNNING] [..] (target/{target}/debug/deps/test-[..][EXE])",
            target = rustc_host()
        ))
        .with_stdout_contains_n("test foo ... ok", 2)
        .run();

    p.root().move_into_the_past();
    p.cargo("test")
        .with_stderr(format!(
            "\
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/{target}/debug/deps/foo-[..][EXE])
[RUNNING] [..] (target/{target}/debug/deps/test-[..][EXE])",
            target = rustc_host()
        ))
        .with_stdout_contains_n("test foo ... ok", 2)
        .run();
}

#[cargo_test]
fn test_twice_with_build_cmd() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                build = "build.rs"
            "#,
        )
        .file("build.rs", "fn main() {}")
        .file("src/lib.rs", "#[test] fn foo() {}")
        .build();

    p.cargo("test")
        .with_stderr(format!(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/{}/debug/deps/foo-[..][EXE])
[DOCTEST] foo",
            rustc_host()
        ))
        .with_stdout_contains("test foo ... ok")
        .with_stdout_contains("running 0 tests")
        .run();

    p.cargo("test")
        .with_stderr(format!(
            "\
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/{}/debug/deps/foo-[..][EXE])
[DOCTEST] foo",
            rustc_host()
        ))
        .with_stdout_contains("test foo ... ok")
        .with_stdout_contains("running 0 tests")
        .run();
}

#[cargo_test]
fn test_then_build() {
    let p = project().file("src/lib.rs", "#[test] fn foo() {}").build();

    p.cargo("test")
        .with_stderr(format!(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/{}/debug/deps/foo-[..][EXE])
[DOCTEST] foo",
            rustc_host()
        ))
        .with_stdout_contains("test foo ... ok")
        .with_stdout_contains("running 0 tests")
        .run();

    p.cargo("build").with_stdout("").run();
}

#[cargo_test]
fn test_no_run() {
    let p = project()
        .file("src/lib.rs", "#[test] fn foo() { panic!() }")
        .build();

    p.cargo("test --no-run")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn test_run_specific_bin_target() {
    let prj = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [[bin]]
                name="bin1"
                path="src/bin1.rs"

                [[bin]]
                name="bin2"
                path="src/bin2.rs"
            "#,
        )
        .file("src/bin1.rs", "#[test] fn test1() { }")
        .file("src/bin2.rs", "#[test] fn test2() { }")
        .build();

    prj.cargo("test --bin bin2")
        .with_stderr(format!(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/{}/debug/deps/bin2-[..][EXE])",
            rustc_host()
        ))
        .with_stdout_contains("test test2 ... ok")
        .run();
}

#[cargo_test]
fn test_run_implicit_bin_target() {
    let prj = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [[bin]]
                name="mybin"
                path="src/mybin.rs"
            "#,
        )
        .file(
            "src/mybin.rs",
            "#[test] fn test_in_bin() { }
               fn main() { panic!(\"Don't execute me!\"); }",
        )
        .file("tests/mytest.rs", "#[test] fn test_in_test() { }")
        .file("benches/mybench.rs", "#[test] fn test_in_bench() { }")
        .file(
            "examples/myexm.rs",
            "#[test] fn test_in_exm() { }
               fn main() { panic!(\"Don't execute me!\"); }",
        )
        .build();

    prj.cargo("test --bins")
        .with_stderr(format!(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/{}/debug/deps/mybin-[..][EXE])",
            rustc_host()
        ))
        .with_stdout_contains("test test_in_bin ... ok")
        .run();
}

#[cargo_test]
fn test_run_specific_test_target() {
    let prj = project()
        .file("src/bin/a.rs", "fn main() { }")
        .file("src/bin/b.rs", "#[test] fn test_b() { } fn main() { }")
        .file("tests/a.rs", "#[test] fn test_a() { }")
        .file("tests/b.rs", "#[test] fn test_b() { }")
        .build();

    prj.cargo("test --test b")
        .with_stderr(format!(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/{}/debug/deps/b-[..][EXE])",
            rustc_host()
        ))
        .with_stdout_contains("test test_b ... ok")
        .run();
}

#[cargo_test]
fn test_run_implicit_test_target() {
    let prj = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [[bin]]
                name="mybin"
                path="src/mybin.rs"
            "#,
        )
        .file(
            "src/mybin.rs",
            "#[test] fn test_in_bin() { }
               fn main() { panic!(\"Don't execute me!\"); }",
        )
        .file("tests/mytest.rs", "#[test] fn test_in_test() { }")
        .file("benches/mybench.rs", "#[test] fn test_in_bench() { }")
        .file(
            "examples/myexm.rs",
            "fn main() { compile_error!(\"Don't build me!\"); }",
        )
        .build();

    prj.cargo("test --tests")
        .with_stderr(format!(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/{target}/debug/deps/mybin-[..][EXE])
[RUNNING] [..] (target/{target}/debug/deps/mytest-[..][EXE])",
            target = rustc_host()
        ))
        .with_stdout_contains("test test_in_test ... ok")
        .run();
}

#[cargo_test]
fn test_run_implicit_bench_target() {
    let prj = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [[bin]]
                name="mybin"
                path="src/mybin.rs"
            "#,
        )
        .file(
            "src/mybin.rs",
            "#[test] fn test_in_bin() { }
               fn main() { panic!(\"Don't execute me!\"); }",
        )
        .file("tests/mytest.rs", "#[test] fn test_in_test() { }")
        .file("benches/mybench.rs", "#[test] fn test_in_bench() { }")
        .file(
            "examples/myexm.rs",
            "fn main() { compile_error!(\"Don't build me!\"); }",
        )
        .build();

    prj.cargo("test --benches")
        .with_stderr(format!(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/{target}/debug/deps/mybin-[..][EXE])
[RUNNING] [..] (target/{target}/debug/deps/mybench-[..][EXE])",
            target = rustc_host()
        ))
        .with_stdout_contains("test test_in_bench ... ok")
        .run();
}

#[cargo_test]
fn test_run_implicit_example_target() {
    let prj = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [[bin]]
                name = "mybin"
                path = "src/mybin.rs"

                [[example]]
                name = "myexm1"

                [[example]]
                name = "myexm2"
                test = true
            "#,
        )
        .file(
            "src/mybin.rs",
            "#[test] fn test_in_bin() { }
               fn main() { panic!(\"Don't execute me!\"); }",
        )
        .file("tests/mytest.rs", "#[test] fn test_in_test() { }")
        .file("benches/mybench.rs", "#[test] fn test_in_bench() { }")
        .file(
            "examples/myexm1.rs",
            "#[test] fn test_in_exm() { }
               fn main() { panic!(\"Don't execute me!\"); }",
        )
        .file(
            "examples/myexm2.rs",
            "#[test] fn test_in_exm() { }
               fn main() { panic!(\"Don't execute me!\"); }",
        )
        .build();

    // Compiles myexm1 as normal, but does not run it.
    prj.cargo("test -v")
        .with_stderr_contains("[RUNNING] `rustc [..]myexm1.rs [..]--crate-type bin[..]")
        .with_stderr_contains("[RUNNING] `rustc [..]myexm2.rs [..]--test[..]")
        .with_stderr_does_not_contain("[RUNNING] [..]myexm1-[..]")
        .with_stderr_contains(format!(
            "[RUNNING] [..]target/{}/debug/examples/myexm2-[..]",
            rustc_host()
        ))
        .run();

    // Only tests myexm2.
    prj.cargo("test --tests")
        .with_stderr_does_not_contain("[RUNNING] [..]myexm1-[..]")
        .with_stderr_contains(format!(
            "[RUNNING] [..]target/{}/debug/examples/myexm2-[..]",
            rustc_host()
        ))
        .run();

    // Tests all examples.
    prj.cargo("test --examples")
        .with_stderr_contains(format!(
            "[RUNNING] [..]target/{}/debug/examples/myexm1-[..]",
            rustc_host()
        ))
        .with_stderr_contains(format!(
            "[RUNNING] [..]target/{}/debug/examples/myexm2-[..]",
            rustc_host()
        ))
        .run();

    // Test an example, even without `test` set.
    prj.cargo("test --example myexm1")
        .with_stderr_contains(format!(
            "[RUNNING] [..]target/{}/debug/examples/myexm1-[..]",
            rustc_host()
        ))
        .run();

    // Tests all examples.
    prj.cargo("test --all-targets")
        .with_stderr_contains(format!(
            "[RUNNING] [..]target/{}/debug/examples/myexm1-[..]",
            rustc_host()
        ))
        .with_stderr_contains(format!(
            "[RUNNING] [..]target/{}/debug/examples/myexm2-[..]",
            rustc_host()
        ))
        .run();
}

#[cargo_test]
fn test_filtered_excludes_compiling_examples() {
    let p = project()
        .file(
            "src/lib.rs",
            "#[cfg(test)] mod tests { #[test] fn foo() { assert!(true); } }",
        )
        .file("examples/ex1.rs", "fn main() {}")
        .build();

    p.cargo("test -v foo")
        .with_stdout(
            "
running 1 test
test tests::foo ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out[..]

",
        )
        .with_stderr(format!(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `rustc --crate-name foo src/lib.rs [..] --test [..]`
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[CWD]/target/{}/debug/deps/foo-[..] foo`
",
            rustc_host()
        ))
        .with_stderr_does_not_contain("[RUNNING][..]rustc[..]ex1[..]")
        .run();
}

#[cargo_test]
fn test_no_harness() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [[bin]]
                name = "foo"
                test = false

                [[test]]
                name = "bar"
                path = "foo.rs"
                harness = false
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("foo.rs", "fn main() {}")
        .build();

    p.cargo("test -- --nocapture")
        .with_stderr(format!(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/{}/debug/deps/bar-[..][EXE])
",
            rustc_host()
        ))
        .run();
}

#[cargo_test]
fn selective_testing() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies.d1]
                    path = "d1"
                [dependencies.d2]
                    path = "d2"

                [lib]
                    name = "foo"
                    doctest = false
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "d1/Cargo.toml",
            r#"
                [package]
                name = "d1"
                version = "0.0.1"
                authors = []

                [lib]
                    name = "d1"
                    doctest = false
            "#,
        )
        .file("d1/src/lib.rs", "")
        .file(
            "d1/src/main.rs",
            "#[allow(unused_extern_crates)] extern crate d1; fn main() {}",
        )
        .file(
            "d2/Cargo.toml",
            r#"
                [package]
                name = "d2"
                version = "0.0.1"
                authors = []

                [lib]
                    name = "d2"
                    doctest = false
            "#,
        )
        .file("d2/src/lib.rs", "")
        .file(
            "d2/src/main.rs",
            "#[allow(unused_extern_crates)] extern crate d2; fn main() {}",
        );
    let p = p.build();

    println!("d1");
    p.cargo("test -p d1")
        .with_stderr(format!(
            "\
[COMPILING] d1 v0.0.1 ([CWD]/d1)
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/{target}/debug/deps/d1-[..][EXE])
[RUNNING] [..] (target/{target}/debug/deps/d1-[..][EXE])",
            target = rustc_host()
        ))
        .with_stdout_contains_n("running 0 tests", 2)
        .run();

    println!("d2");
    p.cargo("test -p d2")
        .with_stderr(format!(
            "\
[COMPILING] d2 v0.0.1 ([CWD]/d2)
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/{target}/debug/deps/d2-[..][EXE])
[RUNNING] [..] (target/{target}/debug/deps/d2-[..][EXE])",
            target = rustc_host()
        ))
        .with_stdout_contains_n("running 0 tests", 2)
        .run();

    println!("whole");
    p.cargo("test")
        .with_stderr(format!(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/{}/debug/deps/foo-[..][EXE])",
            rustc_host()
        ))
        .with_stdout_contains("running 0 tests")
        .run();
}

#[cargo_test]
fn almost_cyclic_but_not_quite() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dev-dependencies.b]
                path = "b"
                [dev-dependencies.c]
                path = "c"
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                #[cfg(test)] extern crate b;
                #[cfg(test)] extern crate c;
            "#,
        )
        .file(
            "b/Cargo.toml",
            r#"
                [package]
                name = "b"
                version = "0.0.1"
                authors = []

                [dependencies.foo]
                path = ".."
            "#,
        )
        .file(
            "b/src/lib.rs",
            r#"
                #[allow(unused_extern_crates)]
                extern crate foo;
            "#,
        )
        .file("c/Cargo.toml", &basic_manifest("c", "0.0.1"))
        .file("c/src/lib.rs", "")
        .build();

    p.cargo("build").run();
    p.cargo("test").run();
}

#[cargo_test]
fn build_then_selective_test() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies.b]
                path = "b"
            "#,
        )
        .file(
            "src/lib.rs",
            "#[allow(unused_extern_crates)] extern crate b;",
        )
        .file(
            "src/main.rs",
            r#"
                #[allow(unused_extern_crates)]
                extern crate b;
                #[allow(unused_extern_crates)]
                extern crate foo;
                fn main() {}
            "#,
        )
        .file("b/Cargo.toml", &basic_manifest("b", "0.0.1"))
        .file("b/src/lib.rs", "")
        .build();

    p.cargo("build").run();
    p.root().move_into_the_past();
    p.cargo("test -p b").run();
}

#[cargo_test]
fn example_dev_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dev-dependencies.bar]
                path = "bar"
            "#,
        )
        .file("src/lib.rs", "")
        .file("examples/e1.rs", "extern crate bar; fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file(
            "bar/src/lib.rs",
            r#"
                // make sure this file takes awhile to compile
                macro_rules! f0( () => (1) );
                macro_rules! f1( () => ({(f0!()) + (f0!())}) );
                macro_rules! f2( () => ({(f1!()) + (f1!())}) );
                macro_rules! f3( () => ({(f2!()) + (f2!())}) );
                macro_rules! f4( () => ({(f3!()) + (f3!())}) );
                macro_rules! f5( () => ({(f4!()) + (f4!())}) );
                macro_rules! f6( () => ({(f5!()) + (f5!())}) );
                macro_rules! f7( () => ({(f6!()) + (f6!())}) );
                macro_rules! f8( () => ({(f7!()) + (f7!())}) );
                pub fn bar() {
                    f8!();
                }
            "#,
        )
        .build();
    p.cargo("test").run();
    p.cargo("run --example e1 --release -v").run();
}

#[cargo_test]
fn selective_testing_with_docs() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies.d1]
                    path = "d1"
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                /// ```
                /// not valid rust
                /// ```
                pub fn foo() {}
            "#,
        )
        .file(
            "d1/Cargo.toml",
            r#"
                [package]
                name = "d1"
                version = "0.0.1"
                authors = []

                [lib]
                name = "d1"
                path = "d1.rs"
            "#,
        )
        .file("d1/d1.rs", "");
    let p = p.build();

    p.cargo("test -p d1")
        .with_stderr(format!(
            "\
[COMPILING] d1 v0.0.1 ([CWD]/d1)
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/{}/debug/deps/d1[..][EXE])
[DOCTEST] d1",
            rustc_host()
        ))
        .with_stdout_contains_n("running 0 tests", 2)
        .run();
}

#[cargo_test]
fn example_bin_same_name() {
    let p = project()
        .file("src/bin/foo.rs", r#"fn main() { println!("bin"); }"#)
        .file("examples/foo.rs", r#"fn main() { println!("example"); }"#)
        .build();

    p.cargo("test --no-run -v")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `rustc [..]`
[RUNNING] `rustc [..]`
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    assert!(!p.bin("foo").is_file());
    assert!(p.bin("examples/foo").is_file());

    p.process(&p.bin("examples/foo"))
        .with_stdout("example\n")
        .run();

    p.cargo("run")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..]",
        )
        .with_stdout("bin")
        .run();
    assert!(p.bin("foo").is_file());
}

#[cargo_test]
fn test_with_example_twice() {
    let p = project()
        .file("src/bin/foo.rs", r#"fn main() { println!("bin"); }"#)
        .file("examples/foo.rs", r#"fn main() { println!("example"); }"#)
        .build();

    println!("first");
    p.cargo("test -v").run();
    assert!(p.bin("examples/foo").is_file());
    println!("second");
    p.cargo("test -v").run();
    assert!(p.bin("examples/foo").is_file());
}

#[cargo_test]
fn example_with_dev_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [lib]
                name = "foo"
                test = false
                doctest = false

                [dev-dependencies.a]
                path = "a"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "examples/ex.rs",
            "#[allow(unused_extern_crates)] extern crate a; fn main() {}",
        )
        .file("a/Cargo.toml", &basic_manifest("a", "0.0.1"))
        .file("a/src/lib.rs", "")
        .build();

    p.cargo("test -v")
        .with_stderr(
            "\
[..]
[..]
[..]
[..]
[RUNNING] `rustc --crate-name ex [..] --extern a=[..]`
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn bin_is_preserved() {
    let p = project()
        .file("src/lib.rs", "")
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build -v").run();
    assert!(p.bin("foo").is_file());

    println!("test");
    p.cargo("test -v").run();
    assert!(p.bin("foo").is_file());
}

#[cargo_test]
fn bad_example() {
    let p = project().file("src/lib.rs", "");
    let p = p.build();

    p.cargo("run --example foo")
        .with_status(101)
        .with_stderr("[ERROR] no example target named `foo`")
        .run();
    p.cargo("run --bin foo")
        .with_status(101)
        .with_stderr("[ERROR] no bin target named `foo`")
        .run();
}

#[cargo_test]
fn doctest_feature() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                [features]
                bar = []
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                /// ```rust
                /// assert_eq!(foo::foo(), 1);
                /// ```
                #[cfg(feature = "bar")]
                pub fn foo() -> i32 { 1 }
            "#,
        )
        .build();

    p.cargo("test --features bar")
        .with_stderr(&format!(
            "\
[COMPILING] foo [..]
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/{}/debug/deps/foo[..][EXE])
[DOCTEST] foo",
            rustc_host()
        ))
        .with_stdout_contains("running 0 tests")
        .with_stdout_contains("test [..] ... ok")
        .run();
}

#[cargo_test]
fn dashes_to_underscores() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo-bar", "0.0.1"))
        .file(
            "src/lib.rs",
            r#"
                /// ```
                /// assert_eq!(foo_bar::foo(), 1);
                /// ```
                pub fn foo() -> i32 { 1 }
            "#,
        )
        .build();

    p.cargo("test -v").run();
}

#[cargo_test]
fn doctest_dev_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dev-dependencies]
                b = { path = "b" }
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                /// ```
                /// extern crate b;
                /// ```
                pub fn foo() {}
            "#,
        )
        .file("b/Cargo.toml", &basic_manifest("b", "0.0.1"))
        .file("b/src/lib.rs", "")
        .build();

    p.cargo("test -v").run();
}

#[cargo_test]
fn filter_no_doc_tests() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
                /// ```
                /// extern crate b;
                /// ```
                pub fn foo() {}
            "#,
        )
        .file("tests/foo.rs", "")
        .build();

    p.cargo("test --test=foo")
        .with_stderr(&format!(
            "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/{}/debug/deps/foo[..][EXE])",
            rustc_host()
        ))
        .with_stdout_contains("running 0 tests")
        .run();
}

#[cargo_test]
fn dylib_doctest() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [lib]
                name = "foo"
                crate-type = ["rlib", "dylib"]
                test = false
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                /// ```
                /// foo::foo();
                /// ```
                pub fn foo() {}
            "#,
        )
        .build();

    p.cargo("test")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[DOCTEST] foo",
        )
        .with_stdout_contains("test [..] ... ok")
        .run();
}

#[cargo_test]
fn dylib_doctest2() {
    // Can't doc-test dylibs, as they're statically linked together.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [lib]
                name = "foo"
                crate-type = ["dylib"]
                test = false
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                /// ```
                /// foo::foo();
                /// ```
                pub fn foo() {}
            "#,
        )
        .build();

    p.cargo("test").with_stdout("").run();
}

#[cargo_test]
fn cyclic_dev_dep_doc_test() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dev-dependencies]
                bar = { path = "bar" }
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                //! ```
                //! extern crate bar;
                //! ```
            "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                authors = []

                [dependencies]
                foo = { path = ".." }
            "#,
        )
        .file(
            "bar/src/lib.rs",
            r#"
                #[allow(unused_extern_crates)]
                extern crate foo;
            "#,
        )
        .build();
    p.cargo("test")
        .with_stderr(&format!(
            "\
[COMPILING] foo v0.0.1 ([..])
[COMPILING] bar v0.0.1 ([..])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/{}/debug/deps/foo[..][EXE])
[DOCTEST] foo",
            rustc_host()
        ))
        .with_stdout_contains("running 0 tests")
        .with_stdout_contains("test [..] ... ok")
        .run();
}

#[cargo_test]
fn dev_dep_with_build_script() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dev-dependencies]
                bar = { path = "bar" }
            "#,
        )
        .file("src/lib.rs", "")
        .file("examples/foo.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                authors = []
                build = "build.rs"
            "#,
        )
        .file("bar/src/lib.rs", "")
        .file("bar/build.rs", "fn main() {}")
        .build();
    p.cargo("test").run();
}

#[cargo_test]
fn no_fail_fast() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
            pub fn add_one(x: i32) -> i32{
                x + 1
            }

            /// ```rust
            /// use foo::sub_one;
            /// assert_eq!(sub_one(101), 100);
            /// ```
            pub fn sub_one(x: i32) -> i32{
                x - 1
            }
            "#,
        )
        .file(
            "tests/test_add_one.rs",
            r#"
            extern crate foo;
            use foo::*;

            #[test]
            fn add_one_test() {
                assert_eq!(add_one(1), 2);
            }

            #[test]
            fn fail_add_one_test() {
                assert_eq!(add_one(1), 1);
            }
            "#,
        )
        .file(
            "tests/test_sub_one.rs",
            r#"
            extern crate foo;
            use foo::*;

            #[test]
            fn sub_one_test() {
                assert_eq!(sub_one(1), 0);
            }
            "#,
        )
        .build();
    p.cargo("test --no-fail-fast")
        .with_status(101)
        .with_stderr_contains(format!(
            "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/{target}/debug/deps/foo-[..][EXE])
[RUNNING] [..] (target/{target}/debug/deps/test_add_one-[..][EXE])",
            target = rustc_host()
        ))
        .with_stdout_contains("running 0 tests")
        .with_stderr_contains(format!(
            "\
[RUNNING] [..] (target/{}/debug/deps/test_sub_one-[..][EXE])
[DOCTEST] foo",
            rustc_host()
        ))
        .with_stdout_contains("test result: FAILED. [..]")
        .with_stdout_contains("test sub_one_test ... ok")
        .with_stdout_contains_n("test [..] ... ok", 3)
        .run();
}

#[cargo_test]
fn test_multiple_packages() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies.d1]
                    path = "d1"
                [dependencies.d2]
                    path = "d2"

                [lib]
                    name = "foo"
                    doctest = false
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "d1/Cargo.toml",
            r#"
                [package]
                name = "d1"
                version = "0.0.1"
                authors = []

                [lib]
                    name = "d1"
                    doctest = false
            "#,
        )
        .file("d1/src/lib.rs", "")
        .file(
            "d2/Cargo.toml",
            r#"
                [package]
                name = "d2"
                version = "0.0.1"
                authors = []

                [lib]
                    name = "d2"
                    doctest = false
            "#,
        )
        .file("d2/src/lib.rs", "");
    let p = p.build();

    p.cargo("test -p d1 -p d2")
        .with_stderr_contains(format!(
            "[RUNNING] [..] (target/{}/debug/deps/d1-[..][EXE])",
            rustc_host()
        ))
        .with_stderr_contains(format!(
            "[RUNNING] [..] (target/{}/debug/deps/d2-[..][EXE])",
            rustc_host()
        ))
        .with_stdout_contains_n("running 0 tests", 2)
        .run();
}

#[cargo_test]
fn bin_does_not_rebuild_tests() {
    let p = project()
        .file("src/lib.rs", "")
        .file("src/main.rs", "fn main() {}")
        .file("tests/foo.rs", "");
    let p = p.build();

    p.cargo("test -v").run();

    sleep_ms(1000);
    fs::write(p.root().join("src/main.rs"), "fn main() { 3; }").unwrap();

    p.cargo("test -v --no-run")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..] src/main.rs [..]`
[RUNNING] `rustc [..] src/main.rs [..]`
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn selective_test_wonky_profile() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [profile.release]
                opt-level = 2

                [dependencies]
                a = { path = "a" }
            "#,
        )
        .file("src/lib.rs", "")
        .file("a/Cargo.toml", &basic_manifest("a", "0.0.1"))
        .file("a/src/lib.rs", "");
    let p = p.build();

    p.cargo("test -v --no-run --release -p foo -p a").run();
}

#[cargo_test]
fn selective_test_optional_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                a = { path = "a", optional = true }
            "#,
        )
        .file("src/lib.rs", "")
        .file("a/Cargo.toml", &basic_manifest("a", "0.0.1"))
        .file("a/src/lib.rs", "");
    let p = p.build();

    p.cargo("test -v --no-run --features a -p a")
        .with_stderr(
            "\
[COMPILING] a v0.0.1 ([..])
[RUNNING] `rustc [..] a/src/lib.rs [..]`
[RUNNING] `rustc [..] a/src/lib.rs [..]`
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn only_test_docs() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
                #[test]
                fn foo() {
                    let a: u32 = "hello";
                }

                /// ```
                /// foo::bar();
                /// println!("ok");
                /// ```
                pub fn bar() {
                }
            "#,
        )
        .file("tests/foo.rs", "this is not rust");
    let p = p.build();

    p.cargo("test --doc")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[DOCTEST] foo",
        )
        .with_stdout_contains("test [..] ... ok")
        .run();
}

#[cargo_test]
fn test_panic_abort_with_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                bar = { path = "bar" }

                [profile.dev]
                panic = 'abort'
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                extern crate bar;

                #[test]
                fn foo() {}
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "")
        .build();
    p.cargo("test -v").run();
}

#[cargo_test]
fn cfg_test_even_with_no_harness() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [lib]
                harness = false
                doctest = false
            "#,
        )
        .file(
            "src/lib.rs",
            r#"#[cfg(test)] fn main() { println!("hello!"); }"#,
        )
        .build();
    p.cargo("test -v")
        .with_stdout("hello!\n")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..]`
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[..]`
",
        )
        .run();
}

#[cargo_test]
fn panic_abort_multiple() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                a = { path = "a" }

                [profile.release]
                panic = 'abort'
            "#,
        )
        .file(
            "src/lib.rs",
            "#[allow(unused_extern_crates)] extern crate a;",
        )
        .file("a/Cargo.toml", &basic_manifest("a", "0.0.1"))
        .file("a/src/lib.rs", "")
        .build();
    p.cargo("test --release -v -p foo -p a").run();
}

#[cargo_test]
fn pass_correct_cfgs_flags_to_rustdoc() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [features]
                default = ["feature_a/default"]
                nightly = ["feature_a/nightly"]

                [dependencies.feature_a]
                path = "libs/feature_a"
                default-features = false
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                #[cfg(test)]
                mod tests {
                    #[test]
                    fn it_works() {
                      assert!(true);
                    }
                }
            "#,
        )
        .file(
            "libs/feature_a/Cargo.toml",
            r#"
                [package]
                name = "feature_a"
                version = "0.1.0"
                authors = []

                [features]
                default = ["mock_serde_codegen"]
                nightly = ["mock_serde_derive"]

                [dependencies]
                mock_serde_derive = { path = "../mock_serde_derive", optional = true }

                [build-dependencies]
                mock_serde_codegen = { path = "../mock_serde_codegen", optional = true }
            "#,
        )
        .file(
            "libs/feature_a/src/lib.rs",
            r#"
                #[cfg(feature = "mock_serde_derive")]
                const MSG: &'static str = "This is safe";

                #[cfg(feature = "mock_serde_codegen")]
                const MSG: &'static str = "This is risky";

                pub fn get() -> &'static str {
                    MSG
                }
            "#,
        )
        .file(
            "libs/mock_serde_derive/Cargo.toml",
            &basic_manifest("mock_serde_derive", "0.1.0"),
        )
        .file("libs/mock_serde_derive/src/lib.rs", "")
        .file(
            "libs/mock_serde_codegen/Cargo.toml",
            &basic_manifest("mock_serde_codegen", "0.1.0"),
        )
        .file("libs/mock_serde_codegen/src/lib.rs", "");
    let p = p.build();

    p.cargo("test --package feature_a --verbose")
        .with_stderr_contains(
            "\
[DOCTEST] feature_a
[RUNNING] `rustdoc [..]--test [..]mock_serde_codegen[..]`",
        )
        .run();

    p.cargo("test --verbose")
        .with_stderr_contains(
            "\
[DOCTEST] foo
[RUNNING] `rustdoc [..]--test [..]feature_a[..]`",
        )
        .run();
}

#[cargo_test]
fn test_release_ignore_panic() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                a = { path = "a" }

                [profile.test]
                panic = 'abort'
                [profile.release]
                panic = 'abort'
            "#,
        )
        .file(
            "src/lib.rs",
            "#[allow(unused_extern_crates)] extern crate a;",
        )
        .file("a/Cargo.toml", &basic_manifest("a", "0.0.1"))
        .file("a/src/lib.rs", "");
    let p = p.build();
    println!("test");
    p.cargo("test -v").run();
    println!("bench");
    p.cargo("bench -v").run();
}

#[cargo_test]
fn test_many_with_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                a = { path = "a" }

                [features]
                foo = []

                [workspace]
            "#,
        )
        .file("src/lib.rs", "")
        .file("a/Cargo.toml", &basic_manifest("a", "0.0.1"))
        .file("a/src/lib.rs", "")
        .build();

    p.cargo("test -v -p a -p foo --features foo").run();
}

#[cargo_test]
fn test_all_workspace() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = { path = "bar" }

                [workspace]
            "#,
        )
        .file("src/main.rs", "#[test] fn foo_test() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "#[test] fn bar_test() {}")
        .build();

    p.cargo("test --workspace")
        .with_stdout_contains("test foo_test ... ok")
        .with_stdout_contains("test bar_test ... ok")
        .run();
}

#[cargo_test]
fn test_all_exclude() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.1.0"

                [workspace]
                members = ["bar", "baz"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "#[test] pub fn bar() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "#[test] pub fn baz() { assert!(false); }")
        .build();

    p.cargo("test --workspace --exclude baz")
        .with_stdout_contains(
            "running 1 test
test bar ... ok",
        )
        .run();
}

#[cargo_test]
fn test_all_exclude_not_found() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.1.0"

                [workspace]
                members = ["bar"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "#[test] pub fn bar() {}")
        .build();

    p.cargo("test --workspace --exclude baz")
        .with_stderr_contains("[WARNING] excluded package(s) `baz` not found in workspace [..]")
        .with_stdout_contains(
            "running 1 test
test bar ... ok",
        )
        .run();
}

#[cargo_test]
fn test_all_exclude_glob() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.1.0"

                [workspace]
                members = ["bar", "baz"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "#[test] pub fn bar() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "#[test] pub fn baz() { assert!(false); }")
        .build();

    p.cargo("test --workspace --exclude '*z'")
        .with_stdout_contains(
            "running 1 test
test bar ... ok",
        )
        .run();
}

#[cargo_test]
fn test_all_exclude_glob_not_found() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.1.0"

                [workspace]
                members = ["bar"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "#[test] pub fn bar() {}")
        .build();

    p.cargo("test --workspace --exclude '*z'")
        .with_stderr_contains(
            "[WARNING] excluded package pattern(s) `*z` not found in workspace [..]",
        )
        .with_stdout_contains(
            "running 1 test
test bar ... ok",
        )
        .run();
}

#[cargo_test]
fn test_all_exclude_broken_glob() {
    let p = project().file("src/main.rs", "fn main() {}").build();

    p.cargo("test --workspace --exclude '[*z'")
        .with_status(101)
        .with_stderr_contains("[ERROR] cannot build glob pattern from `[*z`")
        .run();
}

#[cargo_test]
fn test_all_virtual_manifest() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["a", "b"]
            "#,
        )
        .file("a/Cargo.toml", &basic_manifest("a", "0.1.0"))
        .file("a/src/lib.rs", "#[test] fn a() {}")
        .file("b/Cargo.toml", &basic_manifest("b", "0.1.0"))
        .file("b/src/lib.rs", "#[test] fn b() {}")
        .build();

    p.cargo("test --workspace")
        .with_stdout_contains("running 1 test\ntest a ... ok")
        .with_stdout_contains("running 1 test\ntest b ... ok")
        .run();
}

#[cargo_test]
fn test_virtual_manifest_all_implied() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["a", "b"]
            "#,
        )
        .file("a/Cargo.toml", &basic_manifest("a", "0.1.0"))
        .file("a/src/lib.rs", "#[test] fn a() {}")
        .file("b/Cargo.toml", &basic_manifest("b", "0.1.0"))
        .file("b/src/lib.rs", "#[test] fn b() {}")
        .build();

    p.cargo("test")
        .with_stdout_contains("running 1 test\ntest a ... ok")
        .with_stdout_contains("running 1 test\ntest b ... ok")
        .run();
}

#[cargo_test]
fn test_virtual_manifest_one_project() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar", "baz"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "#[test] fn bar() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "#[test] fn baz() { assert!(false); }")
        .build();

    p.cargo("test -p bar")
        .with_stdout_contains("running 1 test\ntest bar ... ok")
        .with_stdout_does_not_contain("running 1 test\ntest baz ... ok")
        .run();
}

#[cargo_test]
fn test_virtual_manifest_glob() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar", "baz"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "#[test] fn bar() { assert!(false); }")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "#[test] fn baz() {}")
        .build();

    p.cargo("test -p '*z'")
        .with_stdout_does_not_contain("running 1 test\ntest bar ... ok")
        .with_stdout_contains("running 1 test\ntest baz ... ok")
        .run();
}

#[cargo_test]
fn test_virtual_manifest_glob_not_found() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "#[test] fn bar() {}")
        .build();

    p.cargo("test -p bar -p '*z'")
        .with_status(101)
        .with_stderr("[ERROR] package pattern(s) `*z` not found in workspace [..]")
        .run();
}

#[cargo_test]
fn test_virtual_manifest_broken_glob() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "#[test] fn bar() {}")
        .build();

    p.cargo("test -p '[*z'")
        .with_status(101)
        .with_stderr_contains("[ERROR] cannot build glob pattern from `[*z`")
        .run();
}

#[cargo_test]
fn test_all_member_dependency_same_name() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["a"]
            "#,
        )
        .file(
            "a/Cargo.toml",
            r#"
                [project]
                name = "a"
                version = "0.1.0"

                [dependencies]
                a = "0.1.0"
            "#,
        )
        .file("a/src/lib.rs", "#[test] fn a() {}")
        .build();

    Package::new("a", "0.1.0").publish();

    p.cargo("test --workspace")
        .with_stdout_contains("test a ... ok")
        .run();
}

#[cargo_test]
fn doctest_only_with_dev_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "a"
                version = "0.1.0"

                [dev-dependencies]
                b = { path = "b" }
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                /// ```
                /// extern crate b;
                ///
                /// b::b();
                /// ```
                pub fn a() {}
            "#,
        )
        .file("b/Cargo.toml", &basic_manifest("b", "0.1.0"))
        .file("b/src/lib.rs", "pub fn b() {}")
        .build();

    p.cargo("test --doc -v").run();
}

#[cargo_test]
fn test_many_targets() {
    let p = project()
        .file(
            "src/bin/a.rs",
            r#"
                fn main() {}
                #[test] fn bin_a() {}
            "#,
        )
        .file(
            "src/bin/b.rs",
            r#"
                fn main() {}
                #[test] fn bin_b() {}
            "#,
        )
        .file(
            "src/bin/c.rs",
            r#"
                fn main() {}
                #[test] fn bin_c() { panic!(); }
            "#,
        )
        .file(
            "examples/a.rs",
            r#"
                fn main() {}
                #[test] fn example_a() {}
            "#,
        )
        .file(
            "examples/b.rs",
            r#"
                fn main() {}
                #[test] fn example_b() {}
            "#,
        )
        .file("examples/c.rs", "#[test] fn example_c() { panic!(); }")
        .file("tests/a.rs", "#[test] fn test_a() {}")
        .file("tests/b.rs", "#[test] fn test_b() {}")
        .file("tests/c.rs", "does not compile")
        .build();

    p.cargo("test --verbose --bin a --bin b --example a --example b --test a --test b")
        .with_stdout_contains("test bin_a ... ok")
        .with_stdout_contains("test bin_b ... ok")
        .with_stdout_contains("test test_a ... ok")
        .with_stdout_contains("test test_b ... ok")
        .with_stderr_contains("[RUNNING] `rustc --crate-name a examples/a.rs [..]`")
        .with_stderr_contains("[RUNNING] `rustc --crate-name b examples/b.rs [..]`")
        .run();
}

#[cargo_test]
fn doctest_and_registry() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "a"
                version = "0.1.0"

                [dependencies]
                b = { path = "b" }
                c = { path = "c" }

                [workspace]
            "#,
        )
        .file("src/lib.rs", "")
        .file("b/Cargo.toml", &basic_manifest("b", "0.1.0"))
        .file(
            "b/src/lib.rs",
            "
            /// ```
            /// b::foo();
            /// ```
            pub fn foo() {}
        ",
        )
        .file(
            "c/Cargo.toml",
            r#"
                [project]
                name = "c"
                version = "0.1.0"

                [dependencies]
                b = "0.1"
            "#,
        )
        .file("c/src/lib.rs", "")
        .build();

    Package::new("b", "0.1.0").publish();

    p.cargo("test --workspace -v").run();
}

#[cargo_test]
fn cargo_test_env() {
    let src = format!(
        r#"
        #![crate_type = "rlib"]

        #[test]
        fn env_test() {{
            use std::env;
            eprintln!("{{}}", env::var("{}").unwrap());
        }}
        "#,
        cargo::CARGO_ENV
    );

    let p = project()
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file("src/lib.rs", &src)
        .build();

    let cargo = cargo_exe().canonicalize().unwrap();
    p.cargo("test --lib -- --nocapture")
        .with_stderr_contains(cargo.to_str().unwrap())
        .with_stdout_contains("test env_test ... ok")
        .run();
}

#[cargo_test]
fn test_order() {
    let p = project()
        .file("src/lib.rs", "#[test] fn test_lib() {}")
        .file("tests/a.rs", "#[test] fn test_a() {}")
        .file("tests/z.rs", "#[test] fn test_z() {}")
        .build();

    p.cargo("test --workspace")
        .with_stdout_contains(
            "
running 1 test
test test_lib ... ok

test result: ok. [..]


running 1 test
test test_a ... ok

test result: ok. [..]


running 1 test
test test_z ... ok

test result: ok. [..]
",
        )
        .run();
}

#[cargo_test]
fn cyclic_dev() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.1.0"

                [dev-dependencies]
                foo = { path = "." }
            "#,
        )
        .file("src/lib.rs", "#[test] fn test_lib() {}")
        .file("tests/foo.rs", "extern crate foo;")
        .build();

    p.cargo("test --workspace").run();
}

#[cargo_test]
fn publish_a_crate_without_tests() {
    Package::new("testless", "0.1.0")
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "testless"
                version = "0.1.0"
                exclude = ["tests/*"]

                [[test]]
                name = "a_test"
            "#,
        )
        .file("src/lib.rs", "")
        // In real life, the package will have a test,
        // which would be excluded from .crate file by the
        // `exclude` field. Our test harness does not honor
        // exclude though, so let's just not add the file!
        // .file("tests/a_test.rs", "")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                testless = "0.1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("test").run();
    p.cargo("test --package testless").run();
}

#[cargo_test]
fn find_dependency_of_proc_macro_dependency_with_target() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["root", "proc_macro_dep"]
            "#,
        )
        .file(
            "root/Cargo.toml",
            r#"
                [project]
                name = "root"
                version = "0.1.0"
                authors = []

                [dependencies]
                proc_macro_dep = { path = "../proc_macro_dep" }
            "#,
        )
        .file(
            "root/src/lib.rs",
            r#"
                #[macro_use]
                extern crate proc_macro_dep;

                #[derive(Noop)]
                pub struct X;
            "#,
        )
        .file(
            "proc_macro_dep/Cargo.toml",
            r#"
                [project]
                name = "proc_macro_dep"
                version = "0.1.0"
                authors = []

                [lib]
                proc-macro = true

                [dependencies]
                baz = "^0.1"
            "#,
        )
        .file(
            "proc_macro_dep/src/lib.rs",
            r#"
                extern crate baz;
                extern crate proc_macro;
                use proc_macro::TokenStream;

                #[proc_macro_derive(Noop)]
                pub fn noop(_input: TokenStream) -> TokenStream {
                    "".parse().unwrap()
                }
            "#,
        )
        .build();
    Package::new("bar", "0.1.0").publish();
    Package::new("baz", "0.1.0")
        .dep("bar", "0.1")
        .file("src/lib.rs", "extern crate bar;")
        .publish();
    p.cargo("test --workspace --target").arg(rustc_host()).run();
}

#[cargo_test]
fn test_hint_not_masked_by_doctest() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
                /// ```
                /// assert_eq!(1, 1);
                /// ```
                pub fn this_works() {}
            "#,
        )
        .file(
            "tests/integ.rs",
            r#"
                #[test]
                fn this_fails() {
                    panic!();
                }
            "#,
        )
        .build();
    p.cargo("test --no-fail-fast")
        .with_status(101)
        .with_stdout_contains("test this_fails ... FAILED")
        .with_stdout_contains("[..]this_works (line [..]ok")
        .with_stderr_contains(
            "[ERROR] test failed, to rerun pass \
             '--test integ'",
        )
        .run();
}

#[cargo_test]
fn test_hint_workspace_virtual() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["a", "b"]
            "#,
        )
        .file("a/Cargo.toml", &basic_manifest("a", "0.1.0"))
        .file("a/src/lib.rs", "#[test] fn t1() {}")
        .file("b/Cargo.toml", &basic_manifest("b", "0.1.0"))
        .file("b/src/lib.rs", "#[test] fn t1() {assert!(false)}")
        .build();

    p.cargo("test")
        .with_stderr_contains("[ERROR] test failed, to rerun pass '-p b --lib'")
        .with_status(101)
        .run();
    p.cargo("test")
        .cwd("b")
        .with_stderr_contains("[ERROR] test failed, to rerun pass '--lib'")
        .with_status(101)
        .run();
}

#[cargo_test]
fn test_hint_workspace_nonvirtual() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [workspace]
            members = ["a"]
            "#,
        )
        .file("src/lib.rs", "")
        .file("a/Cargo.toml", &basic_manifest("a", "0.1.0"))
        .file("a/src/lib.rs", "#[test] fn t1() {assert!(false)}")
        .build();

    p.cargo("test --workspace")
        .with_stderr_contains("[ERROR] test failed, to rerun pass '-p a --lib'")
        .with_status(101)
        .run();
    p.cargo("test -p a")
        .with_stderr_contains("[ERROR] test failed, to rerun pass '-p a --lib'")
        .with_status(101)
        .run();
}

#[cargo_test]
fn json_artifact_includes_test_flag() {
    // Verify that the JSON artifact output includes `test` flag.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [profile.test]
                opt-level = 1
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("test --lib -v --message-format=json")
        .with_json(
            r#"
                {
                    "reason":"compiler-artifact",
                    "profile": {
                        "debug_assertions": true,
                        "debuginfo": 2,
                        "opt_level": "1",
                        "overflow_checks": true,
                        "test": true
                    },
                    "executable": "[..]/foo-[..]",
                    "features": [],
                    "package_id":"foo 0.0.1 ([..])",
                    "manifest_path": "[..]",
                    "target":{
                        "kind":["lib"],
                        "crate_types":["lib"],
                        "doc": true,
                        "doctest": true,
                        "edition": "2015",
                        "name":"foo",
                        "src_path":"[..]lib.rs",
                        "test": true
                    },
                    "filenames":"{...}",
                    "fresh": false
                }

                {"reason": "build-finished", "success": true}
            "#,
        )
        .run();
}

#[cargo_test]
fn json_artifact_includes_executable_for_library_tests() {
    let p = project()
        .file("src/main.rs", "fn main() { }")
        .file("src/lib.rs", r#"#[test] fn lib_test() {}"#)
        .build();

    p.cargo("test --lib -v --no-run --message-format=json")
        .with_json(
            r#"
                {
                    "executable": "[..]/foo/target/$TARGET/debug/deps/foo-[..][EXE]",
                    "features": [],
                    "filenames": "{...}",
                    "fresh": false,
                    "package_id": "foo 0.0.1 ([..])",
                    "manifest_path": "[..]",
                    "profile": "{...}",
                    "reason": "compiler-artifact",
                    "target": {
                        "crate_types": [ "lib" ],
                        "kind": [ "lib" ],
                        "doc": true,
                        "doctest": true,
                        "edition": "2015",
                        "name": "foo",
                        "src_path": "[..]/foo/src/lib.rs",
                        "test": true
                    }
                }

                {"reason": "build-finished", "success": true}
            "#
            .replace("$TARGET", rustc_host())
            .as_str(),
        )
        .run();
}

#[cargo_test]
fn json_artifact_includes_executable_for_integration_tests() {
    let p = project()
        .file(
            "tests/integration_test.rs",
            r#"#[test] fn integration_test() {}"#,
        )
        .build();

    p.cargo("test -v --no-run --message-format=json --test integration_test")
        .with_json(
            r#"
                {
                    "executable": "[..]/foo/target/$TARGET/debug/deps/integration_test-[..][EXE]",
                    "features": [],
                    "filenames": "{...}",
                    "fresh": false,
                    "package_id": "foo 0.0.1 ([..])",
                    "manifest_path": "[..]",
                    "profile": "{...}",
                    "reason": "compiler-artifact",
                    "target": {
                        "crate_types": [ "bin" ],
                        "kind": [ "test" ],
                        "doc": false,
                        "doctest": false,
                        "edition": "2015",
                        "name": "integration_test",
                        "src_path": "[..]/foo/tests/integration_test.rs",
                        "test": true
                    }
                }

                {"reason": "build-finished", "success": true}
            "#
            .replace("$TARGET", rustc_host())
            .as_str(),
        )
        .run();
}

#[cargo_test]
fn test_build_script_links() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                links = 'something'

                [lib]
                test = false
            "#,
        )
        .file("build.rs", "fn main() {}")
        .file("src/lib.rs", "")
        .build();

    p.cargo("test --no-run").run();
}

#[cargo_test]
fn doctest_skip_staticlib() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"

                [lib]
                crate-type = ["staticlib"]
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
            //! ```
            //! assert_eq!(1,2);
            //! ```
            "#,
        )
        .build();

    p.cargo("test --doc")
        .with_status(101)
        .with_stderr(
            "\
[WARNING] doc tests are not supported for crate type(s) `staticlib` in package `foo`
[ERROR] no library targets found in package `foo`",
        )
        .run();

    p.cargo("test")
        .with_stderr(&format!(
            "\
[COMPILING] foo [..]
[FINISHED] test [..]
[RUNNING] [..] (target/{}/debug/deps/foo-[..])",
            rustc_host()
        ))
        .run();
}

#[cargo_test]
fn can_not_mix_doc_tests_and_regular_tests() {
    let p = project()
        .file(
            "src/lib.rs",
            "\
/// ```
/// assert_eq!(1, 1)
/// ```
pub fn foo() -> u8 { 1 }

#[cfg(test)] mod tests {
    #[test] fn it_works() { assert_eq!(2 + 2, 4); }
}
",
        )
        .build();

    p.cargo("test")
        .with_stderr(&format!(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/{}/debug/deps/foo-[..])
[DOCTEST] foo
",
            rustc_host()
        ))
        .with_stdout(
            "
running 1 test
test tests::it_works ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out[..]


running 1 test
test src/lib.rs - foo (line 1) ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out[..]
\n",
        )
        .run();

    p.cargo("test --lib")
        .with_stderr(&format!(
            "\
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/{}/debug/deps/foo-[..])\n",
            rustc_host()
        ))
        .with_stdout(
            "
running 1 test
test tests::it_works ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out[..]
\n",
        )
        .run();

    // This has been modified to attempt to diagnose spurious errors on CI.
    // For some reason, this is recompiling the lib when it shouldn't. If the
    // root cause is ever found, the changes here should be reverted.
    // See https://github.com/rust-lang/cargo/issues/6887
    p.cargo("test --doc -vv")
        .with_stderr_does_not_contain("[COMPILING] foo [..]")
        .with_stderr_contains("[DOCTEST] foo")
        .with_stdout(
            "
running 1 test
test src/lib.rs - foo (line 1) ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out[..]

",
        )
        .env("CARGO_LOG", "cargo=trace")
        .run();

    p.cargo("test --lib --doc")
        .with_status(101)
        .with_stderr("[ERROR] Can't mix --doc with other target selecting options\n")
        .run();
}

#[cargo_test]
fn can_not_no_run_doc_tests() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
            /// ```
            /// let _x = 1 + "foo";
            /// ```
            pub fn foo() -> u8 { 1 }
            "#,
        )
        .build();

    p.cargo("test --doc --no-run")
        .with_status(101)
        .with_stderr("[ERROR] Can't skip running doc tests with --no-run")
        .run();
}

#[cargo_test]
fn test_all_targets_lib() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("test --all-targets")
        .with_stderr(
            "\
[COMPILING] foo [..]
[FINISHED] test [..]
[RUNNING] [..]foo[..]
",
        )
        .run();
}

#[cargo_test]
fn test_dep_with_dev() {
    Package::new("devdep", "0.1.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"

                [dependencies]
                bar = { path = "bar" }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"

                [dev-dependencies]
                devdep = "0.1"
            "#,
        )
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("test -p bar")
        .with_status(101)
        .with_stderr(
            "[ERROR] package `bar` cannot be tested because it requires dev-dependencies \
             and is not a member of the workspace",
        )
        .run();
}

#[cargo_test]
fn cargo_test_doctest_xcompile_ignores() {
    if !is_nightly() {
        // -Zdoctest-xcompile is unstable
        return;
    }
    // -Zdoctest-xcompile also enables --enable-per-target-ignores which
    // allows the ignore-TARGET syntax.
    let p = project()
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file(
            "src/lib.rs",
            r#"
            ///```ignore-x86_64
            ///assert!(cfg!(not(target_arch = "x86_64")));
            ///```
            pub fn foo() -> u8 {
                4
            }
            "#,
        )
        .build();

    p.cargo("build").run();
    #[cfg(not(target_arch = "x86_64"))]
    p.cargo("test")
        .with_stdout_contains(
            "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out[..]",
        )
        .run();
    #[cfg(target_arch = "x86_64")]
    p.cargo("test")
        .with_status(101)
        .with_stdout_contains(
            "test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out[..]",
        )
        .run();

    #[cfg(not(target_arch = "x86_64"))]
    p.cargo("test -Zdoctest-xcompile")
        .masquerade_as_nightly_cargo()
        .with_stdout_contains(
            "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out[..]",
        )
        .run();

    #[cfg(target_arch = "x86_64")]
    p.cargo("test -Zdoctest-xcompile")
        .masquerade_as_nightly_cargo()
        .with_stdout_contains(
            "test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out[..]",
        )
        .run();
}

#[cargo_test]
fn cargo_test_doctest_xcompile() {
    if !cross_compile::can_run_on_host() {
        return;
    }
    if !is_nightly() {
        // -Zdoctest-xcompile is unstable
        return;
    }
    let p = project()
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file(
            "src/lib.rs",
            r#"

            ///```
            ///assert!(1 == 1);
            ///```
            pub fn foo() -> u8 {
                4
            }
            "#,
        )
        .build();

    p.cargo("build").run();
    p.cargo(&format!("test --target {}", cross_compile::alternate()))
        .with_stdout_contains("running 0 tests")
        .run();
    p.cargo(&format!(
        "test --target {} -Zdoctest-xcompile",
        cross_compile::alternate()
    ))
    .masquerade_as_nightly_cargo()
    .with_stdout_contains(
        "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out[..]",
    )
    .run();
}

#[cargo_test]
fn cargo_test_doctest_xcompile_runner() {
    if !cross_compile::can_run_on_host() {
        return;
    }
    if !is_nightly() {
        // -Zdoctest-xcompile is unstable
        return;
    }

    let runner = project()
        .file("Cargo.toml", &basic_bin_manifest("runner"))
        .file(
            "src/main.rs",
            r#"
            pub fn main() {
                eprintln!("this is a runner");
                let args: Vec<String> = std::env::args().collect();
                std::process::Command::new(&args[1]).spawn();
            }
            "#,
        )
        .build();

    runner.cargo("build").run();
    assert!(runner.bin("runner").is_file());
    let runner_path = paths::root().join("runner");
    fs::copy(&runner.bin("runner"), &runner_path).unwrap();

    let config = paths::root().join(".cargo/config");

    fs::create_dir_all(config.parent().unwrap()).unwrap();
    // Escape Windows backslashes for TOML config.
    let runner_str = runner_path.to_str().unwrap().replace('\\', "\\\\");
    fs::write(
        config,
        format!(
            r#"
            [target.'cfg(target_arch = "{}")']
            runner = "{}"
            "#,
            cross_compile::alternate_arch(),
            runner_str
        ),
    )
    .unwrap();

    let p = project()
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file(
            "src/lib.rs",
            &format!(
                r#"
                ///```
                ///assert!(cfg!(target_arch = "{}"));
                ///```
                pub fn foo() -> u8 {{
                    4
                }}
                "#,
                cross_compile::alternate_arch()
            ),
        )
        .build();

    p.cargo("build").run();
    p.cargo(&format!("test --target {}", cross_compile::alternate()))
        .with_stdout_contains("running 0 tests")
        .run();
    p.cargo(&format!(
        "test --target {} -Zdoctest-xcompile",
        cross_compile::alternate()
    ))
    .masquerade_as_nightly_cargo()
    .with_stdout_contains(
        "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out[..]",
    )
    .with_stderr_contains("this is a runner")
    .run();
}

#[cargo_test]
fn cargo_test_doctest_xcompile_no_runner() {
    if !cross_compile::can_run_on_host() {
        return;
    }
    if !is_nightly() {
        // -Zdoctest-xcompile is unstable
        return;
    }

    let p = project()
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file(
            "src/lib.rs",
            &format!(
                r#"
                ///```
                ///assert!(cfg!(target_arch = "{}"));
                ///```
                pub fn foo() -> u8 {{
                    4
                }}
                "#,
                cross_compile::alternate_arch()
            ),
        )
        .build();

    p.cargo("build").run();
    p.cargo(&format!("test --target {}", cross_compile::alternate()))
        .with_stdout_contains("running 0 tests")
        .run();
    p.cargo(&format!(
        "test --target {} -Zdoctest-xcompile",
        cross_compile::alternate()
    ))
    .masquerade_as_nightly_cargo()
    .with_stdout_contains(
        "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out[..]",
    )
    .run();
}

#[cargo_test]
fn panic_abort_tests() {
    if !is_nightly() {
        // -Zpanic-abort-tests in rustc is unstable
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = 'foo'
                version = '0.1.0'

                [dependencies]
                a = { path = 'a' }

                [profile.dev]
                panic = 'abort'
                [profile.test]
                panic = 'abort'
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                #[test]
                fn foo() {
                    a::foo();
                }
            "#,
        )
        .file("a/Cargo.toml", &basic_lib_manifest("a"))
        .file("a/src/lib.rs", "pub fn foo() {}")
        .build();

    p.cargo("test -Z panic-abort-tests -v")
        .with_stderr_contains("[..]--crate-name a [..]-C panic=abort[..]")
        .with_stderr_contains("[..]--crate-name foo [..]-C panic=abort[..]")
        .with_stderr_contains("[..]--crate-name foo [..]-C panic=abort[..]--test[..]")
        .masquerade_as_nightly_cargo()
        .run();
}

#[cargo_test]
fn panic_abort_only_test() {
    if !is_nightly() {
        // -Zpanic-abort-tests in rustc is unstable
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = 'foo'
                version = '0.1.0'

                [dependencies]
                a = { path = 'a' }

                [profile.test]
                panic = 'abort'
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                #[test]
                fn foo() {
                    a::foo();
                }
            "#,
        )
        .file("a/Cargo.toml", &basic_lib_manifest("a"))
        .file("a/src/lib.rs", "pub fn foo() {}")
        .build();

    p.cargo("test -Z panic-abort-tests -v")
        .with_stderr_contains("warning: `panic` setting is ignored for `test` profile")
        .masquerade_as_nightly_cargo()
        .run();
}

#[cargo_test]
fn panic_abort_test_profile_inherits() {
    if !is_nightly() {
        // -Zpanic-abort-tests in rustc is unstable
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = 'foo'
                version = '0.1.0'

                [dependencies]
                a = { path = 'a' }

                [profile.dev]
                panic = 'abort'
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                #[test]
                fn foo() {
                    a::foo();
                }
            "#,
        )
        .file("a/Cargo.toml", &basic_lib_manifest("a"))
        .file("a/src/lib.rs", "pub fn foo() {}")
        .build();

    p.cargo("test -Z panic-abort-tests -v")
        .masquerade_as_nightly_cargo()
        .with_status(0)
        .run();
}

#[cargo_test]
fn bin_env_for_test() {
    // Test for the `CARGO_BIN_` environment variables for tests.
    //
    // Note: The Unicode binary uses a `[[bin]]` definition because different
    // filesystems normalize utf-8 in different ways. For example, HFS uses
    // "gru\u{308}ßen" and APFS uses "gr\u{fc}ßen". Defining it in TOML forces
    // one form to be used.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2018"

                [[bin]]
                name = 'grüßen'
                path = 'src/bin/grussen.rs'
            "#,
        )
        .file("src/bin/foo.rs", "fn main() {}")
        .file("src/bin/with-dash.rs", "fn main() {}")
        .file("src/bin/grussen.rs", "fn main() {}")
        .build();

    let bin_path = |name| p.bin(name).to_string_lossy().replace("\\", "\\\\");
    p.change_file(
        "tests/check_env.rs",
        &r#"
            #[test]
            fn run_bins() {
                assert_eq!(env!("CARGO_BIN_EXE_foo"), "<FOO_PATH>");
                assert_eq!(env!("CARGO_BIN_EXE_with-dash"), "<WITH_DASH_PATH>");
                assert_eq!(env!("CARGO_BIN_EXE_grüßen"), "<GRÜSSEN_PATH>");
            }
        "#
        .replace("<FOO_PATH>", &bin_path("foo"))
        .replace("<WITH_DASH_PATH>", &bin_path("with-dash"))
        .replace("<GRÜSSEN_PATH>", &bin_path("grüßen")),
    );

    p.cargo("test --test check_env").run();
    p.cargo("check --test check_env").run();
}

#[cargo_test]
fn test_workspaces_cwd() {
    // This tests that all the different test types are executed from the
    // crate directory (manifest_dir), and not from the workspace root.

    let make_lib_file = |expected| {
        format!(
            r#"
                //! ```
                //! assert_eq!("{expected}", std::fs::read_to_string("file.txt").unwrap());
                //! assert_eq!("{expected}", include_str!("../file.txt"));
                //! assert_eq!(
                //!     std::path::PathBuf::from(std::env!("CARGO_MANIFEST_DIR")),
                //!     std::env::current_dir().unwrap(),
                //! );
                //! ```

                #[test]
                fn test_unit_{expected}_cwd() {{
                    assert_eq!("{expected}", std::fs::read_to_string("file.txt").unwrap());
                    assert_eq!("{expected}", include_str!("../file.txt"));
                    assert_eq!(
                        std::path::PathBuf::from(std::env!("CARGO_MANIFEST_DIR")),
                        std::env::current_dir().unwrap(),
                    );
                }}
            "#,
            expected = expected
        )
    };
    let make_test_file = |expected| {
        format!(
            r#"
                #[test]
                fn test_integration_{expected}_cwd() {{
                    assert_eq!("{expected}", std::fs::read_to_string("file.txt").unwrap());
                    assert_eq!("{expected}", include_str!("../file.txt"));
                    assert_eq!(
                        std::path::PathBuf::from(std::env!("CARGO_MANIFEST_DIR")),
                        std::env::current_dir().unwrap(),
                    );
                }}
            "#,
            expected = expected
        )
    };

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "root-crate"
                version = "0.0.0"

                [workspace]
                members = [".", "nested-crate", "very/deeply/nested/deep-crate"]
            "#,
        )
        .file("file.txt", "root")
        .file("src/lib.rs", &make_lib_file("root"))
        .file("tests/integration.rs", &make_test_file("root"))
        .file(
            "nested-crate/Cargo.toml",
            r#"
                [package]
                name = "nested-crate"
                version = "0.0.0"
            "#,
        )
        .file("nested-crate/file.txt", "nested")
        .file("nested-crate/src/lib.rs", &make_lib_file("nested"))
        .file(
            "nested-crate/tests/integration.rs",
            &make_test_file("nested"),
        )
        .file(
            "very/deeply/nested/deep-crate/Cargo.toml",
            r#"
                [package]
                name = "deep-crate"
                version = "0.0.0"
            "#,
        )
        .file("very/deeply/nested/deep-crate/file.txt", "deep")
        .file(
            "very/deeply/nested/deep-crate/src/lib.rs",
            &make_lib_file("deep"),
        )
        .file(
            "very/deeply/nested/deep-crate/tests/integration.rs",
            &make_test_file("deep"),
        )
        .build();

    p.cargo("test --workspace --all")
        .with_stderr_contains("[DOCTEST] root-crate")
        .with_stderr_contains("[DOCTEST] nested-crate")
        .with_stderr_contains("[DOCTEST] deep-crate")
        .with_stdout_contains("test test_unit_root_cwd ... ok")
        .with_stdout_contains("test test_unit_nested_cwd ... ok")
        .with_stdout_contains("test test_unit_deep_cwd ... ok")
        .with_stdout_contains("test test_integration_root_cwd ... ok")
        .with_stdout_contains("test test_integration_nested_cwd ... ok")
        .with_stdout_contains("test test_integration_deep_cwd ... ok")
        .run();

    p.cargo("test -p root-crate --all")
        .with_stderr_contains("[DOCTEST] root-crate")
        .with_stdout_contains("test test_unit_root_cwd ... ok")
        .with_stdout_contains("test test_integration_root_cwd ... ok")
        .run();

    p.cargo("test -p nested-crate --all")
        .with_stderr_contains("[DOCTEST] nested-crate")
        .with_stdout_contains("test test_unit_nested_cwd ... ok")
        .with_stdout_contains("test test_integration_nested_cwd ... ok")
        .run();

    p.cargo("test -p deep-crate --all")
        .with_stderr_contains("[DOCTEST] deep-crate")
        .with_stdout_contains("test test_unit_deep_cwd ... ok")
        .with_stdout_contains("test test_integration_deep_cwd ... ok")
        .run();

    p.cargo("test --all")
        .cwd("nested-crate")
        .with_stderr_contains("[DOCTEST] nested-crate")
        .with_stdout_contains("test test_unit_nested_cwd ... ok")
        .with_stdout_contains("test test_integration_nested_cwd ... ok")
        .run();

    p.cargo("test --all")
        .cwd("very/deeply/nested/deep-crate")
        .with_stderr_contains("[DOCTEST] deep-crate")
        .with_stdout_contains("test test_unit_deep_cwd ... ok")
        .with_stdout_contains("test test_integration_deep_cwd ... ok")
        .run();
}
