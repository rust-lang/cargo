use std::fs::File;
use std::io::prelude::*;
use std::str;

use cargo;
use cargo::util::process;
use support::paths::CargoPathExt;
use support::registry::Package;
use support::{basic_manifest, basic_bin_manifest, basic_lib_manifest, cargo_exe, execs, project};
use support::{is_nightly, rustc_host, sleep_ms};
use support::hamcrest::{assert_that, existing_file, is_not};

#[test]
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
            }"#,
        )
        .build();

    assert_that(p.cargo("build"), execs());
    assert_that(&p.bin("foo"), existing_file());

    assert_that(
        process(&p.bin("foo")),
        execs().with_stdout("hello\n"),
    );

    assert_that(
        p.cargo("test"),
        execs()
            .with_stderr(format!(
                "\
[COMPILING] foo v0.5.0 ({})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/foo-[..][EXE]",
                p.url()
            ))
            .with_stdout_contains("test test_hello ... ok"),
    );
}

#[test]
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

    assert_that(
        p.cargo("test -v --release"),
        execs()
            .with_stderr(format!(
                "\
[COMPILING] bar v0.0.1 ({dir}/bar)
[RUNNING] [..] -C opt-level=3 [..]
[COMPILING] foo v0.1.0 ({dir})
[RUNNING] [..] -C opt-level=3 [..]
[RUNNING] [..] -C opt-level=3 [..]
[RUNNING] [..] -C opt-level=3 [..]
[FINISHED] release [optimized] target(s) in [..]
[RUNNING] `[..]target/release/deps/foo-[..][EXE]`
[RUNNING] `[..]target/release/deps/test-[..][EXE]`
[DOCTEST] foo
[RUNNING] `rustdoc --test [..]lib.rs[..]`",
                dir = p.url()
            ))
            .with_stdout_contains_n("test test ... ok", 2)
            .with_stdout_contains("running 0 tests"),
    );
}

#[test]
fn cargo_test_overflow_checks() {
    if !is_nightly() {
        return;
    }
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
                    [1, i32::max_value()].iter().sum::<i32>();
                });
                assert!(r.is_err());
            }"#,
        )
        .build();

    assert_that(p.cargo("build --release"), execs());
    assert_that(&p.release_bin("foo"), existing_file());

    assert_that(
        process(&p.release_bin("foo")),
        execs().with_stdout(""),
    );
}

#[test]
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

    assert_that(
        p.cargo("test -v hello"),
        execs()
            .with_stderr(format!(
                "\
[COMPILING] foo v0.5.0 ({url})
[RUNNING] `rustc [..] src/main.rs [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[..]target/debug/deps/foo-[..][EXE] hello`",
                url = p.url()
            ))
            .with_stdout_contains("test test_hello ... ok"),
    );
}

#[test]
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

    let output = p.cargo("test -v").exec_with_output().unwrap();
    let output = str::from_utf8(&output.stdout).unwrap();
    assert!(
        output.contains("test bin_test"),
        "bin_test missing\n{}",
        output
    );
    assert!(
        output.contains("test lib_test"),
        "lib_test missing\n{}",
        output
    );
    assert!(
        output.contains("test test_test"),
        "test_test missing\n{}",
        output
    );
}

#[test]
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
            }"#,
        )
        .build();

    assert_that(p.cargo("build"), execs());
    assert_that(&p.bin("foo"), existing_file());

    assert_that(
        process(&p.bin("foo")),
        execs().with_stdout("hello\n"),
    );

    assert_that(
        p.cargo("test"),
        execs()
            .with_stderr(format!(
                "\
[COMPILING] foo v0.5.0 ({url})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/foo-[..][EXE]
[ERROR] test failed, to rerun pass '--bin foo'",
                url = p.url()
            ))
            .with_stdout_contains(
                "
running 1 test
test test_hello ... FAILED

failures:

---- test_hello stdout ----
[..]thread 'test_hello' panicked at 'assertion failed:[..]",
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
            .with_status(101),
    );
}

