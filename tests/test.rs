extern crate cargo;
extern crate cargotest;
extern crate hamcrest;

use std::fs::File;
use std::io::prelude::*;
use std::str;

use cargotest::{sleep_ms, is_nightly};
use cargotest::support::{project, execs, basic_bin_manifest, basic_lib_manifest, cargo_exe};
use cargotest::support::paths::CargoPathExt;
use cargotest::support::registry::Package;
use hamcrest::{assert_that, existing_file, is_not};
use cargo::util::process;

#[test]
fn cargo_test_simple() {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", r#"
            fn hello() -> &'static str {
                "hello"
            }

            pub fn main() {
                println!("{}", hello())
            }

            #[test]
            fn test_hello() {
                assert_eq!(hello(), "hello")
            }"#);

    assert_that(p.cargo_process("build"), execs().with_status(0));
    assert_that(&p.bin("foo"), existing_file());

    assert_that(process(&p.bin("foo")),
                execs().with_status(0).with_stdout("hello\n"));

    assert_that(p.cargo("test"),
                execs().with_status(0).with_stderr(format!("\
[COMPILING] foo v0.5.0 ({})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[/]debug[/]deps[/]foo-[..][EXE]", p.url()))
                       .with_stdout_contains("test test_hello ... ok"));
}

#[test]
fn cargo_test_release() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            authors = []
            version = "0.1.0"

            [dependencies]
            bar = { path = "bar" }
        "#)
        .file("src/lib.rs", r#"
            extern crate bar;
            pub fn foo() { bar::bar(); }

            #[test]
            fn test() { foo(); }
        "#)
        .file("tests/test.rs", r#"
            extern crate foo;

            #[test]
            fn test() { foo::foo(); }
        "#)
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []
        "#)
        .file("bar/src/lib.rs", "pub fn bar() {}");

    assert_that(p.cargo_process("test").arg("-v").arg("--release"),
                execs().with_status(0).with_stderr(format!("\
[COMPILING] bar v0.0.1 ({dir}/bar)
[RUNNING] [..] -C opt-level=3 [..]
[COMPILING] foo v0.1.0 ({dir})
[RUNNING] [..] -C opt-level=3 [..]
[RUNNING] [..] -C opt-level=3 [..]
[RUNNING] [..] -C opt-level=3 [..]
[FINISHED] release [optimized] target(s) in [..]
[RUNNING] `[..]target[/]release[/]deps[/]foo-[..][EXE]`
[RUNNING] `[..]target[/]release[/]deps[/]test-[..][EXE]`
[DOCTEST] foo
[RUNNING] `rustdoc --test [..]lib.rs[..]`", dir = p.url()))
                       .with_stdout_contains_n("test test ... ok", 2)
                       .with_stdout_contains("running 0 tests"));
}

#[test]
fn cargo_test_overflow_checks() {
    if !is_nightly() {
        return;
    }
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.5.0"
            authors = []

            [[bin]]
            name = "foo"

            [profile.release]
            overflow-checks = true
            "#)
        .file("src/foo.rs", r#"
            use std::panic;
            pub fn main() {
                let r = panic::catch_unwind(|| {
                    [1, i32::max_value()].iter().sum::<i32>();
                });
                assert!(r.is_err());
            }"#);

    assert_that(p.cargo_process("build").arg("--release"),
                execs().with_status(0));
    assert_that(&p.release_bin("foo"), existing_file());

    assert_that(process(&p.release_bin("foo")),
                execs().with_status(0).with_stdout(""));
}

#[test]
fn cargo_test_verbose() {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", r#"
            fn main() {}
            #[test] fn test_hello() {}
        "#);

    assert_that(p.cargo_process("test").arg("-v").arg("hello"),
                execs().with_status(0).with_stderr(format!("\
[COMPILING] foo v0.5.0 ({url})
[RUNNING] `rustc [..] src[/]foo.rs [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[..]target[/]debug[/]deps[/]foo-[..][EXE] hello`", url = p.url()))
                       .with_stdout_contains("test test_hello ... ok"));
}

#[test]
fn many_similar_names() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "
            pub fn foo() {}
            #[test] fn lib_test() {}
        ")
        .file("src/main.rs", "
            extern crate foo;
            fn main() {}
            #[test] fn bin_test() { foo::foo() }
        ")
        .file("tests/foo.rs", r#"
            extern crate foo;
            #[test] fn test_test() { foo::foo() }
        "#);

    let output = p.cargo_process("test").arg("-v").exec_with_output().unwrap();
    let output = str::from_utf8(&output.stdout).unwrap();
    assert!(output.contains("test bin_test"), "bin_test missing\n{}", output);
    assert!(output.contains("test lib_test"), "lib_test missing\n{}", output);
    assert!(output.contains("test test_test"), "test_test missing\n{}", output);
}

#[test]
fn cargo_test_failing_test_in_bin() {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", r#"
            fn hello() -> &'static str {
                "hello"
            }

            pub fn main() {
                println!("{}", hello())
            }

            #[test]
            fn test_hello() {
                assert_eq!(hello(), "nope")
            }"#);

    assert_that(p.cargo_process("build"), execs().with_status(0));
    assert_that(&p.bin("foo"), existing_file());

    assert_that(process(&p.bin("foo")),
                execs().with_status(0).with_stdout("hello\n"));

    assert_that(p.cargo("test"),
                execs().with_stderr(format!("\
[COMPILING] foo v0.5.0 ({url})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[/]debug[/]deps[/]foo-[..][EXE]
[ERROR] test failed, to rerun pass '--bin foo'", url = p.url()))
                       .with_stdout_contains("
running 1 test
test test_hello ... FAILED

failures:

---- test_hello stdout ----
<tab>thread 'test_hello' panicked at 'assertion failed: \
    `(left == right)` (left: \
    `\"hello\"`, right: `\"nope\"`)', src[/]foo.rs:12
")
                       .with_stdout_contains("\
failures:
    test_hello
")
                       .with_status(101));
}

#[test]
fn cargo_test_failing_test_in_test() {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", r#"
            pub fn main() {
                println!("hello");
            }"#)
        .file("tests/footest.rs", r#"
            #[test]
            fn test_hello() {
                assert!(false)
            }"#);

    assert_that(p.cargo_process("build"), execs().with_status(0));
    assert_that(&p.bin("foo"), existing_file());

    assert_that(process(&p.bin("foo")),
                execs().with_status(0).with_stdout("hello\n"));

    assert_that(p.cargo("test"),
                execs().with_stderr(format!("\
[COMPILING] foo v0.5.0 ({url})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[/]debug[/]deps[/]foo-[..][EXE]
[RUNNING] target[/]debug[/]deps[/]footest-[..][EXE]
[ERROR] test failed, to rerun pass '--test footest'", url = p.url()))
                       .with_stdout_contains("running 0 tests")
                       .with_stdout_contains("\
running 1 test
test test_hello ... FAILED

failures:

---- test_hello stdout ----
<tab>thread 'test_hello' panicked at 'assertion failed: false', \
      tests[/]footest.rs:4
")
                       .with_stdout_contains("\
failures:
    test_hello
")
                       .with_status(101));
}

#[test]
fn cargo_test_failing_test_in_lib() {
    let p = project("foo")
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file("src/lib.rs", r#"
            #[test]
            fn test_hello() {
                assert!(false)
            }"#);

    assert_that(p.cargo_process("test"),
                execs().with_stderr(format!("\
[COMPILING] foo v0.5.0 ({url})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[/]debug[/]deps[/]foo-[..][EXE]
[ERROR] test failed, to rerun pass '--lib'", url = p.url()))
                       .with_stdout_contains("\
test test_hello ... FAILED

failures:

---- test_hello stdout ----
<tab>thread 'test_hello' panicked at 'assertion failed: false', \
      src[/]lib.rs:4
")
                       .with_stdout_contains("\
failures:
    test_hello
")
                       .with_status(101));
}


#[test]
fn test_with_lib_dep() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [[bin]]
            name = "baz"
            path = "src/main.rs"
        "#)
        .file("src/lib.rs", r#"
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
        "#)
        .file("src/main.rs", "
            extern crate foo;

            fn main() {}

            #[test]
            fn bin_test() {}
        ");

    assert_that(p.cargo_process("test"),
                execs().with_status(0).with_stderr(format!("\
[COMPILING] foo v0.0.1 ({})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[/]debug[/]deps[/]foo-[..][EXE]
[RUNNING] target[/]debug[/]deps[/]baz-[..][EXE]
[DOCTEST] foo", p.url()))
                       .with_stdout_contains("test lib_test ... ok")
                       .with_stdout_contains("test bin_test ... ok")
                       .with_stdout_contains_n("test [..] ... ok", 3));
}

#[test]
fn test_with_deep_lib_dep() {
    let p = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [dependencies.foo]
            path = "../foo"
        "#)
        .file("src/lib.rs", "
            extern crate foo;
            /// ```
            /// bar::bar();
            /// ```
            pub fn bar() {}

            #[test]
            fn bar_test() {
                foo::foo();
            }
        ");
    let p2 = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "
            pub fn foo() {}

            #[test]
            fn foo_test() {}
        ");

    p2.build();
    assert_that(p.cargo_process("test"),
                execs().with_status(0)
                       .with_stderr(&format!("\
[COMPILING] foo v0.0.1 ([..])
[COMPILING] bar v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[..]
[DOCTEST] bar", dir = p.url()))
                       .with_stdout_contains("test bar_test ... ok")
                       .with_stdout_contains_n("test [..] ... ok", 2));
}

#[test]
fn external_test_explicit() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [[test]]
            name = "test"
            path = "src/test.rs"
        "#)
        .file("src/lib.rs", r#"
            pub fn get_hello() -> &'static str { "Hello" }

            #[test]
            fn internal_test() {}
        "#)
        .file("src/test.rs", r#"
            extern crate foo;

            #[test]
            fn external_test() { assert_eq!(foo::get_hello(), "Hello") }
        "#);

    assert_that(p.cargo_process("test"),
                execs().with_status(0).with_stderr(format!("\
[COMPILING] foo v0.0.1 ({})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[/]debug[/]deps[/]foo-[..][EXE]
[RUNNING] target[/]debug[/]deps[/]test-[..][EXE]
[DOCTEST] foo", p.url()))
                       .with_stdout_contains("test internal_test ... ok")
                       .with_stdout_contains("test external_test ... ok")
                       .with_stdout_contains("running 0 tests"));
}

#[test]
fn external_test_named_test() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [[test]]
            name = "test"
        "#)
        .file("src/lib.rs", "")
        .file("tests/test.rs", r#"
            #[test]
            fn foo() { }
        "#);

    assert_that(p.cargo_process("test"),
                execs().with_status(0))
}

#[test]
fn external_test_implicit() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", r#"
            pub fn get_hello() -> &'static str { "Hello" }

            #[test]
            fn internal_test() {}
        "#)
        .file("tests/external.rs", r#"
            extern crate foo;

            #[test]
            fn external_test() { assert_eq!(foo::get_hello(), "Hello") }
        "#);

    assert_that(p.cargo_process("test"),
                execs().with_status(0).with_stderr(format!("\
[COMPILING] foo v0.0.1 ({})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[/]debug[/]deps[/]foo-[..][EXE]
[RUNNING] target[/]debug[/]deps[/]external-[..][EXE]
[DOCTEST] foo", p.url()))
                       .with_stdout_contains("test internal_test ... ok")
                       .with_stdout_contains("test external_test ... ok")
                       .with_stdout_contains("running 0 tests"));
}

#[test]
fn dont_run_examples() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", r#"
        "#)
        .file("examples/dont-run-me-i-will-fail.rs", r#"
            fn main() { panic!("Examples should not be run by 'cargo test'"); }
        "#);
    assert_that(p.cargo_process("test"),
                execs().with_status(0));
}

#[test]
fn pass_through_command_line() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "
            #[test] fn foo() {}
            #[test] fn bar() {}
        ");

    assert_that(p.cargo_process("test").arg("bar"),
                execs().with_status(0)
                       .with_stderr(&format!("\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[/]debug[/]deps[/]foo-[..][EXE]
[DOCTEST] foo", dir = p.url()))
                       .with_stdout_contains("test bar ... ok")
                       .with_stdout_contains("running 0 tests"));

    assert_that(p.cargo("test").arg("foo"),
                execs().with_status(0)
                       .with_stderr("\
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[/]debug[/]deps[/]foo-[..][EXE]
[DOCTEST] foo")
                       .with_stdout_contains("test foo ... ok")
                       .with_stdout_contains("running 0 tests"));
}

// Regression test for running cargo-test twice with
// tests in an rlib
#[test]
fn cargo_test_twice() {
    let p = project("test_twice")
        .file("Cargo.toml", &basic_lib_manifest("test_twice"))
        .file("src/test_twice.rs", r#"
            #![crate_type = "rlib"]

            #[test]
            fn dummy_test() { }
            "#);

    p.cargo_process("build");

    for _ in 0..2 {
        assert_that(p.cargo("test"),
                    execs().with_status(0));
    }
}

#[test]
fn lib_bin_same_name() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [lib]
            name = "foo"
            [[bin]]
            name = "foo"
        "#)
        .file("src/lib.rs", "
            #[test] fn lib_test() {}
        ")
        .file("src/main.rs", "
            extern crate foo;

            #[test]
            fn bin_test() {}
        ");

    assert_that(p.cargo_process("test"),
                execs().with_status(0).with_stderr(format!("\
[COMPILING] foo v0.0.1 ({})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[/]debug[/]deps[/]foo-[..][EXE]
[RUNNING] target[/]debug[/]deps[/]foo-[..][EXE]
[DOCTEST] foo", p.url()))
                       .with_stdout_contains_n("test [..] ... ok", 2)
                       .with_stdout_contains("running 0 tests"));
}

#[test]
fn lib_with_standard_name() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "syntax"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "
            /// ```
            /// syntax::foo();
            /// ```
            pub fn foo() {}

            #[test]
            fn foo_test() {}
        ")
        .file("tests/test.rs", "
            extern crate syntax;

            #[test]
            fn test() { syntax::foo() }
        ");

    assert_that(p.cargo_process("test"),
                execs().with_status(0)
                       .with_stderr(&format!("\
[COMPILING] syntax v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[/]debug[/]deps[/]syntax-[..][EXE]
[RUNNING] target[/]debug[/]deps[/]test-[..][EXE]
[DOCTEST] syntax", dir = p.url()))
                       .with_stdout_contains("test foo_test ... ok")
                       .with_stdout_contains("test test ... ok")
                       .with_stdout_contains_n("test [..] ... ok", 3));
}

#[test]
fn lib_with_standard_name2() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "syntax"
            version = "0.0.1"
            authors = []

            [lib]
            name = "syntax"
            test = false
            doctest = false
        "#)
        .file("src/lib.rs", "
            pub fn foo() {}
        ")
        .file("src/main.rs", "
            extern crate syntax;

            fn main() {}

            #[test]
            fn test() { syntax::foo() }
        ");

    assert_that(p.cargo_process("test"),
                execs().with_status(0)
                       .with_stderr(&format!("\
[COMPILING] syntax v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[/]debug[/]deps[/]syntax-[..][EXE]", dir = p.url()))
                       .with_stdout_contains("test test ... ok"));
}

#[test]
fn lib_without_name() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "syntax"
            version = "0.0.1"
            authors = []

            [lib]
            test = false
            doctest = false
        "#)
        .file("src/lib.rs", "
            pub fn foo() {}
        ")
        .file("src/main.rs", "
            extern crate syntax;

            fn main() {}

            #[test]
            fn test() { syntax::foo() }
        ");

    assert_that(p.cargo_process("test"),
                execs().with_status(0)
                       .with_stderr(&format!("\
[COMPILING] syntax v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[/]debug[/]deps[/]syntax-[..][EXE]", dir = p.url()))
                       .with_stdout_contains("test test ... ok"));
}

#[test]
fn bin_without_name() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "syntax"
            version = "0.0.1"
            authors = []

            [lib]
            test = false
            doctest = false

            [[bin]]
            path = "src/main.rs"
        "#)
        .file("src/lib.rs", "
            pub fn foo() {}
        ")
        .file("src/main.rs", "
            extern crate syntax;

            fn main() {}

            #[test]
            fn test() { syntax::foo() }
        ");

    assert_that(p.cargo_process("test"),
                execs().with_status(101)
                       .with_stderr("\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  binary target bin.name is required"));
}

#[test]
fn bench_without_name() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "syntax"
            version = "0.0.1"
            authors = []

            [lib]
            test = false
            doctest = false

            [[bench]]
            path = "src/bench.rs"
        "#)
        .file("src/lib.rs", "
            pub fn foo() {}
        ")
        .file("src/main.rs", "
            extern crate syntax;

            fn main() {}

            #[test]
            fn test() { syntax::foo() }
        ")
        .file("src/bench.rs", "
            #![feature(test)]
            extern crate syntax;
            extern crate test;

            #[bench]
            fn external_bench(_b: &mut test::Bencher) {}
        ");

    assert_that(p.cargo_process("test"),
                execs().with_status(101)
                       .with_stderr("\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  bench target bench.name is required"));
}

#[test]
fn test_without_name() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "syntax"
            version = "0.0.1"
            authors = []

            [lib]
            test = false
            doctest = false

            [[test]]
            path = "src/test.rs"
        "#)
        .file("src/lib.rs", r#"
            pub fn foo() {}
            pub fn get_hello() -> &'static str { "Hello" }
        "#)
        .file("src/main.rs", "
            extern crate syntax;

            fn main() {}

            #[test]
            fn test() { syntax::foo() }
        ")
        .file("src/test.rs", r#"
            extern crate syntax;

            #[test]
            fn external_test() { assert_eq!(syntax::get_hello(), "Hello") }
        "#);

    assert_that(p.cargo_process("test"),
                execs().with_status(101)
                       .with_stderr("\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  test target test.name is required"));
}

#[test]
fn example_without_name() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "syntax"
            version = "0.0.1"
            authors = []

            [lib]
            test = false
            doctest = false

            [[example]]
            path = "examples/example.rs"
        "#)
        .file("src/lib.rs", "
            pub fn foo() {}
        ")
        .file("src/main.rs", "
            extern crate syntax;

            fn main() {}

            #[test]
            fn test() { syntax::foo() }
        ")
        .file("examples/example.rs", r#"
            extern crate syntax;

            fn main() {
                println!("example1");
            }
        "#);

    assert_that(p.cargo_process("test"),
                execs().with_status(101)
                       .with_stderr("\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  example target example.name is required"));
}

#[test]
fn bin_there_for_integration() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/main.rs", "
            fn main() { std::process::exit(101); }
            #[test] fn main_test() {}
        ")
        .file("tests/foo.rs", r#"
            use std::process::Command;
            #[test]
            fn test_test() {
                let status = Command::new("target/debug/foo").status().unwrap();
                assert_eq!(status.code(), Some(101));
            }
        "#);

    let output = p.cargo_process("test").arg("-v").exec_with_output().unwrap();
    let output = str::from_utf8(&output.stdout).unwrap();
    assert!(output.contains("main_test ... ok"), "no main_test\n{}", output);
    assert!(output.contains("test_test ... ok"), "no test_test\n{}", output);
}

#[test]
fn test_dylib() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [lib]
            name = "foo"
            crate_type = ["dylib"]

            [dependencies.bar]
            path = "bar"
        "#)
        .file("src/lib.rs", r#"
            extern crate bar as the_bar;

            pub fn bar() { the_bar::baz(); }

            #[test]
            fn foo() { bar(); }
        "#)
        .file("tests/test.rs", r#"
            extern crate foo as the_foo;

            #[test]
            fn foo() { the_foo::bar(); }
        "#)
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [lib]
            name = "bar"
            crate_type = ["dylib"]
        "#)
        .file("bar/src/lib.rs", "
             pub fn baz() {}
        ");

    assert_that(p.cargo_process("test"),
                execs().with_status(0)
                       .with_stderr(&format!("\
[COMPILING] bar v0.0.1 ({dir}/bar)
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[/]debug[/]deps[/]foo-[..][EXE]
[RUNNING] target[/]debug[/]deps[/]test-[..][EXE]", dir = p.url()))
                       .with_stdout_contains_n("test foo ... ok", 2));

    p.root().move_into_the_past();
    assert_that(p.cargo("test"),
                execs().with_status(0)
                       .with_stderr("\
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[/]debug[/]deps[/]foo-[..][EXE]
[RUNNING] target[/]debug[/]deps[/]test-[..][EXE]")
                       .with_stdout_contains_n("test foo ... ok", 2));
}

#[test]
fn test_twice_with_build_cmd() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            build = "build.rs"
        "#)
        .file("build.rs", "fn main() {}")
        .file("src/lib.rs", "
            #[test]
            fn foo() {}
        ");

    assert_that(p.cargo_process("test"),
                execs().with_status(0)
                       .with_stderr(&format!("\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[/]debug[/]deps[/]foo-[..][EXE]
[DOCTEST] foo", dir = p.url()))
                       .with_stdout_contains("test foo ... ok")
                       .with_stdout_contains("running 0 tests"));

    assert_that(p.cargo("test"),
                execs().with_status(0)
                       .with_stderr("\
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[/]debug[/]deps[/]foo-[..][EXE]
[DOCTEST] foo")
                       .with_stdout_contains("test foo ... ok")
                       .with_stdout_contains("running 0 tests"));
}

#[test]
fn test_then_build() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "
            #[test]
            fn foo() {}
        ");

    assert_that(p.cargo_process("test"),
                execs().with_status(0)
                       .with_stderr(&format!("\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[/]debug[/]deps[/]foo-[..][EXE]
[DOCTEST] foo", dir = p.url()))
                       .with_stdout_contains("test foo ... ok")
                       .with_stdout_contains("running 0 tests"));

    assert_that(p.cargo("build"),
                execs().with_status(0)
                       .with_stdout(""));
}

#[test]
fn test_no_run() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "
            #[test]
            fn foo() { panic!() }
        ");

    assert_that(p.cargo_process("test").arg("--no-run"),
                execs().with_status(0)
                       .with_stderr(&format!("\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
                       dir = p.url())));
}

#[test]
fn test_run_specific_bin_target() {
    let prj = project("foo")
        .file("Cargo.toml" , r#"
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
        "#)
        .file("src/bin1.rs", "#[test] fn test1() { }")
        .file("src/bin2.rs", "#[test] fn test2() { }");

    assert_that(prj.cargo_process("test").arg("--bin").arg("bin2"),
                execs().with_status(0)
                       .with_stderr(format!("\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[/]debug[/]deps[/]bin2-[..][EXE]", dir = prj.url()))
                       .with_stdout_contains("test test2 ... ok"));
}

#[test]
fn test_run_implicit_bin_target() {
    let prj = project("foo")
        .file("Cargo.toml" , r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [[bin]]
            name="mybin"
            path="src/mybin.rs"
        "#)
        .file("src/mybin.rs", "#[test] fn test_in_bin() { }
               fn main() { panic!(\"Don't execute me!\"); }")
        .file("tests/mytest.rs", "#[test] fn test_in_test() { }")
        .file("benches/mybench.rs", "#[test] fn test_in_bench() { }")
        .file("examples/myexm.rs", "#[test] fn test_in_exm() { }
               fn main() { panic!(\"Don't execute me!\"); }");

    assert_that(prj.cargo_process("test").arg("--bins"),
                execs().with_status(0)
                       .with_stderr(format!("\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[/]debug[/]deps[/]mybin-[..][EXE]", dir = prj.url()))
                       .with_stdout_contains("test test_in_bin ... ok"));
}

#[test]
fn test_run_specific_test_target() {
    let prj = project("foo")
        .file("Cargo.toml" , r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/bin/a.rs", "fn main() { }")
        .file("src/bin/b.rs", "#[test] fn test_b() { } fn main() { }")
        .file("tests/a.rs", "#[test] fn test_a() { }")
        .file("tests/b.rs", "#[test] fn test_b() { }");

    assert_that(prj.cargo_process("test").arg("--test").arg("b"),
                execs().with_status(0)
                       .with_stderr(format!("\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[/]debug[/]deps[/]b-[..][EXE]", dir = prj.url()))
                       .with_stdout_contains("test test_b ... ok"));
}

#[test]
fn test_run_implicit_test_target() {
    let prj = project("foo")
        .file("Cargo.toml" , r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [[bin]]
            name="mybin"
            path="src/mybin.rs"
        "#)
        .file("src/mybin.rs", "#[test] fn test_in_bin() { }
               fn main() { panic!(\"Don't execute me!\"); }")
        .file("tests/mytest.rs", "#[test] fn test_in_test() { }")
        .file("benches/mybench.rs", "#[test] fn test_in_bench() { }")
        .file("examples/myexm.rs", "#[test] fn test_in_exm() { }
               fn main() { panic!(\"Don't execute me!\"); }");

    assert_that(prj.cargo_process("test").arg("--tests"),
                execs().with_status(0)
                       .with_stderr(format!("\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[/]debug[/]deps[/]mytest-[..][EXE]", dir = prj.url()))
                       .with_stdout_contains("test test_in_test ... ok"));
}

#[test]
fn test_run_implicit_bench_target() {
    let prj = project("foo")
        .file("Cargo.toml" , r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [[bin]]
            name="mybin"
            path="src/mybin.rs"
        "#)
        .file("src/mybin.rs", "#[test] fn test_in_bin() { }
               fn main() { panic!(\"Don't execute me!\"); }")
        .file("tests/mytest.rs", "#[test] fn test_in_test() { }")
        .file("benches/mybench.rs", "#[test] fn test_in_bench() { }")
        .file("examples/myexm.rs", "#[test] fn test_in_exm() { }
               fn main() { panic!(\"Don't execute me!\"); }");

    assert_that(prj.cargo_process("test").arg("--benches"),
                execs().with_status(0)
                       .with_stderr(format!("\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[/]debug[/]deps[/]mybench-[..][EXE]", dir = prj.url()))
                       .with_stdout_contains("test test_in_bench ... ok"));
}

#[test]
fn test_run_implicit_example_target() {
    let prj = project("foo")
        .file("Cargo.toml" , r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [[bin]]
            name="mybin"
            path="src/mybin.rs"
        "#)
        .file("src/mybin.rs", "#[test] fn test_in_bin() { }
               fn main() { panic!(\"Don't execute me!\"); }")
        .file("tests/mytest.rs", "#[test] fn test_in_test() { }")
        .file("benches/mybench.rs", "#[test] fn test_in_bench() { }")
        .file("examples/myexm.rs", "#[test] fn test_in_exm() { }
               fn main() { panic!(\"Don't execute me!\"); }");

    assert_that(prj.cargo_process("test").arg("--examples"),
                execs().with_status(0)
                       .with_stderr(format!("\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]", dir = prj.url())));
}

#[test]
fn test_no_harness() {
    let p = project("foo")
        .file("Cargo.toml", r#"
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
        "#)
        .file("src/main.rs", "fn main() {}")
        .file("foo.rs", "fn main() {}");

    assert_that(p.cargo_process("test").arg("--").arg("--nocapture"),
                execs().with_status(0)
                       .with_stderr(&format!("\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[/]debug[/]deps[/]bar-[..][EXE]
",
                       dir = p.url())));
}

#[test]
fn selective_testing() {
    let p = project("foo")
        .file("Cargo.toml", r#"
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
        "#)
        .file("src/lib.rs", "")
        .file("d1/Cargo.toml", r#"
            [package]
            name = "d1"
            version = "0.0.1"
            authors = []

            [lib]
                name = "d1"
                doctest = false
        "#)
        .file("d1/src/lib.rs", "")
        .file("d1/src/main.rs", "extern crate d1; fn main() {}")
        .file("d2/Cargo.toml", r#"
            [package]
            name = "d2"
            version = "0.0.1"
            authors = []

            [lib]
                name = "d2"
                doctest = false
        "#)
        .file("d2/src/lib.rs", "")
        .file("d2/src/main.rs", "extern crate d2; fn main() {}");
    p.build();

    println!("d1");
    assert_that(p.cargo("test").arg("-p").arg("d1"),
                execs().with_status(0)
                       .with_stderr(&format!("\
[COMPILING] d1 v0.0.1 ({dir}/d1)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[/]debug[/]deps[/]d1-[..][EXE]
[RUNNING] target[/]debug[/]deps[/]d1-[..][EXE]", dir = p.url()))
                       .with_stdout_contains_n("running 0 tests", 2));

    println!("d2");
    assert_that(p.cargo("test").arg("-p").arg("d2"),
                execs().with_status(0)
                       .with_stderr(&format!("\
[COMPILING] d2 v0.0.1 ({dir}/d2)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[/]debug[/]deps[/]d2-[..][EXE]
[RUNNING] target[/]debug[/]deps[/]d2-[..][EXE]", dir = p.url()))
                       .with_stdout_contains_n("running 0 tests", 2));

    println!("whole");
    assert_that(p.cargo("test"),
                execs().with_status(0)
                       .with_stderr(&format!("\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[/]debug[/]deps[/]foo-[..][EXE]", dir = p.url()))
                       .with_stdout_contains("running 0 tests"));
}

#[test]
fn almost_cyclic_but_not_quite() {
    let p = project("a")
        .file("Cargo.toml", r#"
            [package]
            name = "a"
            version = "0.0.1"
            authors = []

            [dev-dependencies.b]
            path = "b"
            [dev-dependencies.c]
            path = "c"
        "#)
        .file("src/lib.rs", r#"
            #[cfg(test)] extern crate b;
            #[cfg(test)] extern crate c;
        "#)
        .file("b/Cargo.toml", r#"
            [package]
            name = "b"
            version = "0.0.1"
            authors = []

            [dependencies.a]
            path = ".."
        "#)
        .file("b/src/lib.rs", r#"
            extern crate a;
        "#)
        .file("c/Cargo.toml", r#"
            [package]
            name = "c"
            version = "0.0.1"
            authors = []
        "#)
        .file("c/src/lib.rs", "");

    assert_that(p.cargo_process("build"), execs().with_status(0));
    assert_that(p.cargo("test"),
                execs().with_status(0));
}

#[test]
fn build_then_selective_test() {
    let p = project("a")
        .file("Cargo.toml", r#"
            [package]
            name = "a"
            version = "0.0.1"
            authors = []

            [dependencies.b]
            path = "b"
        "#)
        .file("src/lib.rs", "extern crate b;")
        .file("src/main.rs", "extern crate b; extern crate a; fn main() {}")
        .file("b/Cargo.toml", r#"
            [package]
            name = "b"
            version = "0.0.1"
            authors = []
        "#)
        .file("b/src/lib.rs", "");

    assert_that(p.cargo_process("build"), execs().with_status(0));
    p.root().move_into_the_past();
    assert_that(p.cargo("test").arg("-p").arg("b"),
                execs().with_status(0));
}

#[test]
fn example_dev_dep() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dev-dependencies.bar]
            path = "bar"
        "#)
        .file("src/lib.rs", r#"
        "#)
        .file("examples/e1.rs", r#"
            extern crate bar;
            fn main() { }
        "#)
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []
        "#)
        .file("bar/src/lib.rs", r#"
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
        "#);
    assert_that(p.cargo_process("test"),
                execs().with_status(0));
    assert_that(p.cargo("run")
                 .arg("--example").arg("e1").arg("--release").arg("-v"),
                execs().with_status(0));
}

#[test]
fn selective_testing_with_docs() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.d1]
                path = "d1"
        "#)
        .file("src/lib.rs", r#"
            /// ```
            /// not valid rust
            /// ```
            pub fn foo() {}
        "#)
        .file("d1/Cargo.toml", r#"
            [package]
            name = "d1"
            version = "0.0.1"
            authors = []

            [lib]
            name = "d1"
            path = "d1.rs"
        "#)
        .file("d1/d1.rs", "");
    p.build();

    assert_that(p.cargo("test").arg("-p").arg("d1"),
                execs().with_status(0)
                       .with_stderr(&format!("\
[COMPILING] d1 v0.0.1 ({dir}/d1)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[/]debug[/]deps[/]d1[..][EXE]
[DOCTEST] d1", dir = p.url()))
                       .with_stdout_contains_n("running 0 tests", 2));
}

#[test]
fn example_bin_same_name() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/bin/foo.rs", r#"fn main() { println!("bin"); }"#)
        .file("examples/foo.rs", r#"fn main() { println!("example"); }"#);

    assert_that(p.cargo_process("test").arg("--no-run").arg("-v"),
                execs().with_status(0)
                       .with_stderr(&format!("\
[COMPILING] foo v0.0.1 ({dir})
[RUNNING] `rustc [..]`
[RUNNING] `rustc [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
", dir = p.url())));

    assert_that(&p.bin("foo"), is_not(existing_file()));
    assert_that(&p.bin("examples/foo"), existing_file());

    assert_that(p.process(&p.bin("examples/foo")),
                execs().with_status(0).with_stdout("example\n"));

    assert_that(p.cargo("run"),
                execs().with_status(0)
                       .with_stderr("\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..]")
                       .with_stdout("\
bin
"));
    assert_that(&p.bin("foo"), existing_file());
}

#[test]
fn test_with_example_twice() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/bin/foo.rs", r#"fn main() { println!("bin"); }"#)
        .file("examples/foo.rs", r#"fn main() { println!("example"); }"#);

    println!("first");
    assert_that(p.cargo_process("test").arg("-v"),
                execs().with_status(0));
    assert_that(&p.bin("examples/foo"), existing_file());
    println!("second");
    assert_that(p.cargo("test").arg("-v"),
                execs().with_status(0));
    assert_that(&p.bin("examples/foo"), existing_file());
}

#[test]
fn example_with_dev_dep() {
    let p = project("foo")
        .file("Cargo.toml", r#"
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
        "#)
        .file("src/lib.rs", "")
        .file("examples/ex.rs", "extern crate a; fn main() {}")
        .file("a/Cargo.toml", r#"
            [package]
            name = "a"
            version = "0.0.1"
            authors = []
        "#)
        .file("a/src/lib.rs", "");

    assert_that(p.cargo_process("test").arg("-v"),
                execs().with_status(0)
                       .with_stderr("\
[..]
[..]
[..]
[..]
[RUNNING] `rustc --crate-name ex [..] --extern a=[..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
}

#[test]
fn bin_is_preserved() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "")
        .file("src/main.rs", "fn main() {}");

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0));
    assert_that(&p.bin("foo"), existing_file());

    println!("testing");
    assert_that(p.cargo("test").arg("-v"),
                execs().with_status(0));
    assert_that(&p.bin("foo"), existing_file());
}

#[test]
fn bad_example() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "");
    p.build();

    assert_that(p.cargo("run").arg("--example").arg("foo"),
                execs().with_status(101).with_stderr("\
[ERROR] no example target named `foo`
"));
    assert_that(p.cargo("run").arg("--bin").arg("foo"),
                execs().with_status(101).with_stderr("\
[ERROR] no bin target named `foo`
"));
}

#[test]
fn doctest_feature() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            [features]
            bar = []
        "#)
        .file("src/lib.rs", r#"
            /// ```rust
            /// assert_eq!(foo::foo(), 1);
            /// ```
            #[cfg(feature = "bar")]
            pub fn foo() -> i32 { 1 }
        "#);

    assert_that(p.cargo_process("test").arg("--features").arg("bar"),
                execs().with_status(0)
                       .with_stderr("\
[COMPILING] foo [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[/]debug[/]deps[/]foo[..][EXE]
[DOCTEST] foo")
                       .with_stdout_contains("running 0 tests")
                       .with_stdout_contains("test [..] ... ok"));
}

#[test]
fn dashes_to_underscores() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo-bar"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", r#"
            /// ```
            /// assert_eq!(foo_bar::foo(), 1);
            /// ```
            pub fn foo() -> i32 { 1 }
        "#);

    assert_that(p.cargo_process("test").arg("-v"),
                execs().with_status(0));
}

#[test]
fn doctest_dev_dep() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dev-dependencies]
            b = { path = "b" }
        "#)
        .file("src/lib.rs", r#"
            /// ```
            /// extern crate b;
            /// ```
            pub fn foo() {}
        "#)
        .file("b/Cargo.toml", r#"
            [package]
            name = "b"
            version = "0.0.1"
            authors = []
        "#)
        .file("b/src/lib.rs", "");

    assert_that(p.cargo_process("test").arg("-v"),
                execs().with_status(0));
}

#[test]
fn filter_no_doc_tests() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", r#"
            /// ```
            /// extern crate b;
            /// ```
            pub fn foo() {}
        "#)
        .file("tests/foo.rs", "");

    assert_that(p.cargo_process("test").arg("--test=foo"),
                execs().with_status(0).with_stderr("\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[/]debug[/]deps[/]foo[..][EXE]")
                       .with_stdout_contains("running 0 tests"));
}

#[test]
fn dylib_doctest() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [lib]
            name = "foo"
            crate-type = ["rlib", "dylib"]
            test = false
        "#)
        .file("src/lib.rs", r#"
            /// ```
            /// foo::foo();
            /// ```
            pub fn foo() {}
        "#);

    assert_that(p.cargo_process("test"),
                execs().with_status(0).with_stderr("\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[DOCTEST] foo")
                       .with_stdout_contains("test [..] ... ok"));
}

#[test]
fn dylib_doctest2() {
    // can't doctest dylibs as they're statically linked together
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [lib]
            name = "foo"
            crate-type = ["dylib"]
            test = false
        "#)
        .file("src/lib.rs", r#"
            /// ```
            /// foo::foo();
            /// ```
            pub fn foo() {}
        "#);

    assert_that(p.cargo_process("test"),
                execs().with_status(0).with_stdout(""));
}

#[test]
fn cyclic_dev_dep_doc_test() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dev-dependencies]
            bar = { path = "bar" }
        "#)
        .file("src/lib.rs", r#"
            //! ```
            //! extern crate bar;
            //! ```
        "#)
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = { path = ".." }
        "#)
        .file("bar/src/lib.rs", r#"
            extern crate foo;
        "#);
    assert_that(p.cargo_process("test"),
                execs().with_status(0).with_stderr("\
[COMPILING] foo v0.0.1 ([..])
[COMPILING] bar v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[/]debug[/]deps[/]foo[..][EXE]
[DOCTEST] foo")
                       .with_stdout_contains("running 0 tests")
                       .with_stdout_contains("test [..] ... ok"));
}

#[test]
fn dev_dep_with_build_script() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dev-dependencies]
            bar = { path = "bar" }
        "#)
        .file("src/lib.rs", "")
        .file("examples/foo.rs", "fn main() {}")
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []
            build = "build.rs"
        "#)
        .file("bar/src/lib.rs", "")
        .file("bar/build.rs", "fn main() {}");
    assert_that(p.cargo_process("test"),
                execs().with_status(0));
}

#[test]
fn no_fail_fast() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", r#"
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
        "#)
        .file("tests/test_add_one.rs", r#"
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
        "#)
        .file("tests/test_sub_one.rs", r#"
        extern crate foo;
        use foo::*;

        #[test]
        fn sub_one_test() {
            assert_eq!(sub_one(1), 0);
        }
        "#);
    assert_that(p.cargo_process("test").arg("--no-fail-fast"),
                execs().with_status(101)
                       .with_stderr_contains("\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[/]debug[/]deps[/]foo-[..][EXE]
[RUNNING] target[/]debug[/]deps[/]test_add_one-[..][EXE]")
                       .with_stdout_contains("running 0 tests")
                       .with_stderr_contains("\
[RUNNING] target[/]debug[/]deps[/]test_sub_one-[..][EXE]
[DOCTEST] foo")
                       .with_stdout_contains("test result: FAILED. [..]")
                       .with_stdout_contains("test sub_one_test ... ok")
                       .with_stdout_contains_n("test [..] ... ok", 3));
}

#[test]
fn test_multiple_packages() {
    let p = project("foo")
        .file("Cargo.toml", r#"
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
        "#)
        .file("src/lib.rs", "")
        .file("d1/Cargo.toml", r#"
            [package]
            name = "d1"
            version = "0.0.1"
            authors = []

            [lib]
                name = "d1"
                doctest = false
        "#)
        .file("d1/src/lib.rs", "")
        .file("d2/Cargo.toml", r#"
            [package]
            name = "d2"
            version = "0.0.1"
            authors = []

            [lib]
                name = "d2"
                doctest = false
        "#)
        .file("d2/src/lib.rs", "");
    p.build();

    assert_that(p.cargo("test").arg("-p").arg("d1").arg("-p").arg("d2"),
                execs().with_status(0)
                       .with_stderr_contains("\
[RUNNING] target[/]debug[/]deps[/]d1-[..][EXE]")
                       .with_stderr_contains("\
[RUNNING] target[/]debug[/]deps[/]d2-[..][EXE]")
                       .with_stdout_contains_n("running 0 tests", 2));
}

#[test]
fn bin_does_not_rebuild_tests() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "")
        .file("src/main.rs", "fn main() {}")
        .file("tests/foo.rs", "");
    p.build();

    assert_that(p.cargo("test").arg("-v"),
                execs().with_status(0));

    sleep_ms(1000);
    File::create(&p.root().join("src/main.rs")).unwrap()
         .write_all(b"fn main() { 3; }").unwrap();

    assert_that(p.cargo("test").arg("-v").arg("--no-run"),
                execs().with_status(0)
                       .with_stderr("\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..] src[/]main.rs [..]`
[RUNNING] `rustc [..] src[/]main.rs [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
}

#[test]
fn selective_test_wonky_profile() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [profile.release]
            opt-level = 2

            [dependencies]
            a = { path = "a" }
        "#)
        .file("src/lib.rs", "")
        .file("a/Cargo.toml", r#"
            [package]
            name = "a"
            version = "0.0.1"
            authors = []
        "#)
        .file("a/src/lib.rs", "");
    p.build();

    assert_that(p.cargo("test").arg("-v").arg("--no-run").arg("--release")
                 .arg("-p").arg("foo").arg("-p").arg("a"),
                execs().with_status(0));
}

#[test]
fn selective_test_optional_dep() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            a = { path = "a", optional = true }
        "#)
        .file("src/lib.rs", "")
        .file("a/Cargo.toml", r#"
            [package]
            name = "a"
            version = "0.0.1"
            authors = []
        "#)
        .file("a/src/lib.rs", "");
    p.build();

    assert_that(p.cargo("test").arg("-v").arg("--no-run")
                 .arg("--features").arg("a").arg("-p").arg("a"),
                execs().with_status(0).with_stderr("\
[COMPILING] a v0.0.1 ([..])
[RUNNING] `rustc [..] a[/]src[/]lib.rs [..]`
[RUNNING] `rustc [..] a[/]src[/]lib.rs [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
}

#[test]
fn only_test_docs() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", r#"
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
        "#)
        .file("tests/foo.rs", "this is not rust");
    p.build();

    assert_that(p.cargo("test").arg("--doc"),
                execs().with_status(0)
                       .with_stderr("\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[DOCTEST] foo")
                       .with_stdout_contains("test [..] ... ok"));
}

#[test]
fn test_panic_abort_with_dep() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = { path = "bar" }

            [profile.dev]
            panic = 'abort'
        "#)
        .file("src/lib.rs", r#"
            extern crate bar;

            #[test]
            fn foo() {}
        "#)
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []
        "#)
        .file("bar/src/lib.rs", "");
    assert_that(p.cargo_process("test").arg("-v"),
                execs().with_status(0));
}

#[test]
fn cfg_test_even_with_no_harness() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [lib]
            harness = false
            doctest = false
        "#)
        .file("src/lib.rs", r#"
            #[cfg(test)]
            fn main() {
                println!("hello!");
            }
        "#);
    assert_that(p.cargo_process("test").arg("-v"),
                execs().with_status(0)
                       .with_stdout("hello!\n")
                       .with_stderr("\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[..]`
"));
}

#[test]
fn panic_abort_multiple() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            a = { path = "a" }

            [profile.release]
            panic = 'abort'
        "#)
        .file("src/lib.rs", "extern crate a;")
        .file("a/Cargo.toml", r#"
            [package]
            name = "a"
            version = "0.0.1"
            authors = []
        "#)
        .file("a/src/lib.rs", "");
    assert_that(p.cargo_process("test")
                 .arg("--release").arg("-v")
                 .arg("-p").arg("foo")
                 .arg("-p").arg("a"),
                execs().with_status(0));
}

#[test]
fn pass_correct_cfgs_flags_to_rustdoc() {
    let p = project("foo")
        .file("Cargo.toml", r#"
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
        "#)
        .file("src/lib.rs", r#"
            #[cfg(test)]
            mod tests {
                #[test]
                fn it_works() {
                  assert!(true);
                }
            }
        "#)
        .file("libs/feature_a/Cargo.toml", r#"
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
        "#)
        .file("libs/feature_a/src/lib.rs", r#"
            #[cfg(feature = "mock_serde_derive")]
            const MSG: &'static str = "This is safe";

            #[cfg(feature = "mock_serde_codegen")]
            const MSG: &'static str = "This is risky";

            pub fn get() -> &'static str {
                MSG
            }
        "#)
        .file("libs/mock_serde_derive/Cargo.toml", r#"
            [package]
            name = "mock_serde_derive"
            version = "0.1.0"
            authors = []
        "#)
        .file("libs/mock_serde_derive/src/lib.rs", "")
        .file("libs/mock_serde_codegen/Cargo.toml", r#"
                [package]
                name = "mock_serde_codegen"
                version = "0.1.0"
                authors = []
            "#)
        .file("libs/mock_serde_codegen/src/lib.rs", "");
    p.build();

    assert_that(p.cargo("test")
                .arg("--package").arg("feature_a")
                .arg("--verbose"),
                execs().with_status(0)
                       .with_stderr_contains("\
[DOCTEST] feature_a
[RUNNING] `rustdoc --test [..]mock_serde_codegen[..]`"));

    assert_that(p.cargo("test")
                .arg("--verbose"),
                execs().with_status(0)
                       .with_stderr_contains("\
[DOCTEST] foo
[RUNNING] `rustdoc --test [..]feature_a[..]`"));
}

#[test]
fn test_release_ignore_panic() {
    let p = project("foo")
        .file("Cargo.toml", r#"
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
        "#)
        .file("src/lib.rs", "extern crate a;")
        .file("a/Cargo.toml", r#"
            [package]
            name = "a"
            version = "0.0.1"
            authors = []
        "#)
        .file("a/src/lib.rs", "");
    p.build();
    println!("test");
    assert_that(p.cargo("test").arg("-v"), execs().with_status(0));
    println!("bench");
    assert_that(p.cargo("bench").arg("-v"), execs().with_status(0));
}

#[test]
fn test_many_with_features() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            a = { path = "a" }

            [features]
            foo = []

            [workspace]
        "#)
        .file("src/lib.rs", "")
        .file("a/Cargo.toml", r#"
            [package]
            name = "a"
            version = "0.0.1"
            authors = []
        "#)
        .file("a/src/lib.rs", "");

    assert_that(p.cargo_process("test").arg("-v")
                 .arg("-p").arg("a")
                 .arg("-p").arg("foo")
                 .arg("--features").arg("foo"),
                execs().with_status(0));
}

#[test]
fn test_all_workspace() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            bar = { path = "bar" }

            [workspace]
        "#)
        .file("src/main.rs", r#"
            #[test]
            fn foo_test() {}
        "#)
        .file("bar/Cargo.toml", r#"
            [project]
            name = "bar"
            version = "0.1.0"
        "#)
        .file("bar/src/lib.rs", r#"
            #[test]
            fn bar_test() {}
        "#);

    assert_that(p.cargo_process("test")
                 .arg("--all"),
                execs().with_status(0)
                       .with_stdout_contains("test foo_test ... ok")
                       .with_stdout_contains("test bar_test ... ok"));
}

#[test]
fn test_all_exclude() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.1.0"

            [workspace]
            members = ["bar", "baz"]
        "#)
        .file("src/main.rs", r#"
            fn main() {}
        "#)
        .file("bar/Cargo.toml", r#"
            [project]
            name = "bar"
            version = "0.1.0"
        "#)
        .file("bar/src/lib.rs", r#"
            #[test]
            pub fn bar() {}
        "#)
        .file("baz/Cargo.toml", r#"
            [project]
            name = "baz"
            version = "0.1.0"
        "#)
        .file("baz/src/lib.rs", r#"
            #[test]
            pub fn baz() {
                assert!(false);
            }
        "#);

    assert_that(p.cargo_process("test")
                    .arg("--all")
                    .arg("--exclude")
                    .arg("baz"),
                execs().with_status(0)
                    .with_stdout_contains("running 1 test
test bar ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured"));
}

#[test]
fn test_all_virtual_manifest() {
    let p = project("workspace")
        .file("Cargo.toml", r#"
            [workspace]
            members = ["a", "b"]
        "#)
        .file("a/Cargo.toml", r#"
            [project]
            name = "a"
            version = "0.1.0"
        "#)
        .file("a/src/lib.rs", r#"
            #[test]
            fn a() {}
        "#)
        .file("b/Cargo.toml", r#"
            [project]
            name = "b"
            version = "0.1.0"
        "#)
        .file("b/src/lib.rs", r#"
            #[test]
            fn b() {}
        "#);

    assert_that(p.cargo_process("test")
                 .arg("--all"),
                execs().with_status(0)
                       .with_stdout_contains("test a ... ok")
                       .with_stdout_contains("test b ... ok"));
}

#[test]
fn test_all_member_dependency_same_name() {
    let p = project("workspace")
        .file("Cargo.toml", r#"
            [workspace]
            members = ["a"]
        "#)
        .file("a/Cargo.toml", r#"
            [project]
            name = "a"
            version = "0.1.0"

            [dependencies]
            a = "0.1.0"
        "#)
        .file("a/src/lib.rs", r#"
            #[test]
            fn a() {}
        "#);

    Package::new("a", "0.1.0").publish();

    assert_that(p.cargo_process("test")
                 .arg("--all"),
                execs().with_status(0)
                       .with_stdout_contains("test a ... ok"));
}

#[test]
fn doctest_only_with_dev_dep() {
    let p = project("workspace")
        .file("Cargo.toml", r#"
            [project]
            name = "a"
            version = "0.1.0"

            [dev-dependencies]
            b = { path = "b" }
        "#)
        .file("src/lib.rs", r#"
            /// ```
            /// extern crate b;
            ///
            /// b::b();
            /// ```
            pub fn a() {}
        "#)
        .file("b/Cargo.toml", r#"
            [project]
            name = "b"
            version = "0.1.0"
        "#)
        .file("b/src/lib.rs", r#"
            pub fn b() {}
        "#);

    assert_that(p.cargo_process("test").arg("--doc").arg("-v"),
                execs().with_status(0));
}

#[test]
fn test_many_targets() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.1.0"
        "#)
        .file("src/bin/a.rs", r#"
            fn main() {}
            #[test] fn bin_a() {}
        "#)
        .file("src/bin/b.rs", r#"
            fn main() {}
            #[test] fn bin_b() {}
        "#)
        .file("src/bin/c.rs", r#"
            fn main() {}
            #[test] fn bin_c() { panic!(); }
        "#)
        .file("examples/a.rs", r#"
            fn main() {}
            #[test] fn example_a() {}
        "#)
        .file("examples/b.rs", r#"
            fn main() {}
            #[test] fn example_b() {}
        "#)
        .file("examples/c.rs", r#"
            #[test] fn example_c() { panic!(); }
        "#)
        .file("tests/a.rs", r#"
            #[test] fn test_a() {}
        "#)
        .file("tests/b.rs", r#"
            #[test] fn test_b() {}
        "#)
        .file("tests/c.rs", r#"
            does not compile
        "#);

    assert_that(p.cargo_process("test").arg("--verbose")
                    .arg("--bin").arg("a").arg("--bin").arg("b")
                    .arg("--example").arg("a").arg("--example").arg("b")
                    .arg("--test").arg("a").arg("--test").arg("b"),
                execs()
                    .with_status(0)
                    .with_stdout_contains("test bin_a ... ok")
                    .with_stdout_contains("test bin_b ... ok")
                    .with_stdout_contains("test test_a ... ok")
                    .with_stdout_contains("test test_b ... ok")
                    .with_stderr_contains("[RUNNING] `rustc --crate-name a examples[/]a.rs [..]`")
                    .with_stderr_contains("[RUNNING] `rustc --crate-name b examples[/]b.rs [..]`"))
}

#[test]
fn doctest_and_registry() {
    let p = project("workspace")
        .file("Cargo.toml", r#"
            [project]
            name = "a"
            version = "0.1.0"

            [dependencies]
            b = { path = "b" }
            c = { path = "c" }

            [workspace]
        "#)
        .file("src/lib.rs", "")
        .file("b/Cargo.toml", r#"
            [project]
            name = "b"
            version = "0.1.0"
        "#)
        .file("b/src/lib.rs", "
            /// ```
            /// b::foo();
            /// ```
            pub fn foo() {}
        ")
        .file("c/Cargo.toml", r#"
            [project]
            name = "c"
            version = "0.1.0"

            [dependencies]
            b = "0.1"
        "#)
        .file("c/src/lib.rs", "");

    Package::new("b", "0.1.0").publish();

    assert_that(p.cargo_process("test").arg("--all").arg("-v"),
                execs().with_status(0));
}

#[test]
fn cargo_test_env() {
    let src = format!(r#"
        #![crate_type = "rlib"]

        #[test]
        fn env_test() {{
            use std::env;
            println!("{{}}", env::var("{}").unwrap());
        }}
        "#, cargo::CARGO_ENV);

    let p = project("env_test")
        .file("Cargo.toml", &basic_lib_manifest("env_test"))
        .file("src/lib.rs", &src);

    let mut pr = p.cargo_process("test");
    let cargo = cargo_exe().canonicalize().unwrap();
    assert_that(pr.args(&["--lib", "--", "--nocapture"]),
                execs().with_status(0)
                       .with_stdout_contains(format!("\
{}
test env_test ... ok
", cargo.to_str().unwrap())));
}

#[test]
fn test_order() {
   let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.1.0"
        "#)
        .file("src/lib.rs", r#"
            #[test] fn test_lib() {}
        "#)
        .file("tests/a.rs", r#"
            #[test] fn test_a() {}
        "#)
        .file("tests/z.rs", r#"
            #[test] fn test_z() {}
        "#);

        assert_that(p.cargo_process("test").arg("--all"),
            execs().with_status(0)
                   .with_stdout_contains("
running 1 test
test test_lib ... ok

test result: ok. [..]


running 1 test
test test_a ... ok

test result: ok. [..]


running 1 test
test test_z ... ok

test result: ok. [..]
"));

}

#[test]
fn cyclic_dev() {
   let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.1.0"

            [dev-dependencies]
            foo = { path = "." }
        "#)
        .file("src/lib.rs", r#"
            #[test] fn test_lib() {}
        "#)
        .file("tests/foo.rs", r#"
            extern crate foo;
        "#);

    assert_that(p.cargo_process("test").arg("--all"),
                execs().with_status(0));
}
