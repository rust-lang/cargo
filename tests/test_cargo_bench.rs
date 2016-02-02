use std::str;

use support::{project, execs, basic_bin_manifest, basic_lib_manifest};
use support::{COMPILING, FRESH, RUNNING};
use support::paths::CargoPathExt;
use hamcrest::{assert_that, existing_file};
use cargo::util::process;

fn setup() {}

test!(cargo_bench_simple {
    if !::is_nightly() { return }

    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", r#"
            #![feature(test)]
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

    assert_that(p.cargo_process("build"), execs());
    assert_that(&p.bin("foo"), existing_file());

    assert_that(process(&p.bin("foo")),
                execs().with_stdout("hello\n"));

    assert_that(p.cargo("bench"),
                execs().with_stdout(&format!("\
{} foo v0.5.0 ({})
{} target[..]release[..]foo-[..]

running 1 test
test bench_hello ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

",
        COMPILING, p.url(),
        RUNNING)));
});

test!(bench_tarname {
    if !::is_nightly() { return }

    let prj = project("foo")
        .file("Cargo.toml" , r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("benches/bin1.rs", r#"
            #![feature(test)]
            extern crate test;
            #[bench] fn run1(_ben: &mut test::Bencher) { }"#)
        .file("benches/bin2.rs", r#"
            #![feature(test)]
            extern crate test;
            #[bench] fn run2(_ben: &mut test::Bencher) { }"#);

    let expected_stdout = format!("\
{compiling} foo v0.0.1 ({dir})
{running} target[..]release[..]bin2[..]

running 1 test
test run2 ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

",
       compiling = COMPILING,
       running = RUNNING,
       dir = prj.url());

    assert_that(prj.cargo_process("bench").arg("--bench").arg("bin2"),
        execs().with_status(0).with_stdout(expected_stdout));
});

test!(cargo_bench_verbose {
    if !::is_nightly() { return }

    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", r#"
            #![feature(test)]
            extern crate test;
            fn main() {}
            #[bench] fn bench_hello(_b: &mut test::Bencher) {}
        "#);

    assert_that(p.cargo_process("bench").arg("-v").arg("hello"),
        execs().with_stdout(&format!("\
{compiling} foo v0.5.0 ({url})
{running} `rustc src[..]foo.rs [..]`
{running} `[..]target[..]release[..]foo-[..] hello --bench`

running 1 test
test bench_hello ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

",
        compiling = COMPILING, url = p.url(), running = RUNNING)));
});

test!(many_similar_names {
    if !::is_nightly() { return }

    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "
            #![feature(test)]
            extern crate test;
            pub fn foo() {}
            #[bench] fn lib_bench(_b: &mut test::Bencher) {}
        ")
        .file("src/main.rs", "
            #![feature(test)]
            extern crate foo;
            extern crate test;
            fn main() {}
            #[bench] fn bin_bench(_b: &mut test::Bencher) { foo::foo() }
        ")
        .file("benches/foo.rs", r#"
            #![feature(test)]
            extern crate foo;
            extern crate test;
            #[bench] fn bench_bench(_b: &mut test::Bencher) { foo::foo() }
        "#);

    let output = p.cargo_process("bench").exec_with_output().unwrap();
    let output = str::from_utf8(&output.stdout).unwrap();
    assert!(output.contains("test bin_bench"), "bin_bench missing\n{}", output);
    assert!(output.contains("test lib_bench"), "lib_bench missing\n{}", output);
    assert!(output.contains("test bench_bench"), "bench_bench missing\n{}", output);
});

test!(cargo_bench_failing_test {
    if !::is_nightly() { return }
    if !::can_panic() { return }

    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", r#"
            #![feature(test)]
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

    assert_that(p.cargo_process("build"), execs());
    assert_that(&p.bin("foo"), existing_file());

    assert_that(process(&p.bin("foo")),
                execs().with_stdout("hello\n"));

    assert_that(p.cargo("bench"),
                execs().with_stdout_contains(&format!("\
{} foo v0.5.0 ({})
{} target[..]release[..]foo-[..]

running 1 test
test bench_hello ... ",
        COMPILING, p.url(), RUNNING))
              .with_stderr_contains("\
thread '<main>' panicked at 'assertion failed: \
    `(left == right)` (left: \
    `\"hello\"`, right: `\"nope\"`)', src[..]foo.rs:14
[..]
")
              .with_status(101));
});

test!(bench_with_lib_dep {
    if !::is_nightly() { return }

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
            #![feature(test)]
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
            #![feature(test)]
            extern crate foo;
            extern crate test;

            fn main() {}

            #[bench]
            fn bin_bench(_b: &mut test::Bencher) {}
        ");

    assert_that(p.cargo_process("bench"),
        execs().with_stdout(&format!("\
{} foo v0.0.1 ({})
{running} target[..]release[..]baz-[..]

running 1 test
test bin_bench ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

{running} target[..]release[..]foo-[..]

running 1 test
test lib_bench ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

",
        COMPILING, p.url(), running = RUNNING)))
});

test!(bench_with_deep_lib_dep {
    if !::is_nightly() { return }

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
            #![feature(test)]
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
            #![feature(test)]
            extern crate test;

            pub fn foo() {}

            #[bench]
            fn foo_bench(_b: &mut test::Bencher) {}
        ");

    p2.build();
    assert_that(p.cargo_process("bench"),
                execs().with_status(0)
                       .with_stdout(&format!("\
{compiling} foo v0.0.1 ({dir})
{compiling} bar v0.0.1 ({dir})
{running} target[..]

running 1 test
test bar_bench ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

",
                       compiling = COMPILING, running = RUNNING,
                       dir = p.url())));
});

test!(external_bench_explicit {
    if !::is_nightly() { return }

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
            #![feature(test)]
            extern crate test;
            pub fn get_hello() -> &'static str { "Hello" }

            #[bench]
            fn internal_bench(_b: &mut test::Bencher) {}
        "#)
        .file("src/bench.rs", r#"
            #![feature(test)]
            extern crate foo;
            extern crate test;

            #[bench]
            fn external_bench(_b: &mut test::Bencher) {}
        "#);

    assert_that(p.cargo_process("bench"),
        execs().with_stdout(&format!("\
{} foo v0.0.1 ({})
{running} target[..]release[..]bench-[..]

running 1 test
test external_bench ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

{running} target[..]release[..]foo-[..]

running 1 test
test internal_bench ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

",
        COMPILING, p.url(), running = RUNNING)))
});

test!(external_bench_implicit {
    if !::is_nightly() { return }

    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", r#"
            #![feature(test)]
            extern crate test;

            pub fn get_hello() -> &'static str { "Hello" }

            #[bench]
            fn internal_bench(_b: &mut test::Bencher) {}
        "#)
        .file("benches/external.rs", r#"
            #![feature(test)]
            extern crate foo;
            extern crate test;

            #[bench]
            fn external_bench(_b: &mut test::Bencher) {}
        "#);

    assert_that(p.cargo_process("bench"),
        execs().with_stdout(&format!("\
{} foo v0.0.1 ({})
{running} target[..]release[..]external-[..]

running 1 test
test external_bench ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

{running} target[..]release[..]foo-[..]

running 1 test
test internal_bench ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

",
        COMPILING, p.url(), running = RUNNING)))
});

test!(dont_run_examples {
    if !::is_nightly() { return }

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
    assert_that(p.cargo_process("bench"),
                execs().with_status(0));
});

test!(pass_through_command_line {
    if !::is_nightly() { return }

    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "
            #![feature(test)]
            extern crate test;

            #[bench] fn foo(_b: &mut test::Bencher) {}
            #[bench] fn bar(_b: &mut test::Bencher) {}
        ");

    assert_that(p.cargo_process("bench").arg("bar"),
                execs().with_status(0)
                       .with_stdout(&format!("\
{compiling} foo v0.0.1 ({dir})
{running} target[..]release[..]foo-[..]

running 1 test
test bar ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

",
                       compiling = COMPILING, running = RUNNING,
                       dir = p.url())));

    assert_that(p.cargo("bench").arg("foo"),
                execs().with_status(0)
                       .with_stdout(&format!("\
{running} target[..]release[..]foo-[..]

running 1 test
test foo ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

", running = RUNNING)));
});

// Regression test for running cargo-bench twice with
// tests in an rlib
test!(cargo_bench_twice {
    if !::is_nightly() { return }

    let p = project("test_twice")
        .file("Cargo.toml", &basic_lib_manifest("test_twice"))
        .file("src/test_twice.rs", r#"
            #![crate_type = "rlib"]
            #![feature(test)]

            extern crate test;

            #[bench]
            fn dummy_bench(b: &mut test::Bencher) { }
            "#);

    p.cargo_process("build");

    for _ in 0..2 {
        assert_that(p.cargo("bench"),
                    execs().with_status(0));
    }
});

test!(lib_bin_same_name {
    if !::is_nightly() { return }

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
            #![feature(test)]
            extern crate test;
            #[bench] fn lib_bench(_b: &mut test::Bencher) {}
        ")
        .file("src/main.rs", "
            #![feature(test)]
            extern crate foo;
            extern crate test;

            #[bench]
            fn bin_bench(_b: &mut test::Bencher) {}
        ");

    assert_that(p.cargo_process("bench"),
        execs().with_stdout(&format!("\
{} foo v0.0.1 ({})
{running} target[..]release[..]foo-[..]

running 1 test
test [..] ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

{running} target[..]release[..]foo-[..]

running 1 test
test [..] ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

",
        COMPILING, p.url(), running = RUNNING)))
});

test!(lib_with_standard_name {
    if !::is_nightly() { return }

    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "syntax"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "
            #![feature(test)]
            extern crate test;

            /// ```
            /// syntax::foo();
            /// ```
            pub fn foo() {}

            #[bench]
            fn foo_bench(_b: &mut test::Bencher) {}
        ")
        .file("benches/bench.rs", "
            #![feature(test)]
            extern crate syntax;
            extern crate test;

            #[bench]
            fn bench(_b: &mut test::Bencher) { syntax::foo() }
        ");

    assert_that(p.cargo_process("bench"),
                execs().with_status(0)
                       .with_stdout(&format!("\
{compiling} syntax v0.0.1 ({dir})
{running} target[..]release[..]bench-[..]

running 1 test
test bench ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

{running} target[..]release[..]syntax-[..]

running 1 test
test foo_bench ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

",
                       compiling = COMPILING, running = RUNNING,
                       dir = p.url())));
});

test!(lib_with_standard_name2 {
    if !::is_nightly() { return }

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
            #![feature(test)]
            extern crate syntax;
            extern crate test;

            fn main() {}

            #[bench]
            fn bench(_b: &mut test::Bencher) { syntax::foo() }
        ");

    assert_that(p.cargo_process("bench"),
                execs().with_status(0)
                       .with_stdout(&format!("\
{compiling} syntax v0.0.1 ({dir})
{running} target[..]release[..]syntax-[..]

running 1 test
test bench ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

",
                       compiling = COMPILING, running = RUNNING,
                       dir = p.url())));
});

test!(bench_dylib {
    if !::is_nightly() { return }

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
            #![feature(test)]
            extern crate bar as the_bar;
            extern crate test;

            pub fn bar() { the_bar::baz(); }

            #[bench]
            fn foo(_b: &mut test::Bencher) {}
        "#)
        .file("benches/bench.rs", r#"
            #![feature(test)]
            extern crate foo as the_foo;
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

    assert_that(p.cargo_process("bench").arg("-v"),
                execs().with_status(0)
                       .with_stdout(&format!("\
{compiling} bar v0.0.1 ({dir})
{running} [..] -C opt-level=3 [..]
{compiling} foo v0.0.1 ({dir})
{running} [..] -C opt-level=3 [..]
{running} [..] -C opt-level=3 [..]
{running} [..] -C opt-level=3 [..]
{running} [..]target[..]release[..]bench-[..]

running 1 test
test foo ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

{running} [..]target[..]release[..]foo-[..]

running 1 test
test foo ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

",
                       compiling = COMPILING, running = RUNNING,
                       dir = p.url())));
    p.root().move_into_the_past().unwrap();
    assert_that(p.cargo("bench").arg("-v"),
                execs().with_status(0)
                       .with_stdout(&format!("\
{fresh} bar v0.0.1 ({dir})
{fresh} foo v0.0.1 ({dir})
{running} [..]target[..]release[..]bench-[..]

running 1 test
test foo ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

{running} [..]target[..]release[..]foo-[..]

running 1 test
test foo ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

",
                       fresh = FRESH, running = RUNNING,
                       dir = p.url())));
});

test!(bench_twice_with_build_cmd {
    if !::is_nightly() { return }

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
            #![feature(test)]
            extern crate test;
            #[bench]
            fn foo(_b: &mut test::Bencher) {}
        ");

    assert_that(p.cargo_process("bench"),
                execs().with_status(0)
                       .with_stdout(&format!("\
{compiling} foo v0.0.1 ({dir})
{running} target[..]release[..]foo-[..]

running 1 test
test foo ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

",
                       compiling = COMPILING, running = RUNNING,
                       dir = p.url())));

    assert_that(p.cargo("bench"),
                execs().with_status(0)
                       .with_stdout(&format!("\
{running} target[..]release[..]foo-[..]

running 1 test
test foo ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

",
                       running = RUNNING)));
});

test!(bench_with_examples {
    if !::is_nightly() { return }

    let p = project("testbench")
        .file("Cargo.toml", r#"
            [package]
            name = "testbench"
            version = "6.6.6"
            authors = []

            [[example]]
            name = "teste1"

            [[bench]]
            name = "testb1"
        "#)
        .file("src/lib.rs", r#"
            #![feature(test)]
            extern crate test;
            use test::Bencher;

            pub fn f1() {
                println!("f1");
            }

            pub fn f2() {}

            #[bench]
            fn bench_bench1(_b: &mut Bencher) {
                f2();
            }
        "#)
        .file("benches/testb1.rs", "
            #![feature(test)]
            extern crate testbench;
            extern crate test;

            use test::Bencher;

            #[bench]
            fn bench_bench2(_b: &mut Bencher) {
                testbench::f2();
            }
        ")
        .file("examples/teste1.rs", r#"
            extern crate testbench;

            fn main() {
                println!("example1");
                testbench::f1();
            }
        "#);

    assert_that(p.cargo_process("bench").arg("-v"),
                execs().with_status(0)
                       .with_stdout(&format!("\
{compiling} testbench v6.6.6 ({url})
{running} `rustc [..]`
{running} `rustc [..]`
{running} `rustc [..]`
{running} `{dir}[..]target[..]release[..]testb1-[..] --bench`

running 1 test
test bench_bench2 ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

{running} `{dir}[..]target[..]release[..]testbench-[..] --bench`

running 1 test
test bench_bench1 ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

",
                       compiling = COMPILING,
                       running = RUNNING,
                       dir = p.root().display(),
                       url = p.url())));
});

test!(test_a_bench {
    if !::is_nightly() { return }

    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            authors = []
            version = "0.1.0"

            [lib]
            name = "foo"
            test = false
            doctest = false

            [[bench]]
            name = "b"
            test = true
        "#)
        .file("src/lib.rs", "")
        .file("benches/b.rs", r#"
            #[test]
            fn foo() {}
        "#);

    assert_that(p.cargo_process("test"),
                execs().with_status(0)
                       .with_stdout(&format!("\
{compiling} foo v0.1.0 ([..])
{running} target[..]debug[..]b-[..]

running 1 test
test foo ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured

", compiling = COMPILING, running = RUNNING)));
});

test!(test_bench_no_run {
    if !::is_nightly() { return }

    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            authors = []
            version = "0.1.0"
        "#)
        .file("src/lib.rs", "")
        .file("benches/bbaz.rs", r#"
            #![feature(test)] 

            extern crate test;

            use test::Bencher;

            #[bench]
            fn bench_baz(_: &mut Bencher) {}
        "#);

    assert_that(p.cargo_process("bench").arg("--no-run"),
                execs().with_status(0)
                       .with_stdout(&format!("\
{compiling} foo v0.1.0 ([..])
", compiling = COMPILING)));
});

test!(test_bench_multiple_packages {
    if !::is_nightly() { return }

    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            authors = []
            version = "0.1.0"

            [dependencies.bar]
            path = "../bar"

            [dependencies.baz]
            path = "../baz"
        "#)
        .file("src/lib.rs", "");

    let bar = project("bar")
        .file("Cargo.toml", r#"
            [project]
            name = "bar"
            authors = []
            version = "0.1.0"

            [[bench]]
            name = "bbar"
            test = true
        "#)
        .file("src/lib.rs", "")
        .file("benches/bbar.rs", r#"
            #![feature(test)]
            extern crate test;

            use test::Bencher;

            #[bench]
            fn bench_bar(_b: &mut Bencher) {}
        "#);
    bar.build();

    let baz = project("baz")
        .file("Cargo.toml", r#"
            [project]
            name = "baz"
            authors = []
            version = "0.1.0"

            [[bench]]
            name = "bbaz"
            test = true
        "#)
        .file("src/lib.rs", "")
        .file("benches/bbaz.rs", r#"
            #![feature(test)]
            extern crate test;

            use test::Bencher;

            #[bench]
            fn bench_baz(_b: &mut Bencher) {}
        "#);
    baz.build();


    assert_that(p.cargo_process("bench").arg("-p").arg("bar").arg("-p").arg("baz"),
                execs().with_status(0)
                       .with_stdout_contains(&format!("\
{running} target[..]release[..]bbaz-[..]

running 1 test
test bench_baz ... bench:           0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured
", running = RUNNING))
                       .with_stdout_contains(&format!("\
{running} target[..]release[..]bbar-[..]

running 1 test
test bench_bar ... bench:           0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured
", running = RUNNING)));
});