#[test]
fn cargo_test_failing_test_in_test() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", r#"pub fn main() { println!("hello"); }"#)
        .file("tests/footest.rs", "#[test] fn test_hello() { assert!(false) }")
        .build();

    assert_that(p.cargo("build"), execs());
    assert_that(&p.bin("foo"), existing_file());

    assert_that(
        process(&p.bin("foo")),
        execs().with_stdout("hello\n"),
    );

    assert_that(
        p.cargo("test"),
        execs()
            .with_stderr(format!(
                "\
[COMPILING] foo v0.5.0 ({url})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/foo-[..][EXE]
[RUNNING] target/debug/deps/footest-[..][EXE]
[ERROR] test failed, to rerun pass '--test footest'",
                url = p.url()
            ))
            .with_stdout_contains("running 0 tests")
            .with_stdout_contains(
                "\
running 1 test
test test_hello ... FAILED

failures:

---- test_hello stdout ----
[..]thread 'test_hello' panicked at 'assertion failed: false', \
      tests/footest.rs:1[..]
",
            )
            .with_stdout_contains(
                "\
failures:
    test_hello
",
            )
            .with_status(101),
    );
}

#[test]
fn cargo_test_failing_test_in_lib() {
    let p = project()
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file("src/lib.rs", "#[test] fn test_hello() { assert!(false) }")
        .build();

    assert_that(
        p.cargo("test"),
        execs()
            .with_stderr(format!(
                "\
[COMPILING] foo v0.5.0 ({url})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/foo-[..][EXE]
[ERROR] test failed, to rerun pass '--lib'",
                url = p.url()
            ))
            .with_stdout_contains(
                "\
test test_hello ... FAILED

failures:

---- test_hello stdout ----
[..]thread 'test_hello' panicked at 'assertion failed: false', \
      src/lib.rs:1[..]
",
            )
            .with_stdout_contains(
                "\
failures:
    test_hello
",
            )
            .with_status(101),
    );
}

#[test]
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

    assert_that(
        p.cargo("test"),
        execs()
            .with_stderr(format!(
                "\
[COMPILING] foo v0.0.1 ({})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/foo-[..][EXE]
[RUNNING] target/debug/deps/baz-[..][EXE]
[DOCTEST] foo",
                p.url()
            ))
            .with_stdout_contains("test lib_test ... ok")
            .with_stdout_contains("test bin_test ... ok")
            .with_stdout_contains_n("test [..] ... ok", 3),
    );
}

#[test]
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
    let _p2 = project().at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("src/lib.rs", "pub fn bar() {} #[test] fn foo_test() {}")
        .build();

    assert_that(
        p.cargo("test"),
        execs()
            .with_stderr(&format!(
                "\
[COMPILING] bar v0.0.1 ([..])
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[..]
[DOCTEST] foo",
                dir = p.url()
            ))
            .with_stdout_contains("test bar_test ... ok")
            .with_stdout_contains_n("test [..] ... ok", 2),
    );
}

#[test]
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

    assert_that(
        p.cargo("test"),
        execs()
            .with_stderr(format!(
                "\
[COMPILING] foo v0.0.1 ({})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/foo-[..][EXE]
[RUNNING] target/debug/deps/test-[..][EXE]
[DOCTEST] foo",
                p.url()
            ))
            .with_stdout_contains("test internal_test ... ok")
            .with_stdout_contains("test external_test ... ok")
            .with_stdout_contains("running 0 tests"),
    );
}

#[test]
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

    assert_that(p.cargo("test"), execs())
}

#[test]
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

    assert_that(
        p.cargo("test"),
        execs()
            .with_stderr(format!(
                "\
[COMPILING] foo v0.0.1 ({})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/foo-[..][EXE]
[RUNNING] target/debug/deps/external-[..][EXE]
[DOCTEST] foo",
                p.url()
            ))
            .with_stdout_contains("test internal_test ... ok")
            .with_stdout_contains("test external_test ... ok")
            .with_stdout_contains("running 0 tests"),
    );
}

#[test]
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
    assert_that(p.cargo("test"), execs());
}

