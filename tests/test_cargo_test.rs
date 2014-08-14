use std::path;
use std::str;

use support::{project, execs, basic_bin_manifest, basic_lib_manifest};
use support::{COMPILING, cargo_dir, ResultTest, FRESH, RUNNING, DOCTEST};
use support::paths::PathExt;
use hamcrest::{assert_that, existing_file};
use cargo::util::process;

fn setup() {}

test!(cargo_test_simple {
    let p = project("foo")
        .file("Cargo.toml", basic_bin_manifest("foo").as_slice())
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

    assert_that(p.cargo_process("cargo-build"), execs());
    assert_that(&p.bin("foo"), existing_file());

    assert_that(
        process(p.bin("foo")),
        execs().with_stdout("hello\n"));

    assert_that(p.process(cargo_dir().join("cargo-test")),
        execs().with_stdout(format!("\
{} foo v0.5.0 ({})
{} target[..]test[..]foo

running 1 test
test test_hello ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured

",
        COMPILING, p.url(),
        RUNNING)));
})

test!(cargo_test_verbose {
    let p = project("foo")
        .file("Cargo.toml", basic_bin_manifest("foo").as_slice())
        .file("src/foo.rs", r#"
            fn main() {}
            #[test] fn test_hello() {}
        "#);

    assert_that(p.cargo_process("cargo-test").arg("-v").arg("hello"),
        execs().with_stdout(format!("\
{running} `rustc src[..]foo.rs [..]`
{compiling} foo v0.5.0 ({url})
{running} `[..]target[..]test[..]foo-[..] hello`

running 1 test
test test_hello ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured

",
        compiling = COMPILING, url = p.url(), running = RUNNING)));
})

test!(many_similar_names {
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

    let output = p.cargo_process("cargo-test").exec_with_output().assert();
    let output = str::from_utf8(output.output.as_slice()).assert();
    assert!(output.contains("test bin_test"), "bin_test missing\n{}", output);
    assert!(output.contains("test lib_test"), "lib_test missing\n{}", output);
    assert!(output.contains("test test_test"), "test_test missing\n{}", output);
})

test!(cargo_test_failing_test {
    let p = project("foo")
        .file("Cargo.toml", basic_bin_manifest("foo").as_slice())
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

    assert_that(p.cargo_process("cargo-build"), execs());
    assert_that(&p.bin("foo"), existing_file());

    assert_that(
        process(p.bin("foo")),
        execs().with_stdout("hello\n"));

    assert_that(p.process(cargo_dir().join("cargo-test")),
        execs().with_stdout(format!("\
{} foo v0.5.0 ({})
{} target[..]test[..]foo

running 1 test
test test_hello ... FAILED

failures:

---- test_hello stdout ----
<tab>task 'test_hello' failed at 'assertion failed: \
    `(left == right) && (right == left)` (left: \
    `hello`, right: `nope`)', src{sep}foo.rs:12
<tab>
<tab>

failures:
    test_hello

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured

",
        COMPILING, p.url(), RUNNING,
        sep = path::SEP))
              .with_stderr(format!("\
task '<main>' failed at 'Some tests failed', [..]
"))
              .with_status(101));
})

test!(test_with_lib_dep {
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
            ///     println!("{}", foo::foo());
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

    assert_that(p.cargo_process("cargo-test"),
        execs().with_stdout(format!("\
{} foo v0.0.1 ({})
{running} target[..]test[..]baz-[..]

running 1 test
test bin_test ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured

{running} target[..]test[..]foo

running 1 test
test lib_test ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured

{doctest} foo

running 1 test
test foo_0 ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured

",
        COMPILING, p.url(), running = RUNNING, doctest = DOCTEST)))
})

test!(test_with_deep_lib_dep {
    let p = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [dependencies.foo]
            path = "../foo"

            [[lib]]
            name = "bar"
            doctest = false
        "#)
        .file("src/lib.rs", "
            extern crate foo;
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
    assert_that(p.cargo_process("cargo-test"),
                execs().with_status(0)
                       .with_stdout(format!("\
{compiling} foo v0.0.1 ({dir})
{compiling} bar v0.0.1 ({dir})
{running} target[..]

running 1 test
test bar_test ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured

",
                       compiling = COMPILING, running = RUNNING,
                       dir = p.url()).as_slice()));
})

test!(external_test_explicit {
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

    assert_that(p.cargo_process("cargo-test"),
        execs().with_stdout(format!("\
{} foo v0.0.1 ({})
{running} target[..]test[..]foo-[..]

running 1 test
test internal_test ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured

{running} target[..]test[..]test-[..]

running 1 test
test external_test ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured

{doctest} foo

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured

",
        COMPILING, p.url(), running = RUNNING, doctest = DOCTEST)))
})

test!(external_test_implicit {
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

    assert_that(p.cargo_process("cargo-test"),
        execs().with_stdout(format!("\
{} foo v0.0.1 ({})
{running} target[..]test[..]external-[..]

running 1 test
test external_test ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured

{running} target[..]test[..]foo-[..]

running 1 test
test internal_test ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured

{doctest} foo

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured

",
        COMPILING, p.url(), running = RUNNING, doctest = DOCTEST)))
})

test!(dont_run_examples {
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
            fn main() { fail!("Examples should not be run by 'cargo test'"); }
        "#);
    assert_that(p.cargo_process("cargo-test"),
                execs().with_status(0));
})

test!(pass_through_command_line {
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

    assert_that(p.cargo_process("cargo-test").arg("bar"),
                execs().with_status(0)
                       .with_stdout(format!("\
{compiling} foo v0.0.1 ({dir})
{running} target[..]test[..]foo

running 1 test
test bar ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured

{doctest} foo

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured

",
                       compiling = COMPILING, running = RUNNING,
                       doctest = DOCTEST,
                       dir = p.url()).as_slice()));

    assert_that(p.cargo_process("cargo-test").arg("foo"),
                execs().with_status(0)
                       .with_stdout(format!("\
{compiling} foo v0.0.1 ({dir})
{running} target[..]test[..]foo

running 1 test
test foo ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured

{doctest} foo

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured

",
                       compiling = COMPILING, running = RUNNING,
                       doctest = DOCTEST,
                       dir = p.url()).as_slice()));
})

// Regression test for running cargo-test twice with
// tests in an rlib
test!(cargo_test_twice {
    let p = project("test_twice")
        .file("Cargo.toml", basic_lib_manifest("test_twice").as_slice())
        .file("src/test_twice.rs", r#"
            #![crate_type = "rlib"]

            #[test]
            fn dummy_test() { }
            "#);

    p.cargo_process("cargo-build");

    for _ in range(0u, 2) {
        assert_that(p.process(cargo_dir().join("cargo-test")),
                    execs().with_status(0));
    }
})

test!(lib_bin_same_name {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [[lib]]
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

    assert_that(p.cargo_process("cargo-test"),
        execs().with_stdout(format!("\
{} foo v0.0.1 ({})
{running} target[..]test[..]foo-[..]

running 1 test
test [..] ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured

{running} target[..]test[..]foo-[..]

running 1 test
test [..] ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured

{doctest} foo

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured

",
        COMPILING, p.url(), running = RUNNING, doctest = DOCTEST)))
})

test!(lib_with_standard_name {
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

    assert_that(p.cargo_process("cargo-test"),
                execs().with_status(0)
                       .with_stdout(format!("\
{compiling} syntax v0.0.1 ({dir})
{running} target[..]test[..]syntax-[..]

running 1 test
test foo_test ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured

{running} target[..]test[..]test-[..]

running 1 test
test test ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured

{doctest} syntax

running 1 test
test foo_0 ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured

",
                       compiling = COMPILING, running = RUNNING,
                       doctest = DOCTEST, dir = p.url()).as_slice()));
})

test!(lib_with_standard_name2 {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "syntax"
            version = "0.0.1"
            authors = []

            [[lib]]
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

    assert_that(p.cargo_process("cargo-test"),
                execs().with_status(0)
                       .with_stdout(format!("\
{compiling} syntax v0.0.1 ({dir})
{running} target[..]test[..]syntax-[..]

running 1 test
test test ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured

",
                       compiling = COMPILING, running = RUNNING,
                       dir = p.url()).as_slice()));
})

test!(bin_there_for_integration {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/main.rs", "
            fn main() { std::os::set_exit_status(1); }
            #[test] fn main_test() {}
        ")
        .file("tests/foo.rs", r#"
            use std::io::Command;
            #[test]
            fn test_test() {
                let status = Command::new("target/test/foo").status().unwrap();
                assert!(status.matches_exit_status(1));
            }
        "#);

    let output = p.cargo_process("cargo-test").exec_with_output().assert();
    let output = str::from_utf8(output.output.as_slice()).assert();
    assert!(output.contains("main_test ... ok"), "no main_test\n{}", output);
    assert!(output.contains("test_test ... ok"), "no test_test\n{}", output);
})

test!(test_dylib {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [[lib]]
            name = "foo"
            crate_type = ["dylib"]

            [dependencies.bar]
            path = "bar"
        "#)
        .file("src/lib.rs", "
            extern crate bar;

            pub fn bar() { bar::baz(); }

            #[test]
            fn foo() { bar(); }
        ")
        .file("tests/test.rs", r#"
            extern crate foo;

            #[test]
            fn foo() { foo::bar(); }
        "#)
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [[lib]]
            name = "bar"
            crate_type = ["dylib"]
        "#)
        .file("bar/src/lib.rs", "
             pub fn baz() {}
        ");

    assert_that(p.cargo_process("cargo-test"),
                execs().with_status(0)
                       .with_stdout(format!("\
{compiling} bar v0.0.1 ({dir})
{compiling} foo v0.0.1 ({dir})
{running} target[..]test[..]foo-[..]

running 1 test
test foo ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured

{running} target[..]test[..]test-[..]

running 1 test
test foo ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured

{doctest} foo

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured

",
                       compiling = COMPILING, running = RUNNING,
                       doctest = DOCTEST,
                       dir = p.url()).as_slice()));
    p.root().move_into_the_past().assert();
    assert_that(p.process(cargo_dir().join("cargo-test")),
                execs().with_status(0)
                       .with_stdout(format!("\
{fresh} bar v0.0.1 ({dir})
{fresh} foo v0.0.1 ({dir})
{running} target[..]test[..]foo-[..]

running 1 test
test foo ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured

{running} target[..]test[..]test-[..]

running 1 test
test foo ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured

{doctest} foo

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured

",
                       fresh = FRESH, running = RUNNING,
                       doctest = DOCTEST,
                       dir = p.url()).as_slice()));
})

test!(test_twice_with_build_cmd {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            build = 'true'
        "#)
        .file("src/lib.rs", "
            #[test]
            fn foo() {}
        ");

    assert_that(p.cargo_process("cargo-test"),
                execs().with_status(0)
                       .with_stdout(format!("\
{compiling} foo v0.0.1 ({dir})
{running} target[..]test[..]foo-[..]

running 1 test
test foo ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured

{doctest} foo

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured

",
                       compiling = COMPILING, running = RUNNING,
                       doctest = DOCTEST,
                       dir = p.url()).as_slice()));

    assert_that(p.process(cargo_dir().join("cargo-test")),
                execs().with_status(0)
                       .with_stdout(format!("\
{fresh} foo v0.0.1 ({dir})
{running} target[..]test[..]foo-[..]

running 1 test
test foo ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured

{doctest} foo

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured

",
                       fresh = FRESH, running = RUNNING,
                       doctest = DOCTEST,
                       dir = p.url()).as_slice()));
})
