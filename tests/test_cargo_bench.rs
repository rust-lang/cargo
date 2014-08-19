use std::path;
use std::str;

use support::{project, execs, basic_bin_manifest, basic_lib_manifest};
use support::{COMPILING, cargo_dir, ResultTest, FRESH, RUNNING, DOCTEST};
use support::paths::PathExt;
use hamcrest::{assert_that, existing_file};
use cargo::util::process;

fn setup() {}

test!(cargo_bench_simple {
    let p = project("foo")
        .file("Cargo.toml", basic_bin_manifest("foo").as_slice())
        .file("src/foo.rs", r#"
            extern crate test;

            fn hello() -> &'static str {
                "hello"
            }

            pub fn main() {
                println!("{}", hello())
            }

            #[bench]
            fn bench_hello(_b: &mut test::Bencher) {
                assert_eq!(hello(), "hello")
            }"#);

    assert_that(p.cargo_process("cargo-build"), execs());
    assert_that(&p.bin("foo"), existing_file());

    assert_that(
        process(p.bin("foo")),
        execs().with_stdout("hello\n"));

    assert_that(p.process(cargo_dir().join("cargo-bench")),
        execs().with_stdout(format!("\
{} foo v0.5.0 ({})
{} target[..]release[..]foo

running 1 test
test bench_hello ... bench:         0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

",
        COMPILING, p.url(),
        RUNNING)));
})