#[test]
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

    assert_that(
        p.cargo("test bar"),
        execs()
            .with_stderr(&format!(
                "\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/foo-[..][EXE]
[DOCTEST] foo",
                dir = p.url()
            ))
            .with_stdout_contains("test bar ... ok")
            .with_stdout_contains("running 0 tests"),
    );

    assert_that(
        p.cargo("test foo"),
        execs()
            .with_stderr(
                "\
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/foo-[..][EXE]
[DOCTEST] foo",
            )
            .with_stdout_contains("test foo ... ok")
            .with_stdout_contains("running 0 tests"),
    );
}

// Regression test for running cargo-test twice with
// tests in an rlib
#[test]
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

    p.cargo("build");

    for _ in 0..2 {
        assert_that(p.cargo("test"), execs());
    }
}

#[test]
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

    assert_that(
        p.cargo("test"),
        execs()
            .with_stderr(format!(
                "\
[COMPILING] foo v0.0.1 ({})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/foo-[..][EXE]
[RUNNING] target/debug/deps/foo-[..][EXE]
[DOCTEST] foo",
                p.url()
            ))
            .with_stdout_contains_n("test [..] ... ok", 2)
            .with_stdout_contains("running 0 tests"),
    );
}

#[test]
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

    assert_that(
        p.cargo("test"),
        execs()
            .with_stderr(&format!(
                "\
[COMPILING] syntax v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/syntax-[..][EXE]
[RUNNING] target/debug/deps/test-[..][EXE]
[DOCTEST] syntax",
                dir = p.url()
            ))
            .with_stdout_contains("test foo_test ... ok")
            .with_stdout_contains("test test ... ok")
            .with_stdout_contains_n("test [..] ... ok", 3),
    );
}

#[test]
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

    assert_that(
        p.cargo("test"),
        execs()
            .with_stderr(&format!(
                "\
[COMPILING] syntax v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/syntax-[..][EXE]",
                dir = p.url()
            ))
            .with_stdout_contains("test test ... ok"),
    );
}

#[test]
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

    assert_that(
        p.cargo("test"),
        execs()
            .with_stderr(&format!(
                "\
[COMPILING] syntax v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/syntax-[..][EXE]",
                dir = p.url()
            ))
            .with_stdout_contains("test test ... ok"),
    );
}

#[test]
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

    assert_that(
        p.cargo("test"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  binary target bin.name is required",
        ),
    );
}

#[test]
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

    assert_that(
        p.cargo("test"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  benchmark target bench.name is required",
        ),
    );
}

#[test]
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

    assert_that(
        p.cargo("test"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  test target test.name is required",
        ),
    );
}

#[test]
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

    assert_that(
        p.cargo("test"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  example target example.name is required",
        ),
    );
}

#[test]
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
            r#"
            use std::process::Command;
            #[test]
            fn test_test() {
                let status = Command::new("target/debug/foo").status().unwrap();
                assert_eq!(status.code(), Some(101));
            }
        "#,
        )
        .build();

    let output = p.cargo("test -v").exec_with_output().unwrap();
    let output = str::from_utf8(&output.stdout).unwrap();
    assert!(
        output.contains("main_test ... ok"),
        "no main_test\n{}",
        output
    );
    assert!(
        output.contains("test_test ... ok"),
        "no test_test\n{}",
        output
    );
}

#[test]
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

    assert_that(
        p.cargo("test"),
        execs()
            .with_stderr(&format!(
                "\
[COMPILING] bar v0.0.1 ({dir}/bar)
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/foo-[..][EXE]
[RUNNING] target/debug/deps/test-[..][EXE]",
                dir = p.url()
            ))
            .with_stdout_contains_n("test foo ... ok", 2),
    );

    p.root().move_into_the_past();
    assert_that(
        p.cargo("test"),
        execs()
            .with_stderr(
                "\
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/foo-[..][EXE]
[RUNNING] target/debug/deps/test-[..][EXE]",
            )
            .with_stdout_contains_n("test foo ... ok", 2),
    );
}

