extern crate cargotest;
extern crate cargo;
extern crate hamcrest;

use std::str;

use cargo::util::process;
use cargotest::is_nightly;
use cargotest::support::paths::CargoPathExt;
use cargotest::support::{project, execs, basic_bin_manifest, basic_lib_manifest};
use hamcrest::{assert_that, existing_file};

#[test]
fn cargo_bench_simple() {
    if !is_nightly() { return }

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
                execs().with_stderr(&format!("\
[COMPILING] foo v0.5.0 ({})
[FINISHED] release [optimized] target(s) in [..]
[RUNNING] target[..]release[..]foo-[..][EXE]", p.url()))
                       .with_stdout("
running 1 test
test bench_hello ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

"));
}

#[test]
fn bench_tarname() {
    if !is_nightly() { return }

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

    assert_that(prj.cargo_process("bench").arg("--bench").arg("bin2"),
        execs().with_status(0)
               .with_stderr(format!("\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] release [optimized] target(s) in [..]
[RUNNING] target[..]release[..]bin2-[..][EXE]
", dir = prj.url()))
               .with_stdout("
running 1 test
test run2 ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

"));
}

#[test]
fn cargo_bench_verbose() {
    if !is_nightly() { return }

    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", r#"
            #![feature(test)]
            extern crate test;
            fn main() {}
            #[bench] fn bench_hello(_b: &mut test::Bencher) {}
        "#);

    assert_that(p.cargo_process("bench").arg("-v").arg("hello"),
                execs().with_stderr(&format!("\
[COMPILING] foo v0.5.0 ({url})
[RUNNING] `rustc src[..]foo.rs [..]`
[FINISHED] release [optimized] target(s) in [..]
[RUNNING] `[..]target[..]release[..]foo-[..][EXE] hello --bench`", url = p.url()))
                       .with_stdout("
running 1 test
test bench_hello ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

"));
}

#[test]
fn many_similar_names() {
    if !is_nightly() { return }

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
}

#[test]
fn cargo_bench_failing_test() {
    if !is_nightly() { return }

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
                execs().with_stdout_contains("
running 1 test
test bench_hello ... ")
                       .with_stderr_contains(format!("\
[COMPILING] foo v0.5.0 ({})
[FINISHED] release [optimized] target(s) in [..]
[RUNNING] target[..]release[..]foo-[..][EXE]
thread '[..]' panicked at 'assertion failed: \
    `(left == right)` (left: \
    `\"hello\"`, right: `\"nope\"`)', src[..]foo.rs:14
[..]
", p.url()))
                       .with_status(101));
}

#[test]
fn bench_with_lib_dep() {
    if !is_nightly() { return }

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
                execs().with_stderr(&format!("\
[COMPILING] foo v0.0.1 ({})
[FINISHED] release [optimized] target(s) in [..]
[RUNNING] target[..]release[..]baz-[..][EXE]
[RUNNING] target[..]release[..]foo-[..][EXE]", p.url()))
                       .with_stdout("
running 1 test
test bin_bench ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured


running 1 test
test lib_bench ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

"))
}

#[test]
fn bench_with_deep_lib_dep() {
    if !is_nightly() { return }

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
                       .with_stderr(&format!("\
[COMPILING] foo v0.0.1 ([..])
[COMPILING] bar v0.0.1 ({dir})
[FINISHED] release [optimized] target(s) in [..]
[RUNNING] target[..]release[..]deps[..]bar-[..][EXE]", dir = p.url()))
                       .with_stdout("
running 1 test
test bar_bench ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

"));
}

#[test]
fn external_bench_explicit() {
    if !is_nightly() { return }

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
                execs().with_stderr(&format!("\
[COMPILING] foo v0.0.1 ({})
[FINISHED] release [optimized] target(s) in [..]
[RUNNING] target[..]release[..]bench-[..][EXE]
[RUNNING] target[..]release[..]foo-[..][EXE]", p.url()))
                       .with_stdout("
running 1 test
test external_bench ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured


running 1 test
test internal_bench ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

"))
}

#[test]
fn external_bench_implicit() {
    if !is_nightly() { return }

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
                execs().with_stderr(&format!("\
[COMPILING] foo v0.0.1 ({})
[FINISHED] release [optimized] target(s) in [..]
[RUNNING] target[..]release[..]external-[..][EXE]
[RUNNING] target[..]release[..]foo-[..][EXE]", p.url()))
                       .with_stdout("
running 1 test
test external_bench ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured


running 1 test
test internal_bench ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

"))
}

#[test]
fn dont_run_examples() {
    if !is_nightly() { return }

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
}

#[test]
fn pass_through_command_line() {
    if !is_nightly() { return }

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
                .with_stderr(&format!("\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] release [optimized] target(s) in [..]
[RUNNING] target[..]release[..]foo-[..][EXE]", dir = p.url()))
                .with_stdout("
running 1 test
test bar ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

"));

    assert_that(p.cargo("bench").arg("foo"),
                execs().with_status(0)
                       .with_stderr("[FINISHED] release [optimized] target(s) in [..]
[RUNNING] target[..]release[..]foo-[..][EXE]")
                       .with_stdout("
running 1 test
test foo ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

"));
}

// Regression test for running cargo-bench twice with
// tests in an rlib
#[test]
fn cargo_bench_twice() {
    if !is_nightly() { return }

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
}

#[test]
fn lib_bin_same_name() {
    if !is_nightly() { return }

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
                execs().with_stderr(&format!("\
[COMPILING] foo v0.0.1 ({})
[FINISHED] release [optimized] target(s) in [..]
[RUNNING] target[..]release[..]foo-[..][EXE]
[RUNNING] target[..]release[..]foo-[..][EXE]", p.url()))
                       .with_stdout("
running 1 test
test [..] ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured


running 1 test
test [..] ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

"))
}

#[test]
fn lib_with_standard_name() {
    if !is_nightly() { return }

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
                       .with_stderr(&format!("\
[COMPILING] syntax v0.0.1 ({dir})
[FINISHED] release [optimized] target(s) in [..]
[RUNNING] target[..]release[..]bench-[..][EXE]
[RUNNING] target[..]release[..]syntax-[..][EXE]", dir = p.url()))
                       .with_stdout("
running 1 test
test bench ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured


running 1 test
test foo_bench ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

"));
}

#[test]
fn lib_with_standard_name2() {
    if !is_nightly() { return }

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
                       .with_stderr(&format!("\
[COMPILING] syntax v0.0.1 ({dir})
[FINISHED] release [optimized] target(s) in [..]
[RUNNING] target[..]release[..]syntax-[..][EXE]", dir = p.url()))
                       .with_stdout("
running 1 test
test bench ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

"));
}

#[test]
fn bench_dylib() {
    if !is_nightly() { return }

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
                       .with_stderr(&format!("\
[COMPILING] bar v0.0.1 ({dir}/bar)
[RUNNING] [..] -C opt-level=3 [..]
[COMPILING] foo v0.0.1 ({dir})
[RUNNING] [..] -C opt-level=3 [..]
[RUNNING] [..] -C opt-level=3 [..]
[RUNNING] [..] -C opt-level=3 [..]
[FINISHED] release [optimized] target(s) in [..]
[RUNNING] [..]target[..]release[..]bench-[..][EXE]
[RUNNING] [..]target[..]release[..]foo-[..][EXE]", dir = p.url()))
                       .with_stdout("
running 1 test
test foo ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured


running 1 test
test foo ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

"));
    p.root().move_into_the_past();
    assert_that(p.cargo("bench").arg("-v"),
                execs().with_status(0)
                       .with_stderr(&format!("\
[FRESH] bar v0.0.1 ({dir}/bar)
[FRESH] foo v0.0.1 ({dir})
[FINISHED] release [optimized] target(s) in [..]
[RUNNING] [..]target[..]release[..]bench-[..][EXE]
[RUNNING] [..]target[..]release[..]foo-[..][EXE]", dir = p.url()))
                       .with_stdout("
running 1 test
test foo ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured


running 1 test
test foo ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

"));
}

#[test]
fn bench_twice_with_build_cmd() {
    if !is_nightly() { return }

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
                       .with_stderr(&format!("\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] release [optimized] target(s) in [..]
[RUNNING] target[..]release[..]foo-[..][EXE]", dir = p.url()))
                       .with_stdout("
running 1 test
test foo ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

"));

    assert_that(p.cargo("bench"),
                execs().with_status(0)
                       .with_stderr("[FINISHED] release [optimized] target(s) in [..]
[RUNNING] target[..]release[..]foo-[..][EXE]")
                       .with_stdout("
running 1 test
test foo ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

"));
}

#[test]
fn bench_with_examples() {
    if !is_nightly() { return }

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
            #[cfg(test)]
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
                       .with_stderr(&format!("\
[COMPILING] testbench v6.6.6 ({url})
[RUNNING] `rustc [..]`
[RUNNING] `rustc [..]`
[RUNNING] `rustc [..]`
[FINISHED] release [optimized] target(s) in [..]
[RUNNING] `{dir}[..]target[..]release[..]testb1-[..][EXE] --bench`
[RUNNING] `{dir}[..]target[..]release[..]testbench-[..][EXE] --bench`",
                dir = p.root().display(), url = p.url()))
                       .with_stdout("
running 1 test
test bench_bench2 ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured


running 1 test
test bench_bench1 ... bench: [..] 0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured

"));
}

#[test]
fn test_a_bench() {
    if !is_nightly() { return }

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
                       .with_stderr("\
[COMPILING] foo v0.1.0 ([..])
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[..]debug[..]b-[..][EXE]")
                       .with_stdout("
running 1 test
test foo ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured

"));
}

#[test]
fn test_bench_no_run() {
    if !is_nightly() { return }

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
                       .with_stderr("\
[COMPILING] foo v0.1.0 ([..])
[FINISHED] release [optimized] target(s) in [..]
"));
}

#[test]
fn test_bench_multiple_packages() {
    if !is_nightly() { return }

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
                       .with_stderr_contains("\
[RUNNING] target[..]release[..]bbaz-[..][EXE]")
                       .with_stdout_contains("
running 1 test
test bench_baz ... bench:           0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured
")
                       .with_stderr_contains("\
[RUNNING] target[..]release[..]bbar-[..][EXE]")
                       .with_stdout_contains("
running 1 test
test bench_bar ... bench:           0 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured
"));
}