test!(cargo_bench_verbose {
    let p = project("foo")
        .file("Cargo.toml", basic_bin_manifest("foo").as_slice())
        .file("src/foo.rs", r#"
            extern crate test;
            fn main() {}
            #[bench] fn bench_hello(_b: &mut test::Bencher) {}
        "#);

    assert_that(p.cargo_process("cargo-bench").arg("-v").arg("hello"),
        execs().with_stdout(format!("\
{running} `rustc src[..]foo.rs [..]`
{compiling} foo v0.5.0 ({url})
{running} `[..]target[..]release[..]foo-[..] hello --bench`

running 1 test
test bench_hello ... bench:         0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

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
            extern crate test;
            pub fn foo() {}
            #[bench] fn lib_bench(_b: &mut test::Bencher) {}
        ")
        .file("src/main.rs", "
            extern crate foo;
            extern crate test;
            fn main() {}
            #[bench] fn bin_bench(_b: &mut test::Bencher) { foo::foo() }
        ")
        .file("benches/foo.rs", r#"
            extern crate foo;
            extern crate test;
            #[bench] fn bench_bench(_b: &mut test::Bencher) { foo::foo() }
        "#);

    let output = p.cargo_process("cargo-bench").exec_with_output().assert();
    let output = str::from_utf8(output.output.as_slice()).assert();
    assert!(output.contains("test bin_bench"), "bin_bench missing\n{}", output);
    assert!(output.contains("test lib_bench"), "lib_bench missing\n{}", output);
    assert!(output.contains("test bench_bench"), "bench_bench missing\n{}", output);
})

test!(cargo_bench_failing_test {
    let p = project("foo")
        .file("Cargo.toml", basic_bin_manifest("foo").as_slice())
        .file("src/foo.rs", r#"
            extern crate test;
            fn hello() -> &'static str {
                "hello"
            }

            pub fn main() {
                println!("{}", hello())
            }

            #[bench]
            fn bench_hello(_b: &mut test::Bencher) {
                assert_eq!(hello(), "nope")
            }"#);

    assert_that(p.cargo_process("cargo-build"), execs());
    assert_that(&p.bin("foo"), existing_file());

    assert_that(
        process(p.bin("foo")),
        execs().with_stdout("hello\n"));

    assert_that(p.process(cargo_dir().join("cargo-bench")),
        execs().with_stdout(format!("\
{} foo v0.5.0 ({})
{} target[..]release[..]foo

running 1 test
test bench_hello ... ",
        COMPILING, p.url(), RUNNING))
              .with_stderr(format!("\
task '<main>' failed at 'assertion failed: \
    `(left == right) && (right == left)` (left: \
    `hello`, right: `nope`)', src{sep}foo.rs:13
", sep = path::SEP))
              .with_status(101));
})

test!(bench_with_lib_dep {
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
            extern crate test;
            ///
            /// ```rust
            /// extern crate foo;
            /// fn main() {
            ///     println!("{}", foo::foo());
            /// }
            /// ```
            ///
            pub fn foo(){}
            #[bench] fn lib_bench(_b: &mut test::Bencher) {}
        "#)
        .file("src/main.rs", "
            extern crate foo;
            extern crate test;

            fn main() {}

            #[bench]
            fn bin_bench(_b: &mut test::Bencher) {}
        ");

    assert_that(p.cargo_process("cargo-bench"),
        execs().with_stdout(format!("\
{} foo v0.0.1 ({})
{running} target[..]release[..]baz-[..]

running 1 test
test bin_bench ... bench:         0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

{running} target[..]release[..]foo

running 1 test
test lib_bench ... bench:         0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

{doctest} foo

running 1 test
test foo_0 ... ignored

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured

",
        COMPILING, p.url(), running = RUNNING, doctest = DOCTEST)))
})

test!(bench_with_deep_lib_dep {
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
            extern crate test;
            #[bench]
            fn bar_bench(_b: &mut test::Bencher) {
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
            extern crate test;

            pub fn foo() {}

            #[bench]
            fn foo_bench(_b: &mut test::Bencher) {}
        ");

    p2.build();
    assert_that(p.cargo_process("cargo-bench"),
                execs().with_status(0)
                       .with_stdout(format!("\
{compiling} foo v0.0.1 ({dir})
{compiling} bar v0.0.1 ({dir})
{running} target[..]

running 1 test
test bar_bench ... bench:         0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

{doctest} bar

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured

",
                       compiling = COMPILING, running = RUNNING,
                       doctest = DOCTEST,
                       dir = p.url()).as_slice()));
})

test!(external_bench_explicit {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [[bench]]
            name = "bench"
            path = "src/bench.rs"
        "#)
        .file("src/lib.rs", r#"
            extern crate test;
            pub fn get_hello() -> &'static str { "Hello" }

            #[bench]
            fn internal_bench(_b: &mut test::Bencher) {}
        "#)
        .file("src/bench.rs", r#"
            extern crate foo;
            extern crate test;

            #[bench]
            fn external_bench(_b: &mut test::Bencher) {}
        "#);

    assert_that(p.cargo_process("cargo-bench"),
        execs().with_stdout(format!("\
{} foo v0.0.1 ({})
{running} target[..]release[..]bench-[..]

running 1 test
test external_bench ... bench:         0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

{running} target[..]release[..]foo-[..]

running 1 test
test internal_bench ... bench:         0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

{doctest} foo

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured

",
        COMPILING, p.url(), running = RUNNING, doctest = DOCTEST)))
})

test!(external_bench_implicit {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", r#"
            extern crate test;

            pub fn get_hello() -> &'static str { "Hello" }

            #[bench]
            fn internal_bench(_b: &mut test::Bencher) {}
        "#)
        .file("benches/external.rs", r#"
            extern crate foo;
            extern crate test;

            #[bench]
            fn external_bench(_b: &mut test::Bencher) {}
        "#);

    assert_that(p.cargo_process("cargo-bench"),
        execs().with_stdout(format!("\
{} foo v0.0.1 ({})
{running} target[..]release[..]external-[..]

running 1 test
test external_bench ... bench:         0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

{running} target[..]release[..]foo-[..]

running 1 test
test internal_bench ... bench:         0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

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
    assert_that(p.cargo_process("cargo-bench"),
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
            extern crate test;

            #[bench] fn foo(_b: &mut test::Bencher) {}
            #[bench] fn bar(_b: &mut test::Bencher) {}
        ");

    assert_that(p.cargo_process("cargo-bench").arg("bar"),
                execs().with_status(0)
                       .with_stdout(format!("\
{compiling} foo v0.0.1 ({dir})
{running} target[..]release[..]foo

running 1 test
test bar ... bench:         0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

{doctest} foo

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured

",
                       compiling = COMPILING, running = RUNNING,
                       doctest = DOCTEST,
                       dir = p.url()).as_slice()));

    assert_that(p.cargo_process("cargo-bench").arg("foo"),
                execs().with_status(0)
                       .with_stdout(format!("\
{compiling} foo v0.0.1 ({dir})
{running} target[..]release[..]foo

running 1 test
test foo ... bench:         0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

{doctest} foo

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured

",
                       compiling = COMPILING, running = RUNNING,
                       doctest = DOCTEST,
                       dir = p.url()).as_slice()));
})

// Regression test for running cargo-bench twice with
// tests in an rlib
test!(cargo_bench_twice {
    let p = project("test_twice")
        .file("Cargo.toml", basic_lib_manifest("test_twice").as_slice())
        .file("src/test_twice.rs", r#"
            #![crate_type = "rlib"]

            extern crate test;

            #[bench]
            fn dummy_bench(b: &mut test::Bencher) { }
            "#);

    p.cargo_process("cargo-build");

    for _ in range(0u, 2) {
        assert_that(p.process(cargo_dir().join("cargo-bench")),
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

            [lib]
            name = "foo"
            [[bin]]
            name = "foo"
        "#)
        .file("src/lib.rs", "
            extern crate test;
            #[bench] fn lib_bench(_b: &mut test::Bencher) {}
        ")
        .file("src/main.rs", "
            extern crate foo;
            extern crate test;

            #[bench]
            fn bin_bench(_b: &mut test::Bencher) {}
        ");

    assert_that(p.cargo_process("cargo-bench"),
        execs().with_stdout(format!("\
{} foo v0.0.1 ({})
{running} target[..]release[..]foo-[..]

running 1 test
test [..] ... bench:         0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

{running} target[..]release[..]foo-[..]

running 1 test
test [..] ... bench:         0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

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
            extern crate test;

            /// ```
            /// syntax::foo();
            /// ```
            pub fn foo() {}

            #[bench]
            fn foo_bench(_b: &mut test::Bencher) {}
        ")
        .file("benches/bench.rs", "
            extern crate syntax;
            extern crate test;

            #[bench]
            fn bench(_b: &mut test::Bencher) { syntax::foo() }
        ");

    assert_that(p.cargo_process("cargo-bench"),
                execs().with_status(0)
                       .with_stdout(format!("\
{compiling} syntax v0.0.1 ({dir})
{running} target[..]release[..]bench-[..]

running 1 test
test bench ... bench:         0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

{running} target[..]release[..]syntax-[..]

running 1 test
test foo_bench ... bench:         0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

{doctest} syntax

running 1 test
test foo_0 ... ignored

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured

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

            [lib]
            name = "syntax"
            bench = false
            doctest = false
        "#)
        .file("src/lib.rs", "
            pub fn foo() {}
        ")
        .file("src/main.rs", "
            extern crate syntax;
            extern crate test;

            fn main() {}

            #[bench]
            fn bench(_b: &mut test::Bencher) { syntax::foo() }
        ");

    assert_that(p.cargo_process("cargo-bench"),
                execs().with_status(0)
                       .with_stdout(format!("\
{compiling} syntax v0.0.1 ({dir})
{running} target[..]release[..]syntax-[..]

running 1 test
test bench ... bench:         0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

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
            extern crate test;
            fn main() { std::os::set_exit_status(1); }
            #[bench] fn main_bench(_b: &mut test::Bencher) {}
        ")
        .file("benches/foo.rs", r#"
            extern crate test;
            use std::io::Command;
            #[bench]
            fn bench_bench(_b: &mut test::Bencher) {
                let status = Command::new("target/release/foo").status().unwrap();
                assert!(status.matches_exit_status(1));
            }
        "#);

    let output = p.cargo_process("cargo-bench").exec_with_output().assert();
    let output = str::from_utf8(output.output.as_slice()).assert();
    assert!(output.contains("main_bench ... bench:         0 ns/iter (+/- 0)"),
                            "no main_bench\n{}",
                            output);
    assert!(output.contains("bench_bench ... bench:         0 ns/iter (+/- 0)"),
                            "no bench_bench\n{}",
                            output);
})

test!(bench_dylib {
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
            extern crate the_bar = "bar";
            extern crate test;

            pub fn bar() { the_bar::baz(); }

            #[bench]
            fn foo(_b: &mut test::Bencher) {}
        "#)
        .file("benches/bench.rs", r#"
            extern crate the_foo = "foo";
            extern crate test;

            #[bench]
            fn foo(_b: &mut test::Bencher) { the_foo::bar(); }
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

    assert_that(p.cargo_process("cargo-bench"),
                execs().with_status(0)
                       .with_stdout(format!("\
{compiling} bar v0.0.1 ({dir})
{compiling} foo v0.0.1 ({dir})
{running} target[..]release[..]bench-[..]

running 1 test
test foo ... bench:         0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

{running} target[..]release[..]foo-[..]

running 1 test
test foo ... bench:         0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

{doctest} foo

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured

",
                       compiling = COMPILING, running = RUNNING,
                       doctest = DOCTEST,
                       dir = p.url()).as_slice()));
    p.root().move_into_the_past().assert();
    assert_that(p.process(cargo_dir().join("cargo-bench")),
                execs().with_status(0)
                       .with_stdout(format!("\
{fresh} bar v0.0.1 ({dir})
{fresh} foo v0.0.1 ({dir})
{running} target[..]release[..]bench-[..]

running 1 test
test foo ... bench:         0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

{running} target[..]release[..]foo-[..]

running 1 test
test foo ... bench:         0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

{doctest} foo

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured

",
                       fresh = FRESH, running = RUNNING,
                       doctest = DOCTEST,
                       dir = p.url()).as_slice()));
})

test!(bench_twice_with_build_cmd {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            build = 'true'
        "#)
        .file("src/lib.rs", "
            extern crate test;
            #[bench]
            fn foo(_b: &mut test::Bencher) {}
        ");

    assert_that(p.cargo_process("cargo-bench"),
                execs().with_status(0)
                       .with_stdout(format!("\
{compiling} foo v0.0.1 ({dir})
{running} target[..]release[..]foo-[..]

running 1 test
test foo ... bench:         0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

{doctest} foo

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured

",
                       compiling = COMPILING, running = RUNNING,
                       doctest = DOCTEST,
                       dir = p.url()).as_slice()));

    assert_that(p.process(cargo_dir().join("cargo-bench")),
                execs().with_status(0)
                       .with_stdout(format!("\
{fresh} foo v0.0.1 ({dir})
{running} target[..]release[..]foo-[..]

running 1 test
test foo ... bench:         0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

{doctest} foo

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured

",
                       fresh = FRESH, running = RUNNING,
                       doctest = DOCTEST,
                       dir = p.url()).as_slice()));
})