#[test]
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

    assert_that(
        p.cargo("test"),
        execs()
            .with_stderr(&format!(
                "\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/foo-[..][EXE]
[DOCTEST] foo",
                dir = p.url()
            ))
            .with_stdout_contains("test foo ... ok")
            .with_stdout_contains("running 0 tests"),
    );

    assert_that(
        p.cargo("test"),
        execs()
            .with_stderr(
                "\
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/foo-[..][EXE]
[DOCTEST] foo",
            )
            .with_stdout_contains("test foo ... ok")
            .with_stdout_contains("running 0 tests"),
    );
}

#[test]
fn test_then_build() {
    let p = project()
        .file("src/lib.rs", "#[test] fn foo() {}")
        .build();

    assert_that(
        p.cargo("test"),
        execs()
            .with_stderr(&format!(
                "\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/foo-[..][EXE]
[DOCTEST] foo",
                dir = p.url()
            ))
            .with_stdout_contains("test foo ... ok")
            .with_stdout_contains("running 0 tests"),
    );

    assert_that(p.cargo("build"), execs().with_stdout(""));
}

#[test]
fn test_no_run() {
    let p = project()
        .file("src/lib.rs", "#[test] fn foo() { panic!() }")
        .build();

    assert_that(
        p.cargo("test --no-run"),
        execs().with_stderr(&format!(
            "\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            dir = p.url()
        )),
    );
}

#[test]
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

    assert_that(
        prj.cargo("test --bin bin2"),
        execs()
            .with_stderr(format!(
                "\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/bin2-[..][EXE]",
                dir = prj.url()
            ))
            .with_stdout_contains("test test2 ... ok"),
    );
}

#[test]
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

    assert_that(
        prj.cargo("test --bins"),
        execs()
            .with_stderr(format!(
                "\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/mybin-[..][EXE]",
                dir = prj.url()
            ))
            .with_stdout_contains("test test_in_bin ... ok"),
    );
}

#[test]
fn test_run_specific_test_target() {
    let prj = project()
        .file("src/bin/a.rs", "fn main() { }")
        .file("src/bin/b.rs", "#[test] fn test_b() { } fn main() { }")
        .file("tests/a.rs", "#[test] fn test_a() { }")
        .file("tests/b.rs", "#[test] fn test_b() { }")
        .build();

    assert_that(
        prj.cargo("test --test b"),
        execs()
            .with_stderr(format!(
                "\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/b-[..][EXE]",
                dir = prj.url()
            ))
            .with_stdout_contains("test test_b ... ok"),
    );
}

#[test]
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

    assert_that(
        prj.cargo("test --tests"),
        execs()
            .with_stderr(format!(
                "\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/mybin-[..][EXE]
[RUNNING] target/debug/deps/mytest-[..][EXE]",
                dir = prj.url()
            ))
            .with_stdout_contains("test test_in_test ... ok"),
    );
}

#[test]
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

    assert_that(
        prj.cargo("test --benches"),
        execs()
            .with_stderr(format!(
                "\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/mybin-[..][EXE]
[RUNNING] target/debug/deps/mybench-[..][EXE]",
                dir = prj.url()
            ))
            .with_stdout_contains("test test_in_bench ... ok"),
    );
}

#[test]
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
    assert_that(
        prj.cargo("test -v"),
        execs()
            .with_stderr_contains("[RUNNING] `rustc [..]myexm1.rs --crate-type bin[..]")
            .with_stderr_contains("[RUNNING] `rustc [..]myexm2.rs [..]--test[..]")
            .with_stderr_does_not_contain("[RUNNING] [..]myexm1-[..]")
            .with_stderr_contains("[RUNNING] [..]target/debug/examples/myexm2-[..]"),
    );

    // Only tests myexm2.
    assert_that(
        prj.cargo("test --tests"),
        execs()
            .with_stderr_does_not_contain("[RUNNING] [..]myexm1-[..]")
            .with_stderr_contains("[RUNNING] [..]target/debug/examples/myexm2-[..]"),
    );

    // Tests all examples.
    assert_that(
        prj.cargo("test --examples"),
        execs()
            .with_stderr_contains("[RUNNING] [..]target/debug/examples/myexm1-[..]")
            .with_stderr_contains("[RUNNING] [..]target/debug/examples/myexm2-[..]"),
    );

    // Test an example, even without `test` set.
    assert_that(
        prj.cargo("test --example myexm1"),
        execs()
            .with_stderr_contains("[RUNNING] [..]target/debug/examples/myexm1-[..]"),
    );

    // Tests all examples.
    assert_that(
        prj.cargo("test --all-targets"),
        execs()
            .with_stderr_contains("[RUNNING] [..]target/debug/examples/myexm1-[..]")
            .with_stderr_contains("[RUNNING] [..]target/debug/examples/myexm2-[..]"),
    );
}

