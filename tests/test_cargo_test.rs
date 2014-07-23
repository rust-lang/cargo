use std::path;
use std::str;

use support::{project, execs, basic_bin_manifest, basic_lib_manifest};
use support::{COMPILING, cargo_dir, ResultTest};
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
        execs().with_stdout(format!("{} foo v0.5.0 (file:{})\n\n\
                                    running 1 test\n\
                                    test test_hello ... ok\n\n\
                                    test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured\n\n",
                                    COMPILING, p.root().display())));
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
        execs().with_stdout(format!("{} foo v0.5.0 (file:{})\n\n\
                                    running 1 test\n\
                                    test test_hello ... FAILED\n\n\
                                    failures:\n\n\
                                    ---- test_hello stdout ----\n<tab>\
                                    task 'test_hello' failed at 'assertion failed: \
                                    `(left == right) && (right == left)` (left: \
                                    `hello`, right: `nope`)', src{sep}foo.rs:12\n<tab>\n<tab>\n\n\
                                    failures:\n    test_hello\n\n\
                                    test result: FAILED. 0 passed; 1 failed; \
                                    0 ignored; 0 measured\n\n",
                                    COMPILING, p.root().display(),
                                    sep = path::SEP))
              .with_stderr(format!("\
task '<main>' failed at 'Some tests failed', [..]
Could not execute process `{test}[..]` (status=101)
", test = p.root().join("target/test/foo").display()))
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
        .file("src/lib.rs", "
            pub fn foo(){}
            #[test] fn lib_test() {}
        ")
        .file("src/main.rs", "
            extern crate foo;

            fn main() {}

            #[test]
            fn bin_test() {}
        ");

    let output = p.cargo_process("cargo-test")
                  .exec_with_output().assert();
    let out = str::from_utf8(output.output.as_slice()).assert();

    let bin = "\
running 1 test
test bin_test ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured";
    let lib = "\
running 1 test
test lib_test ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured";

    let head = format!("{compiling} foo v0.0.1 (file:{dir})",
                       compiling = COMPILING, dir = p.root().display());

    assert!(out == format!("{}\n\n{}\n\n\n{}\n\n", head, bin, lib).as_slice() ||
            out == format!("{}\n\n{}\n\n\n{}\n\n", head, lib, bin).as_slice());
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
{compiling} foo v0.0.1 (file:{dir})
{compiling} bar v0.0.1 (file:{dir})

running 1 test
test bar_test ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured\n\n\
                       ",
                       compiling = COMPILING,
                       dir = p.root().display()).as_slice()));
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

    let output = p.cargo_process("cargo-test")
                  .exec_with_output().assert();
    let out = str::from_utf8(output.output.as_slice()).assert();

    let internal = "\
running 1 test
test internal_test ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured";
    let external = "\
running 1 test
test external_test ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured";

    let head = format!("{compiling} foo v0.0.1 (file:{dir})",
                       compiling = COMPILING, dir = p.root().display());

    assert!(out == format!("{}\n\n{}\n\n\n{}\n\n", head, internal, external).as_slice() ||
            out == format!("{}\n\n{}\n\n\n{}\n\n", head, external, internal).as_slice());
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

    let output = p.cargo_process("cargo-test")
                  .exec_with_output().assert();
    let out = str::from_utf8(output.output.as_slice()).assert();

    let internal = "\
running 1 test
test internal_test ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured";
    let external = "\
running 1 test
test external_test ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured";

    let head = format!("{compiling} foo v0.0.1 (file:{dir})",
                       compiling = COMPILING, dir = p.root().display());

    assert!(out == format!("{}\n\n{}\n\n\n{}\n\n", head, internal, external).as_slice() ||
            out == format!("{}\n\n{}\n\n\n{}\n\n", head, external, internal).as_slice());
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
{compiling} foo v0.0.1 (file:{dir})

running 1 test
test bar ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured\n\n\
                       ",
                       compiling = COMPILING,
                       dir = p.root().display()).as_slice()));

    assert_that(p.cargo_process("cargo-test").arg("foo"),
                execs().with_status(0)
                       .with_stdout(format!("\
{compiling} foo v0.0.1 (file:{dir})

running 1 test
test foo ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured\n\n\
                       ",
                       compiling = COMPILING,
                       dir = p.root().display()).as_slice()));
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

    let output = p.cargo_process("cargo-test")
                  .exec_with_output().assert();
    let out = str::from_utf8(output.output.as_slice()).assert();

    let bin = "\
running 1 test
test bin_test ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured";
    let lib = "\
running 1 test
test lib_test ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured";

    let head = format!("{compiling} foo v0.0.1 (file:{dir})",
                       compiling = COMPILING, dir = p.root().display());

    assert!(out == format!("{}\n\n{}\n\n\n{}\n\n", head, bin, lib).as_slice() ||
            out == format!("{}\n\n{}\n\n\n{}\n\n", head, lib, bin).as_slice());
})

test!(lib_with_standard_name {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "syntax"
            version = "0.0.1"
            authors = []

            [[lib]]
            name = "syntax"
            test = false
        "#)
        .file("src/lib.rs", "
            pub fn foo() {}
        ")
        .file("tests/test.rs", "
            extern crate syntax;

            #[test]
            fn test() { syntax::foo() }
        ");

    assert_that(p.cargo_process("cargo-test"),
                execs().with_status(0)
                       .with_stdout(format!("\
{compiling} syntax v0.0.1 (file:{dir})

running 1 test
test test ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured\n\n\
                       ",
                       compiling = COMPILING,
                       dir = p.root().display()).as_slice()));
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
{compiling} syntax v0.0.1 (file:{dir})

running 1 test
test test ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured\n\n\
                       ",
                       compiling = COMPILING,
                       dir = p.root().display()).as_slice()));
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
