//! Tests for the `cargo bench` command.

use crate::prelude::*;
use cargo_test_support::{basic_bin_manifest, basic_lib_manifest, basic_manifest, project, str};

#[cargo_test(nightly, reason = "bench")]
fn cargo_bench_simple() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file(
            "src/main.rs",
            r#"
            #![feature(test)]
            #[cfg(test)]
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
            }
            "#,
        )
        .build();

    p.cargo("build").run();
    assert!(p.bin("foo").is_file());

    p.process(&p.bin("foo"))
        .with_stdout_data(str![[r#"
hello

"#]])
        .run();

    p.cargo("bench")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `bench` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] unittests src/main.rs (target/release/deps/foo-[HASH][EXE])

"#]])
        .with_stdout_data(str![[r#"

running 1 test
test bench_hello ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .run();
}

#[cargo_test(nightly, reason = "bench")]
fn bench_bench_implicit() {
    let p = project()
        .file(
            "src/main.rs",
            r#"
            #![feature(test)]
            #[cfg(test)]
            extern crate test;
            #[bench] fn run1(_ben: &mut test::Bencher) { }
            fn main() { println!("Hello main!"); }
            "#,
        )
        .file(
            "tests/other.rs",
            r#"
            #![feature(test)]
            extern crate test;
            #[bench] fn run3(_ben: &mut test::Bencher) { }
            "#,
        )
        .file(
            "benches/mybench.rs",
            r#"
            #![feature(test)]
            extern crate test;
            #[bench] fn run2(_ben: &mut test::Bencher) { }
            "#,
        )
        .build();

    p.cargo("bench --benches")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `bench` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] [..] (target/release/deps/foo-[HASH][EXE])
[RUNNING] [..] (target/release/deps/mybench-[HASH][EXE])

"#]])
        .with_stdout_data(str![[r#"

running 1 test
test run1 ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 0 filtered out; finished in [ELAPSED]s


running 1 test
test run2 ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .run();
}

#[cargo_test(nightly, reason = "bench")]
fn bench_bin_implicit() {
    let p = project()
        .file(
            "src/main.rs",
            r#"
            #![feature(test)]
            #[cfg(test)]
            extern crate test;
            #[bench] fn run1(_ben: &mut test::Bencher) { }
            fn main() { println!("Hello main!"); }
            "#,
        )
        .file(
            "tests/other.rs",
            r#"
            #![feature(test)]
            extern crate test;
            #[bench] fn run3(_ben: &mut test::Bencher) { }
            "#,
        )
        .file(
            "benches/mybench.rs",
            r#"
            #![feature(test)]
            extern crate test;
            #[bench] fn run2(_ben: &mut test::Bencher) { }
            "#,
        )
        .build();

    p.cargo("bench --bins")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `bench` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] [..] (target/release/deps/foo-[HASH][EXE])

"#]])
        .with_stdout_data(str![[r#"

running 1 test
test run1 ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .run();
}

#[cargo_test(nightly, reason = "bench")]
fn bench_tarname() {
    let p = project()
        .file(
            "benches/bin1.rs",
            r#"
            #![feature(test)]
            extern crate test;
            #[bench] fn run1(_ben: &mut test::Bencher) { }
            "#,
        )
        .file(
            "benches/bin2.rs",
            r#"
            #![feature(test)]
            extern crate test;
            #[bench] fn run2(_ben: &mut test::Bencher) { }
            "#,
        )
        .build();

    p.cargo("bench --bench bin2")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `bench` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] [..] (target/release/deps/bin2-[HASH][EXE])

"#]])
        .with_stdout_data(str![[r#"

running 1 test
test run2 ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .run();
}

#[cargo_test(nightly, reason = "bench")]
fn bench_multiple_targets() {
    let p = project()
        .file(
            "benches/bin1.rs",
            r#"
            #![feature(test)]
            extern crate test;
            #[bench] fn run1(_ben: &mut test::Bencher) { }
            "#,
        )
        .file(
            "benches/bin2.rs",
            r#"
            #![feature(test)]
            extern crate test;
            #[bench] fn run2(_ben: &mut test::Bencher) { }
            "#,
        )
        .file(
            "benches/bin3.rs",
            r#"
            #![feature(test)]
            extern crate test;
            #[bench] fn run3(_ben: &mut test::Bencher) { }
            "#,
        )
        .build();

    // This should not have anything about `run3` in it.
    p.cargo("bench --bench bin1 --bench bin2")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `bench` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] benches/bin1.rs (target/release/deps/bin1-[HASH][EXE])
[RUNNING] benches/bin2.rs (target/release/deps/bin2-[HASH][EXE])

"#]])
        .run();
}

#[cargo_test(nightly, reason = "bench")]
fn cargo_bench_verbose() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file(
            "src/main.rs",
            r#"
                #![feature(test)]
                #[cfg(test)]
                extern crate test;
                fn main() {}
                #[bench] fn bench_hello(_b: &mut test::Bencher) {}
            "#,
        )
        .build();

    p.cargo("bench -v hello")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[RUNNING] `rustc [..] src/main.rs [..]`
[FINISHED] `bench` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] `[..]target/release/deps/foo-[HASH][EXE] hello --bench`

"#]])
        .with_stdout_data(str![[r#"

running 1 test
test bench_hello ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .run();
}

#[cargo_test(nightly, reason = "bench")]
fn many_similar_names() {
    let p = project()
        .file(
            "src/lib.rs",
            "
            #![feature(test)]
            #[cfg(test)]
            extern crate test;
            pub fn foo() {}
            #[bench] fn lib_bench(_b: &mut test::Bencher) {}
        ",
        )
        .file(
            "src/main.rs",
            "
            #![feature(test)]
            #[cfg(test)]
            extern crate foo;
            #[cfg(test)]
            extern crate test;
            fn main() {}
            #[bench] fn bin_bench(_b: &mut test::Bencher) { foo::foo() }
        ",
        )
        .file(
            "benches/foo.rs",
            r#"
                #![feature(test)]
                extern crate foo;
                extern crate test;
                #[bench] fn bench_bench(_b: &mut test::Bencher) { foo::foo() }
            "#,
        )
        .build();

    p.cargo("bench")
        .with_stdout_data(str![[r#"

running 1 test
test lib_bench ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 0 filtered out; finished in [ELAPSED]s


running 1 test
test bin_bench ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 0 filtered out; finished in [ELAPSED]s


running 1 test
test bench_bench ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .run();
}

#[cargo_test(nightly, reason = "bench")]
fn cargo_bench_failing_test() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file(
            "src/main.rs",
            r#"
            #![feature(test)]
            #[cfg(test)]
            extern crate test;
            fn hello() -> &'static str {
                "hello"
            }

            pub fn main() {
                println!("{}", hello())
            }

            #[bench]
            fn bench_hello(_b: &mut test::Bencher) {
                assert_eq!(hello(), "nope", "NOPE!")
            }
            "#,
        )
        .build();

    p.cargo("build").run();
    assert!(p.bin("foo").is_file());

    p.process(&p.bin("foo"))
        .with_stdout_data(str![[r#"
hello

"#]])
        .run();

    // Force libtest into serial execution so that the test header will be printed.
    p.cargo("bench -- --test-threads=1")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `bench` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] [..] (target/release/deps/foo-[HASH][EXE])
[ERROR] bench failed, to rerun pass `--bin foo`

"#]])
        .with_stdout_data("...\n[..]NOPE![..]\n...")
        .with_status(101)
        .run();
}

#[cargo_test(nightly, reason = "bench")]
fn bench_with_lib_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [[bin]]
                name = "baz"
                path = "src/main.rs"
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                #![feature(test)]
                #[cfg(test)]
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
            "#,
        )
        .file(
            "src/main.rs",
            "
            #![feature(test)]
            #[allow(unused_extern_crates)]
            extern crate foo;
            #[cfg(test)]
            extern crate test;

            fn main() {}

            #[bench]
            fn bin_bench(_b: &mut test::Bencher) {}
        ",
        )
        .build();

    p.cargo("bench")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `bench` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] [..] (target/release/deps/foo-[HASH][EXE])
[RUNNING] [..] (target/release/deps/baz-[HASH][EXE])

"#]])
        .with_stdout_data(str![[r#"

running 1 test
test lib_bench ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 0 filtered out; finished in [ELAPSED]s


running 1 test
test bin_bench ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .run();
}

#[cargo_test(nightly, reason = "bench")]
fn bench_with_deep_lib_dep() {
    let p = project()
        .at("bar")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies.foo]
                path = "../foo"
            "#,
        )
        .file(
            "src/lib.rs",
            "
            #![feature(test)]
            #[cfg(test)]
            extern crate foo;
            #[cfg(test)]
            extern crate test;
            #[bench]
            fn bar_bench(_b: &mut test::Bencher) {
                foo::foo();
            }
        ",
        )
        .build();
    let _p2 = project()
        .file(
            "src/lib.rs",
            "
            #![feature(test)]
            #[cfg(test)]
            extern crate test;

            pub fn foo() {}

            #[bench]
            fn foo_bench(_b: &mut test::Bencher) {}
        ",
        )
        .build();

    p.cargo("bench")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] bar v0.0.1 ([ROOT]/bar)
[FINISHED] `bench` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] [..] (target/release/deps/bar-[HASH][EXE])

"#]])
        .with_stdout_data(str![[r#"

running 1 test
test bar_bench ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .run();
}

#[cargo_test(nightly, reason = "bench")]
fn external_bench_explicit() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [[bench]]
                name = "bench"
                path = "src/bench.rs"
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                #![feature(test)]
                #[cfg(test)]
                extern crate test;
                pub fn get_hello() -> &'static str { "Hello" }

                #[bench]
                fn internal_bench(_b: &mut test::Bencher) {}
            "#,
        )
        .file(
            "src/bench.rs",
            r#"
                #![feature(test)]
                #[allow(unused_extern_crates)]
                extern crate foo;
                extern crate test;

                #[bench]
                fn external_bench(_b: &mut test::Bencher) {}
            "#,
        )
        .build();

    p.cargo("bench")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `bench` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] [..] (target/release/deps/foo-[HASH][EXE])
[RUNNING] [..] (target/release/deps/bench-[HASH][EXE])

"#]])
        .with_stdout_data(str![[r#"

running 1 test
test internal_bench ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 0 filtered out; finished in [ELAPSED]s


running 1 test
test external_bench ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .run();
}

#[cargo_test(nightly, reason = "bench")]
fn external_bench_implicit() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
                #![feature(test)]
                #[cfg(test)]
                extern crate test;

                pub fn get_hello() -> &'static str { "Hello" }

                #[bench]
                fn internal_bench(_b: &mut test::Bencher) {}
            "#,
        )
        .file(
            "benches/external.rs",
            r#"
                #![feature(test)]
                #[allow(unused_extern_crates)]
                extern crate foo;
                extern crate test;

                #[bench]
                fn external_bench(_b: &mut test::Bencher) {}
            "#,
        )
        .build();

    p.cargo("bench")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `bench` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] [..] (target/release/deps/foo-[HASH][EXE])
[RUNNING] [..] (target/release/deps/external-[HASH][EXE])

"#]])
        .with_stdout_data(str![[r#"

running 1 test
test internal_bench ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 0 filtered out; finished in [ELAPSED]s


running 1 test
test external_bench ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .run();
}

#[cargo_test(nightly, reason = "bench")]
fn bench_autodiscover_2015() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                edition = "2015"

                [features]
                magic = []

                [[bench]]
                name = "bench_magic"
                required-features = ["magic"]
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "benches/bench_basic.rs",
            r#"
                #![feature(test)]
                #[allow(unused_extern_crates)]
                extern crate foo;
                extern crate test;

                #[bench]
                fn bench_basic(_b: &mut test::Bencher) {}
            "#,
        )
        .file(
            "benches/bench_magic.rs",
            r#"
                #![feature(test)]
                #[allow(unused_extern_crates)]
                extern crate foo;
                extern crate test;

                #[bench]
                fn bench_magic(_b: &mut test::Bencher) {}
            "#,
        )
        .build();

    p.cargo("bench bench_basic")
        .with_stderr_data(str![[r#"
[WARNING] An explicit [[bench]] section is specified in Cargo.toml which currently
disables Cargo from automatically inferring other benchmark targets.
This inference behavior will change in the Rust 2018 edition and the following
files will be included as a benchmark target:

* [..]bench_basic.rs

This is likely to break cargo build or cargo test as these files may not be
ready to be compiled as a benchmark target today. You can future-proof yourself
and disable this warning by adding `autobenches = false` to your [package]
section. You may also move the files to a location where Cargo would not
automatically infer them to be a target, such as in subfolders.

For more information on this warning you can consult
https://github.com/rust-lang/cargo/issues/5330
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `bench` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] [..] (target/release/deps/foo-[HASH][EXE])

"#]])
        .run();
}

#[cargo_test(nightly, reason = "bench")]
fn dont_run_examples() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            "examples/dont-run-me-i-will-fail.rs",
            r#"fn main() { panic!("Examples should not be run by 'cargo test'"); }"#,
        )
        .build();
    p.cargo("bench").run();
}

#[cargo_test(nightly, reason = "bench")]
fn pass_through_command_line() {
    let p = project()
        .file(
            "src/lib.rs",
            "
            #![feature(test)]
            #[cfg(test)]
            extern crate test;

            #[bench] fn foo(_b: &mut test::Bencher) {}
            #[bench] fn bar(_b: &mut test::Bencher) {}
        ",
        )
        .build();

    p.cargo("bench bar")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `bench` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] [..] (target/release/deps/foo-[HASH][EXE])

"#]])
        .with_stdout_data(str![[r#"

running 1 test
test bar ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 1 filtered out; finished in [ELAPSED]s


"#]])
        .run();

    p.cargo("bench foo")
        .with_stderr_data(str![[r#"
[FINISHED] `bench` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] [..] (target/release/deps/foo-[HASH][EXE])

"#]])
        .with_stdout_data(str![[r#"

running 1 test
test foo ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 1 filtered out; finished in [ELAPSED]s


"#]])
        .run();
}

// Regression test for running cargo-bench twice with
// tests in an rlib
#[cargo_test(nightly, reason = "bench")]
fn cargo_bench_twice() {
    let p = project()
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file(
            "src/foo.rs",
            r#"
            #![crate_type = "rlib"]
            #![feature(test)]
            #[cfg(test)]
            extern crate test;

            #[bench]
            fn dummy_bench(b: &mut test::Bencher) { }
            "#,
        )
        .build();

    for _ in 0..2 {
        p.cargo("bench").run();
    }
}

#[cargo_test(nightly, reason = "bench")]
fn lib_bin_same_name() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [lib]
                name = "foo"
                [[bin]]
                name = "foo"
            "#,
        )
        .file(
            "src/lib.rs",
            "
            #![feature(test)]
            #[cfg(test)]
            extern crate test;
            #[bench] fn lib_bench(_b: &mut test::Bencher) {}
        ",
        )
        .file(
            "src/main.rs",
            "
            #![feature(test)]
            #[allow(unused_extern_crates)]
            extern crate foo;
            #[cfg(test)]
            extern crate test;

            #[bench]
            fn bin_bench(_b: &mut test::Bencher) {}
        ",
        )
        .build();

    p.cargo("bench")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `bench` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] [..] (target/release/deps/foo-[HASH][EXE])
[RUNNING] [..] (target/release/deps/foo-[HASH][EXE])

"#]])
        .with_stdout_data(str![[r#"

running 1 test
test lib_bench ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 0 filtered out; finished in [ELAPSED]s


running 1 test
test bin_bench ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .run();
}

#[cargo_test(nightly, reason = "bench")]
fn lib_with_standard_name() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("syntax", "0.0.1"))
        .file(
            "src/lib.rs",
            "
            #![feature(test)]
            #[cfg(test)]
            extern crate test;

            /// ```
            /// syntax::foo();
            /// ```
            pub fn foo() {}

            #[bench]
            fn foo_bench(_b: &mut test::Bencher) {}
        ",
        )
        .file(
            "benches/bench.rs",
            "
            #![feature(test)]
            extern crate syntax;
            extern crate test;

            #[bench]
            fn bench(_b: &mut test::Bencher) { syntax::foo() }
        ",
        )
        .build();

    p.cargo("bench")
        .with_stderr_data(str![[r#"
[COMPILING] syntax v0.0.1 ([ROOT]/foo)
[FINISHED] `bench` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] [..] (target/release/deps/syntax-[HASH][EXE])
[RUNNING] [..] (target/release/deps/bench-[HASH][EXE])

"#]])
        .with_stdout_data(str![[r#"

running 1 test
test foo_bench ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 0 filtered out; finished in [ELAPSED]s


running 1 test
test bench ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .run();
}

#[cargo_test(nightly, reason = "bench")]
fn lib_with_standard_name2() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "syntax"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [lib]
                name = "syntax"
                bench = false
                doctest = false
            "#,
        )
        .file("src/lib.rs", "pub fn foo() {}")
        .file(
            "src/main.rs",
            "
            #![feature(test)]
            #[cfg(test)]
            extern crate syntax;
            #[cfg(test)]
            extern crate test;

            fn main() {}

            #[bench]
            fn bench(_b: &mut test::Bencher) { syntax::foo() }
        ",
        )
        .build();

    p.cargo("bench")
        .with_stderr_data(str![[r#"
[COMPILING] syntax v0.0.1 ([ROOT]/foo)
[FINISHED] `bench` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] [..] (target/release/deps/syntax-[HASH][EXE])

"#]])
        .with_stdout_data(str![[r#"

running 1 test
test bench ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .run();
}

#[cargo_test(nightly, reason = "bench")]
fn bench_dylib() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [lib]
                name = "foo"
                crate-type = ["dylib"]

                [dependencies.bar]
                path = "bar"
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                #![feature(test)]
                extern crate bar as the_bar;
                #[cfg(test)]
                extern crate test;

                pub fn bar() { the_bar::baz(); }

                #[bench]
                fn foo(_b: &mut test::Bencher) {}
            "#,
        )
        .file(
            "benches/bench.rs",
            r#"
                #![feature(test)]
                extern crate foo as the_foo;
                extern crate test;

                #[bench]
                fn foo(_b: &mut test::Bencher) { the_foo::bar(); }
            "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [lib]
                name = "bar"
                crate-type = ["dylib"]
            "#,
        )
        .file("bar/src/lib.rs", "pub fn baz() {}")
        .build();

    p.cargo("bench -v")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] bar v0.0.1 ([ROOT]/foo/bar)
[RUNNING] [..] -C opt-level=3 [..]
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] [..] -C opt-level=3 [..]
[RUNNING] [..] -C opt-level=3 [..]
[RUNNING] [..] -C opt-level=3 [..]
[FINISHED] `bench` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] `[..]target/release/deps/foo-[HASH][EXE] --bench`
[RUNNING] `[..]target/release/deps/bench-[HASH][EXE] --bench`

"#]])
        .with_stdout_data(str![[r#"

running 1 test
test foo ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 0 filtered out; finished in [ELAPSED]s


running 1 test
test foo ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .run();

    p.root().move_into_the_past();
    p.cargo("bench -v")
        .with_stderr_data(str![[r#"
[FRESH] bar v0.0.1 ([ROOT]/foo/bar)
[FRESH] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `bench` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] `[..]target/release/deps/foo-[HASH][EXE] --bench`
[RUNNING] `[..]target/release/deps/bench-[HASH][EXE] --bench`

"#]])
        .with_stdout_data(str![[r#"

running 1 test
test foo ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 0 filtered out; finished in [ELAPSED]s


running 1 test
test foo ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .run();
}

#[cargo_test(nightly, reason = "bench")]
fn bench_twice_with_build_cmd() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []
                build = "build.rs"
            "#,
        )
        .file("build.rs", "fn main() {}")
        .file(
            "src/lib.rs",
            "
            #![feature(test)]
            #[cfg(test)]
            extern crate test;
            #[bench]
            fn foo(_b: &mut test::Bencher) {}
        ",
        )
        .build();

    p.cargo("bench")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `bench` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] [..] (target/release/deps/foo-[HASH][EXE])

"#]])
        .with_stdout_data(str![[r#"

running 1 test
test foo ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .run();

    p.cargo("bench")
        .with_stderr_data(str![[r#"
[FINISHED] `bench` profile [optimized] target(s) in [..]
[RUNNING] [..] (target/release/deps/foo-[HASH][EXE])

"#]])
        .with_stdout_data(str![[r#"

running 1 test
test foo ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .run();
}

#[cargo_test(nightly, reason = "bench")]
fn bench_with_examples() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "6.6.6"
                edition = "2015"
                authors = []

                [[example]]
                name = "teste1"

                [[bench]]
                name = "testb1"
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                #![feature(test)]
                #[cfg(test)]
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
            "#,
        )
        .file(
            "benches/testb1.rs",
            "
            #![feature(test)]
            extern crate foo;
            extern crate test;

            use test::Bencher;

            #[bench]
            fn bench_bench2(_b: &mut Bencher) {
                foo::f2();
            }
        ",
        )
        .file(
            "examples/teste1.rs",
            r#"
                extern crate foo;

                fn main() {
                    println!("example1");
                    foo::f1();
                }
            "#,
        )
        .build();

    p.cargo("bench -v")
        .with_stderr_data(str![[r#"
[COMPILING] foo v6.6.6 ([ROOT]/foo)
[RUNNING] `rustc [..]`
[RUNNING] `rustc [..]`
[RUNNING] `rustc [..]`
[FINISHED] `bench` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/foo/target/release/deps/foo-[HASH][EXE] --bench`
[RUNNING] `[ROOT]/foo/target/release/deps/testb1-[HASH][EXE] --bench`

"#]])
        .with_stdout_data(str![[r#"

running 1 test
test bench_bench1 ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 0 filtered out; finished in [ELAPSED]s


running 1 test
test bench_bench2 ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .run();
}

#[cargo_test(nightly, reason = "bench")]
fn test_a_bench() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                authors = []
                version = "0.1.0"
                edition = "2015"

                [lib]
                name = "foo"
                test = false
                doctest = false

                [[bench]]
                name = "b"
                test = true
            "#,
        )
        .file("src/lib.rs", "")
        .file("benches/b.rs", "#[test] fn foo() {}")
        .build();

    p.cargo("test")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] [..] (target/debug/deps/b-[HASH][EXE])

"#]])
        .with_stdout_data(str![[r#"

running 1 test
test foo ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .run();
}

#[cargo_test(nightly, reason = "bench")]
fn test_bench_no_run() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            "benches/bbaz.rs",
            r#"
                #![feature(test)]

                extern crate test;

                use test::Bencher;

                #[bench]
                fn bench_baz(_: &mut Bencher) {}
            "#,
        )
        .build();

    p.cargo("bench --no-run")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `bench` profile [optimized] target(s) in [ELAPSED]s
[EXECUTABLE] benches src/lib.rs (target/release/deps/foo-[HASH][EXE])
[EXECUTABLE] benches/bbaz.rs (target/release/deps/bbaz-[HASH][EXE])

"#]])
        .run();
}

#[cargo_test(nightly, reason = "bench")]
fn test_bench_no_run_emit_json() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            "benches/bbaz.rs",
            r#"
                #![feature(test)]

                extern crate test;

                use test::Bencher;

                #[bench]
                fn bench_baz(_: &mut Bencher) {}
            "#,
        )
        .build();

    p.cargo("bench --no-run --message-format json")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `bench` profile [optimized] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(nightly, reason = "bench")]
fn test_bench_no_fail_fast() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file(
            "src/main.rs",
            r#"
            #![feature(test)]
            #[cfg(test)]
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
            }

            #[bench]
            fn bench_nope(_b: &mut test::Bencher) {
                assert_eq!("nope", hello(), "NOPE!")
            }
            "#,
        )
        .file(
            "benches/b1.rs",
            r#"
                #![feature(test)]
                extern crate test;
                #[bench]
                fn b1_fail(_b: &mut test::Bencher) { assert_eq!(1, 2, "ONE=TWO"); }
            "#,
        )
        .build();

    p.cargo("bench --no-fail-fast -- --test-threads=1")
        .with_status(101)
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `bench` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] unittests src/main.rs (target/release/deps/foo-[HASH][EXE])
[ERROR] bench failed, to rerun pass `--bin foo`
[RUNNING] benches/b1.rs (target/release/deps/b1-[HASH][EXE])
[ERROR] bench failed, to rerun pass `--bench b1`
[ERROR] 2 targets failed:
    `--bin foo`
    `--bench b1`

"#]])
        .with_stdout_data(
            r#"
...
[..]NOPE![..]
...
[..]ONE=TWO[..]
...
"#,
        )
        .run();
}

#[cargo_test(nightly, reason = "bench")]
fn test_bench_multiple_packages() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                authors = []
                version = "0.1.0"
                edition = "2015"

                [dependencies.bar]
                path = "../bar"

                [dependencies.baz]
                path = "../baz"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    let _bar = project()
        .at("bar")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                authors = []
                version = "0.1.0"
                edition = "2015"

                [[bench]]
                name = "bbar"
                test = true
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "benches/bbar.rs",
            r#"
                #![feature(test)]
                extern crate test;

                use test::Bencher;

                #[bench]
                fn bench_bar(_b: &mut Bencher) {}
            "#,
        )
        .build();

    let _baz = project()
        .at("baz")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "baz"
                authors = []
                version = "0.1.0"
                edition = "2015"

                [[bench]]
                name = "bbaz"
                test = true
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "benches/bbaz.rs",
            r#"
                #![feature(test)]
                extern crate test;

                use test::Bencher;

                #[bench]
                fn bench_baz(_b: &mut Bencher) {}
            "#,
        )
        .build();

    p.cargo("bench -p bar -p baz")
        .with_stderr_data(str![[r#"
[RUNNING] [..] (target/release/deps/bbaz-[HASH][EXE])
[RUNNING] [..] (target/release/deps/bbar-[HASH][EXE])

"#]])
        .with_stderr_data(
            str![[r#"
[LOCKING] 2 packages to latest compatible versions
[COMPILING] bar v0.1.0 ([ROOT]/bar)
[COMPILING] baz v0.1.0 ([ROOT]/baz)
[FINISHED] `bench` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/release/deps/bar-[HASH][EXE])
[RUNNING] benches/bbar.rs (target/release/deps/bbar-[HASH][EXE])
[RUNNING] unittests src/lib.rs (target/release/deps/baz-[HASH][EXE])
[RUNNING] benches/bbaz.rs (target/release/deps/bbaz-[HASH][EXE])

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test(nightly, reason = "bench")]
fn bench_all_workspace() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                bar = { path = "bar" }

                [workspace]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "benches/foo.rs",
            r#"
                #![feature(test)]
                extern crate test;

                use test::Bencher;

                #[bench]
                fn bench_foo(_: &mut Bencher) -> () { () }
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .file(
            "bar/benches/bar.rs",
            r#"
                #![feature(test)]
                extern crate test;

                use test::Bencher;

                #[bench]
                fn bench_bar(_: &mut Bencher) -> () { () }
            "#,
        )
        .build();

    p.cargo("bench --workspace")
        .with_stderr_data(str![[r#"
[COMPILING] bar v0.1.0 ([ROOT]/foo/bar)
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `bench` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/release/deps/bar-[HASH][EXE])
[RUNNING] benches/bar.rs (target/release/deps/bar-[HASH][EXE])
[RUNNING] unittests src/main.rs (target/release/deps/foo-[HASH][EXE])
[RUNNING] benches/foo.rs (target/release/deps/foo-[HASH][EXE])

"#]])
        .with_stdout_data(str![[r#"

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s


running 1 test
test bench_bar ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 0 filtered out; finished in [ELAPSED]s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s


running 1 test
test bench_foo ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .run();
}

#[cargo_test(nightly, reason = "bench")]
fn bench_all_exclude() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [workspace]
                members = ["bar", "baz"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file(
            "bar/src/lib.rs",
            r#"
                #![feature(test)]
                #[cfg(test)]
                extern crate test;

                #[bench]
                pub fn bar(b: &mut test::Bencher) {
                    b.iter(|| {});
                }
            "#,
        )
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file(
            "baz/src/lib.rs",
            "#[test] pub fn baz() { break_the_build(); }",
        )
        .build();

    p.cargo("bench --workspace --exclude baz")
        .with_stdout_data(str![[r#"

running 1 test
test bar ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 0 filtered out; finished in [ELAPSED]s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .run();
}

#[cargo_test(nightly, reason = "bench")]
fn bench_all_exclude_glob() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [workspace]
                members = ["bar", "baz"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file(
            "bar/src/lib.rs",
            r#"
                #![feature(test)]
                #[cfg(test)]
                extern crate test;

                #[bench]
                pub fn bar(b: &mut test::Bencher) {
                    b.iter(|| {});
                }
            "#,
        )
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file(
            "baz/src/lib.rs",
            "#[test] pub fn baz() { break_the_build(); }",
        )
        .build();

    p.cargo("bench --workspace --exclude '*z'")
        .with_stdout_data(str![[r#"

running 1 test
test bar ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 0 filtered out; finished in [ELAPSED]s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .run();
}

#[cargo_test(nightly, reason = "bench")]
fn bench_all_virtual_manifest() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar", "baz"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .file(
            "bar/benches/bar.rs",
            r#"
                #![feature(test)]
                extern crate test;

                use test::Bencher;

                #[bench]
                fn bench_bar(_: &mut Bencher) -> () { () }
            "#,
        )
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "pub fn baz() {}")
        .file(
            "baz/benches/baz.rs",
            r#"
                #![feature(test)]
                extern crate test;

                use test::Bencher;

                #[bench]
                fn bench_baz(_: &mut Bencher) -> () { () }
            "#,
        )
        .build();

    // The order in which bar and baz are built is not guaranteed
    p.cargo("bench --workspace")
        .with_stderr_data(
            str![[r#"
[COMPILING] bar v0.1.0 ([ROOT]/foo/bar)
[COMPILING] baz v0.1.0 ([ROOT]/foo/baz)
[FINISHED] `bench` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/release/deps/bar-[HASH][EXE])
[RUNNING] benches/bar.rs (target/release/deps/bar-[HASH][EXE])
[RUNNING] unittests src/lib.rs (target/release/deps/baz-[HASH][EXE])
[RUNNING] benches/baz.rs (target/release/deps/baz-[HASH][EXE])

"#]]
            .unordered(),
        )
        .with_stdout_data(str![[r#"

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s


running 1 test
test bench_bar ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 0 filtered out; finished in [ELAPSED]s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s


running 1 test
test bench_baz ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .run();
}

#[cargo_test(nightly, reason = "bench")]
fn bench_virtual_manifest_glob() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar", "baz"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() { break_the_build(); }")
        .file(
            "bar/benches/bar.rs",
            r#"
                #![feature(test)]
                extern crate test;

                use test::Bencher;

                #[bench]
                fn bench_bar(_: &mut Bencher) -> () { break_the_build(); }
            "#,
        )
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "pub fn baz() {}")
        .file(
            "baz/benches/baz.rs",
            r#"
                #![feature(test)]
                extern crate test;

                use test::Bencher;

                #[bench]
                fn bench_baz(_: &mut Bencher) -> () { () }
            "#,
        )
        .build();

    // This should not have `bar` built or benched
    p.cargo("bench -p '*z'")
        .with_stderr_data(str![[r#"
[COMPILING] baz v0.1.0 ([ROOT]/foo/baz)
[FINISHED] `bench` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/release/deps/baz-[HASH][EXE])
[RUNNING] benches/baz.rs (target/release/deps/baz-[HASH][EXE])

"#]])
        .with_stdout_data(str![[r#"

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s


running 1 test
test bench_baz ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .run();
}

// https://github.com/rust-lang/cargo/issues/4287
#[cargo_test(nightly, reason = "bench")]
fn legacy_bench_name() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [[bench]]
                name = "bench"
            "#,
        )
        .file("src/lib.rs", "pub fn foo() {}")
        .file(
            "src/bench.rs",
            r#"
                #![feature(test)]
                extern crate test;

                use test::Bencher;

                #[bench]
                fn bench_foo(_: &mut Bencher) -> () { () }
            "#,
        )
        .build();

    p.cargo("bench")
        .with_stderr_data(str![[r#"
[WARNING] path `src/bench.rs` was erroneously implicitly accepted for benchmark `bench`,
please set bench.path in Cargo.toml
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `bench` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/release/deps/foo-[HASH][EXE])
[RUNNING] src/bench.rs (target/release/deps/bench-[HASH][EXE])

"#]])
        .run();
}

#[cargo_test(nightly, reason = "bench")]
fn bench_virtual_manifest_all_implied() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar", "baz"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn foo() {}")
        .file(
            "bar/benches/bar.rs",
            r#"
                #![feature(test)]
                extern crate test;
                use test::Bencher;
                #[bench]
                fn bench_bar(_: &mut Bencher) -> () { () }
            "#,
        )
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "pub fn baz() {}")
        .file(
            "baz/benches/baz.rs",
            r#"
                #![feature(test)]
                extern crate test;
                use test::Bencher;
                #[bench]
                fn bench_baz(_: &mut Bencher) -> () { () }
            "#,
        )
        .build();

    // The order in which bar and baz are built is not guaranteed

    p.cargo("bench")
        .with_stderr_data(
            str![[r#"
[COMPILING] bar v0.1.0 ([ROOT]/foo/bar)
[COMPILING] baz v0.1.0 ([ROOT]/foo/baz)
[FINISHED] `bench` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/release/deps/bar-[HASH][EXE])
[RUNNING] benches/bar.rs (target/release/deps/bar-[HASH][EXE])
[RUNNING] unittests src/lib.rs (target/release/deps/baz-[HASH][EXE])
[RUNNING] benches/baz.rs (target/release/deps/baz-[HASH][EXE])

"#]]
            .unordered(),
        )
        .with_stdout_data(str![[r#"

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s


running 1 test
test bench_bar ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 0 filtered out; finished in [ELAPSED]s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s


running 1 test
test bench_baz ... bench:           [AVG_ELAPSED] ns/iter (+/- [JITTER])

test result: ok. 0 passed; 0 failed; 0 ignored; 1 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .run();
}

#[cargo_test(nightly, reason = "bench")]
fn json_artifact_includes_executable_for_benchmark() {
    let p = project()
        .file(
            "benches/benchmark.rs",
            r#"
                #![feature(test)]
                extern crate test;

                use test::Bencher;

                #[bench]
                fn bench_foo(_: &mut Bencher) -> () { () }
            "#,
        )
        .build();

    p.cargo("bench --no-run --message-format=json")
        .with_stdout_data(
            str![[r#"
[
  {
    "executable": "[..]",
    "features": [],
    "filenames": "{...}",
    "fresh": false,
    "manifest_path": "[ROOT]/foo/Cargo.toml",
    "package_id": "path+[ROOTURL]/foo#0.0.1",
    "profile": "{...}",
    "reason": "compiler-artifact",
    "target": {
      "crate_types": [
        "bin"
      ],
      "doc": false,
      "doctest": false,
      "edition": "2015",
      "kind": [
        "bench"
      ],
      "name": "benchmark",
      "src_path": "[ROOT]/foo/benches/benchmark.rs",
      "test": false
    }
  },
  {
    "reason": "build-finished",
    "success": true
  }
]
"#]]
            .is_json()
            .against_jsonlines(),
        )
        .run();
}

#[cargo_test(nightly, reason = "bench")]
fn cargo_bench_print_env_verbose() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.1"))
        .file(
            "src/main.rs",
            r#"
            #![feature(test)]
            #[cfg(test)]
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
            }
            "#,
        )
        .build();
    p.cargo("bench -vv")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `[..]CARGO_MANIFEST_DIR=[ROOT]/foo[..] rustc[..]`
[FINISHED] `bench` profile [optimized] target(s) in [..]
[RUNNING] `[..]CARGO_MANIFEST_DIR=[ROOT]/foo[..] [ROOT]/foo/target/release/deps/foo-[HASH][EXE] --bench`

"#]])
        .run();
}