#[test]
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

    assert_that(
        p.cargo("test -- --nocapture"),
        execs().with_stderr(&format!(
            "\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/bar-[..][EXE]
",
            dir = p.url()
        )),
    );
}

#[test]
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
    assert_that(
        p.cargo("test -p d1"),
        execs()
            .with_stderr(&format!(
                "\
[COMPILING] d1 v0.0.1 ({dir}/d1)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/d1-[..][EXE]
[RUNNING] target/debug/deps/d1-[..][EXE]",
                dir = p.url()
            ))
            .with_stdout_contains_n("running 0 tests", 2),
    );

    println!("d2");
    assert_that(
        p.cargo("test -p d2"),
        execs()
            .with_stderr(&format!(
                "\
[COMPILING] d2 v0.0.1 ({dir}/d2)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/d2-[..][EXE]
[RUNNING] target/debug/deps/d2-[..][EXE]",
                dir = p.url()
            ))
            .with_stdout_contains_n("running 0 tests", 2),
    );

    println!("whole");
    assert_that(
        p.cargo("test"),
        execs()
            .with_stderr(&format!(
                "\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/foo-[..][EXE]",
                dir = p.url()
            ))
            .with_stdout_contains("running 0 tests"),
    );
}

#[test]
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

    assert_that(p.cargo("build"), execs());
    assert_that(p.cargo("test"), execs());
}

#[test]
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
        .file("src/lib.rs", "#[allow(unused_extern_crates)] extern crate b;")
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

    assert_that(p.cargo("build"), execs());
    p.root().move_into_the_past();
    assert_that(p.cargo("test -p b"), execs());
}

#[test]
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
    assert_that(p.cargo("test"), execs());
    assert_that(
        p.cargo("run --example e1 --release -v"),
        execs(),
    );
}

#[test]
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

    assert_that(
        p.cargo("test -p d1"),
        execs()
            .with_stderr(&format!(
                "\
[COMPILING] d1 v0.0.1 ({dir}/d1)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/d1[..][EXE]
[DOCTEST] d1",
                dir = p.url()
            ))
            .with_stdout_contains_n("running 0 tests", 2),
    );
}

#[test]
fn example_bin_same_name() {
    let p = project()
        .file("src/bin/foo.rs", r#"fn main() { println!("bin"); }"#)
        .file("examples/foo.rs", r#"fn main() { println!("example"); }"#)
        .build();

    assert_that(
        p.cargo("test --no-run -v"),
        execs().with_stderr(&format!(
            "\
[COMPILING] foo v0.0.1 ({dir})
[RUNNING] `rustc [..]`
[RUNNING] `rustc [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            dir = p.url()
        )),
    );

    assert_that(&p.bin("foo"), is_not(existing_file()));
    assert_that(&p.bin("examples/foo"), existing_file());

    assert_that(
        p.process(&p.bin("examples/foo")),
        execs().with_stdout("example\n"),
    );

    assert_that(
        p.cargo("run"),
        execs()
            .with_stderr(
                "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..]",
            )
            .with_stdout("bin"),
    );
    assert_that(&p.bin("foo"), existing_file());
}

#[test]
fn test_with_example_twice() {
    let p = project()
        .file("src/bin/foo.rs", r#"fn main() { println!("bin"); }"#)
        .file("examples/foo.rs", r#"fn main() { println!("example"); }"#)
        .build();

    println!("first");
    assert_that(p.cargo("test -v"), execs());
    assert_that(&p.bin("examples/foo"), existing_file());
    println!("second");
    assert_that(p.cargo("test -v"), execs());
    assert_that(&p.bin("examples/foo"), existing_file());
}

#[test]
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

    assert_that(
        p.cargo("test -v"),
        execs().with_stderr(
            "\
[..]
[..]
[..]
[..]
[RUNNING] `rustc --crate-name ex [..] --extern a=[..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ),
    );
}

#[test]
fn bin_is_preserved() {
    let p = project()
        .file("src/lib.rs", "")
        .file("src/main.rs", "fn main() {}")
        .build();

    assert_that(p.cargo("build -v"), execs());
    assert_that(&p.bin("foo"), existing_file());

    println!("testing");
    assert_that(p.cargo("test -v"), execs());
    assert_that(&p.bin("foo"), existing_file());
}

#[test]
fn bad_example() {
    let p = project()
        .file("src/lib.rs", "");
    let p = p.build();

    assert_that(
        p.cargo("run --example foo"),
        execs()
            .with_status(101)
            .with_stderr("[ERROR] no example target named `foo`"),
    );
    assert_that(
        p.cargo("run --bin foo"),
        execs()
            .with_status(101)
            .with_stderr("[ERROR] no bin target named `foo`"),
    );
}

#[test]
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

    assert_that(
        p.cargo("test --features bar"),
        execs()
            .with_stderr(
                "\
[COMPILING] foo [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/foo[..][EXE]
[DOCTEST] foo",
            )
            .with_stdout_contains("running 0 tests")
            .with_stdout_contains("test [..] ... ok"),
    );
}

#[test]
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

    assert_that(p.cargo("test -v"), execs());
}

#[test]
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

    assert_that(p.cargo("test -v"), execs());
}

#[test]
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

    assert_that(
        p.cargo("test --test=foo"),
        execs()
            .with_stderr(
                "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/foo[..][EXE]",
            )
            .with_stdout_contains("running 0 tests"),
    );
}

#[test]
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

    assert_that(
        p.cargo("test"),
        execs()
            .with_stderr(
                "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[DOCTEST] foo",
            )
            .with_stdout_contains("test [..] ... ok"),
    );
}

#[test]
fn dylib_doctest2() {
    // can't doctest dylibs as they're statically linked together
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

    assert_that(p.cargo("test"), execs().with_stdout(""));
}

#[test]
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
    assert_that(
        p.cargo("test"),
        execs()
            .with_stderr(
                "\
[COMPILING] foo v0.0.1 ([..])
[COMPILING] bar v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/foo[..][EXE]
[DOCTEST] foo",
            )
            .with_stdout_contains("running 0 tests")
            .with_stdout_contains("test [..] ... ok"),
    );
}

#[test]
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
    assert_that(p.cargo("test"), execs());
}

#[test]
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
    assert_that(
        p.cargo("test --no-fail-fast"),
        execs()
            .with_status(101)
            .with_stderr_contains(
                "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/foo-[..][EXE]
[RUNNING] target/debug/deps/test_add_one-[..][EXE]",
            )
            .with_stdout_contains("running 0 tests")
            .with_stderr_contains(
                "\
[RUNNING] target/debug/deps/test_sub_one-[..][EXE]
[DOCTEST] foo",
            )
            .with_stdout_contains("test result: FAILED. [..]")
            .with_stdout_contains("test sub_one_test ... ok")
            .with_stdout_contains_n("test [..] ... ok", 3),
    );
}

#[test]
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

    assert_that(
        p.cargo("test -p d1 -p d2"),
        execs()
            .with_stderr_contains("[RUNNING] target/debug/deps/d1-[..][EXE]")
            .with_stderr_contains("[RUNNING] target/debug/deps/d2-[..][EXE]")
            .with_stdout_contains_n("running 0 tests", 2),
    );
}

#[test]
fn bin_does_not_rebuild_tests() {
    let p = project()
        .file("src/lib.rs", "")
        .file("src/main.rs", "fn main() {}")
        .file("tests/foo.rs", "");
    let p = p.build();

    assert_that(p.cargo("test -v"), execs());

    sleep_ms(1000);
    File::create(&p.root().join("src/main.rs"))
        .unwrap()
        .write_all(b"fn main() { 3; }")
        .unwrap();

    assert_that(
        p.cargo("test -v --no-run"),
        execs().with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..] src/main.rs [..]`
[RUNNING] `rustc [..] src/main.rs [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ),
    );
}

#[test]
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

    assert_that(
        p.cargo("test -v --no-run --release -p foo -p a"),
        execs(),
    );
}

#[test]
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

    assert_that(
        p.cargo("test -v --no-run --features a -p a"),
        execs().with_stderr(
            "\
[COMPILING] a v0.0.1 ([..])
[RUNNING] `rustc [..] a/src/lib.rs [..]`
[RUNNING] `rustc [..] a/src/lib.rs [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ),
    );
}

#[test]
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

    assert_that(
        p.cargo("test --doc"),
        execs()
            .with_stderr(
                "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[DOCTEST] foo",
            )
            .with_stdout_contains("test [..] ... ok"),
    );
}

#[test]
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
    assert_that(p.cargo("test -v"), execs());
}

#[test]
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
        .file("src/lib.rs", r#"#[cfg(test)] fn main() { println!("hello!"); }"#)
        .build();
    assert_that(
        p.cargo("test -v"),
        execs().with_stdout("hello!\n").with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[..]`
",
        ),
    );
}

#[test]
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
    assert_that(
        p.cargo("test --release -v -p foo -p a"),
        execs(),
    );
}

#[test]
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
        .file("libs/mock_serde_derive/Cargo.toml", &basic_manifest("mock_serde_derive", "0.1.0"))
        .file("libs/mock_serde_derive/src/lib.rs", "")
        .file("libs/mock_serde_codegen/Cargo.toml", &basic_manifest("mock_serde_codegen", "0.1.0"))
        .file("libs/mock_serde_codegen/src/lib.rs", "");
    let p = p.build();

    assert_that(
        p.cargo("test --package feature_a --verbose"),
        execs().with_stderr_contains(
            "\
[DOCTEST] feature_a
[RUNNING] `rustdoc --test [..]mock_serde_codegen[..]`",
        ),
    );

    assert_that(
        p.cargo("test --verbose"),
        execs().with_stderr_contains(
            "\
[DOCTEST] foo
[RUNNING] `rustdoc --test [..]feature_a[..]`",
        ),
    );
}

#[test]
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
    assert_that(p.cargo("test -v"), execs());
    println!("bench");
    assert_that(p.cargo("bench -v"), execs());
}

#[test]
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

    assert_that(
        p.cargo("test -v -p a -p foo --features foo"),
        execs(),
    );
}

#[test]
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

    assert_that(
        p.cargo("test --all"),
        execs()
            .with_stdout_contains("test foo_test ... ok")
            .with_stdout_contains("test bar_test ... ok"),
    );
}

#[test]
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

    assert_that(
        p.cargo("test --all --exclude baz"),
        execs().with_stdout_contains(
            "running 1 test
test bar ... ok",
        ),
    );
}

#[test]
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

    assert_that(
        p.cargo("test --all"),
        execs()
            .with_stdout_contains("test a ... ok")
            .with_stdout_contains("test b ... ok"),
    );
}

#[test]
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

    assert_that(
        p.cargo("test"),
        execs()
            .with_stdout_contains("test a ... ok")
            .with_stdout_contains("test b ... ok"),
    );
}

#[test]
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

    assert_that(
        p.cargo("test --all"),
        execs().with_stdout_contains("test a ... ok"),
    );
}

#[test]
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

    assert_that(
        p.cargo("test --doc -v"),
        execs(),
    );
}

#[test]
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

    assert_that(
        p.cargo("test --verbose --bin a --bin b --example a --example b --test a --test b"),
        execs()
            .with_stdout_contains("test bin_a ... ok")
            .with_stdout_contains("test bin_b ... ok")
            .with_stdout_contains("test test_a ... ok")
            .with_stdout_contains("test test_b ... ok")
            .with_stderr_contains("[RUNNING] `rustc --crate-name a examples/a.rs [..]`")
            .with_stderr_contains("[RUNNING] `rustc --crate-name b examples/b.rs [..]`"),
    )
}

#[test]
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

    assert_that(
        p.cargo("test --all -v"),
        execs(),
    );
}

#[test]
fn cargo_test_env() {
    let src = format!(
        r#"
        #![crate_type = "rlib"]

        #[test]
        fn env_test() {{
            use std::env;
            println!("{{}}", env::var("{}").unwrap());
        }}
        "#,
        cargo::CARGO_ENV
    );

    let p = project()
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file("src/lib.rs", &src)
        .build();

    let cargo = cargo_exe().canonicalize().unwrap();
    assert_that(
        p.cargo("test --lib -- --nocapture"),
        execs().with_stdout_contains(format!(
            "\
{}
test env_test ... ok
",
            cargo.to_str().unwrap()
        )),
    );
}

#[test]
fn test_order() {
    let p = project()
        .file("src/lib.rs", "#[test] fn test_lib() {}")
        .file("tests/a.rs", "#[test] fn test_a() {}")
        .file("tests/z.rs", "#[test] fn test_z() {}")
        .build();

    assert_that(
        p.cargo("test --all"),
        execs().with_stdout_contains(
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
        ),
    );
}

#[test]
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

    assert_that(p.cargo("test --all"), execs());
}

#[test]
fn publish_a_crate_without_tests() {
    Package::new("testless", "0.1.0")
        .file("Cargo.toml", r#"
            [project]
            name = "testless"
            version = "0.1.0"
            exclude = ["tests/*"]

            [[test]]
            name = "a_test"
        "#)
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

    assert_that(p.cargo("test"), execs());
    assert_that(
        p.cargo("test --package testless"),
        execs(),
    );
}

#[test]
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
    assert_that(p.cargo("test --all --target").arg(rustc_host()), execs());
}

#[test]
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
    assert_that(
        p.cargo("test --no-fail-fast"),
        execs()
            .with_status(101)
            .with_stdout_contains("test this_fails ... FAILED")
            .with_stdout_contains("[..]this_works (line [..]ok")
            .with_stderr_contains(
                "[ERROR] test failed, to rerun pass \
                 '--test integ'",
            ),
    );
}

#[test]
fn test_hint_workspace() {
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

    assert_that(
        p.cargo("test"),
        execs()
            .with_stderr_contains("[ERROR] test failed, to rerun pass '-p b --lib'")
            .with_status(101),
    );
}

#[test]
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

    assert_that(
        p.cargo("test -v --message-format=json"),
        execs().with_json(
            r#"
    {
        "reason":"compiler-artifact",
        "profile": {
            "debug_assertions": true,
            "debuginfo": 2,
            "opt_level": "0",
            "overflow_checks": true,
            "test": false
        },
        "features": [],
        "package_id":"foo 0.0.1 ([..])",
        "target":{
            "kind":["lib"],
            "crate_types":["lib"],
            "edition": "2015",
            "name":"foo",
            "src_path":"[..]lib.rs"
        },
        "filenames":["[..].rlib"],
        "fresh": false
    }

    {
        "reason":"compiler-artifact",
        "profile": {
            "debug_assertions": true,
            "debuginfo": 2,
            "opt_level": "1",
            "overflow_checks": true,
            "test": true
        },
        "features": [],
        "package_id":"foo 0.0.1 ([..])",
        "target":{
            "kind":["lib"],
            "crate_types":["lib"],
            "edition": "2015",
            "name":"foo",
            "src_path":"[..]lib.rs"
        },
        "filenames":["[..]/foo-[..]"],
        "fresh": false
    }
"#,
        ),
    );
}

#[test]
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

    assert_that(
        p.cargo("test --no-run"),
        execs(),
    );
}

#[test]
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

    assert_that(
        p.cargo("test --doc"),
        execs().with_status(101).with_stderr(
            "\
[WARNING] doc tests are not supported for crate type(s) `staticlib` in package `foo`
[ERROR] no library targets found in package `foo`",
        ),
    );

    assert_that(
        p.cargo("test"),
        execs().with_stderr(
            "\
[COMPILING] foo [..]
[FINISHED] dev [..]
[RUNNING] target/debug/deps/foo-[..]",
        ),
    )
}
