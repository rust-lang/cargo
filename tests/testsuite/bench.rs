//! Tests for the `cargo bench` command.

use cargo_test_support::paths::CargoPathExt;
use cargo_test_support::{basic_bin_manifest, basic_lib_manifest, basic_manifest, project};

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

    p.process(&p.bin("foo")).with_stdout("hello\n").run();

    p.cargo("bench")
        .with_stderr(
            "\
[COMPILING] foo v0.5.0 ([CWD])
[FINISHED] bench [optimized] target(s) in [..]
[RUNNING] [..] (target/release/deps/foo-[..][EXE])",
        )
        .with_stdout_contains("test bench_hello ... bench: [..]")
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
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] bench [optimized] target(s) in [..]
[RUNNING] [..] (target/release/deps/foo-[..][EXE])
[RUNNING] [..] (target/release/deps/mybench-[..][EXE])
",
        )
        .with_stdout_contains("test run2 ... bench: [..]")
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
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] bench [optimized] target(s) in [..]
[RUNNING] [..] (target/release/deps/foo-[..][EXE])
",
        )
        .with_stdout_contains("test run1 ... bench: [..]")
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
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] bench [optimized] target(s) in [..]
[RUNNING] [..] (target/release/deps/bin2-[..][EXE])
",
        )
        .with_stdout_contains("test run2 ... bench: [..]")
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

    p.cargo("bench --bench bin1 --bench bin2")
        .with_stdout_contains("test run1 ... bench: [..]")
        .with_stdout_contains("test run2 ... bench: [..]")
        .with_stdout_does_not_contain("[..]run3[..]")
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
        .with_stderr(
            "\
[COMPILING] foo v0.5.0 ([CWD])
[RUNNING] `rustc [..] src/main.rs [..]`
[FINISHED] bench [optimized] target(s) in [..]
[RUNNING] `[..]target/release/deps/foo-[..][EXE] hello --bench`",
        )
        .with_stdout_contains("test bench_hello ... bench: [..]")
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
        .with_stdout_contains("test bin_bench ... bench:           0 ns/iter (+/- 0)")
        .with_stdout_contains("test lib_bench ... bench:           0 ns/iter (+/- 0)")
        .with_stdout_contains("test bench_bench ... bench:           0 ns/iter (+/- 0)")
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
                assert_eq!(hello(), "nope")
            }
            "#,
        )
        .build();

    p.cargo("build").run();
    assert!(p.bin("foo").is_file());

    p.process(&p.bin("foo")).with_stdout("hello\n").run();

    // Force libtest into serial execution so that the test header will be printed.
    p.cargo("bench -- --test-threads=1")
        .with_stdout_contains("test bench_hello ...[..]")
        .with_stderr_contains(
            "\
[COMPILING] foo v0.5.0 ([CWD])[..]
[FINISHED] bench [optimized] target(s) in [..]
[RUNNING] [..] (target/release/deps/foo-[..][EXE])",
        )
        .with_stdout_contains("[..]thread '[..]' panicked at[..]")
        .with_stdout_contains("[..]assertion [..]failed[..]")
        .with_stdout_contains("[..]left: [..]\"hello\"[..]")
        .with_stdout_contains("[..]right: [..]\"nope\"[..]")
        .with_stdout_contains("[..]src/main.rs:15[..]")
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
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] bench [optimized] target(s) in [..]
[RUNNING] [..] (target/release/deps/foo-[..][EXE])
[RUNNING] [..] (target/release/deps/baz-[..][EXE])",
        )
        .with_stdout_contains("test lib_bench ... bench: [..]")
        .with_stdout_contains("test bin_bench ... bench: [..]")
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
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[COMPILING] bar v0.0.1 ([CWD])
[FINISHED] bench [optimized] target(s) in [..]
[RUNNING] [..] (target/release/deps/bar-[..][EXE])",
        )
        .with_stdout_contains("test bar_bench ... bench: [..]")
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
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] bench [optimized] target(s) in [..]
[RUNNING] [..] (target/release/deps/foo-[..][EXE])
[RUNNING] [..] (target/release/deps/bench-[..][EXE])",
        )
        .with_stdout_contains("test internal_bench ... bench: [..]")
        .with_stdout_contains("test external_bench ... bench: [..]")
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
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] bench [optimized] target(s) in [..]
[RUNNING] [..] (target/release/deps/foo-[..][EXE])
[RUNNING] [..] (target/release/deps/external-[..][EXE])",
        )
        .with_stdout_contains("test internal_bench ... bench: [..]")
        .with_stdout_contains("test external_bench ... bench: [..]")
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
        .with_stderr(
            "warning: \
An explicit [[bench]] section is specified in Cargo.toml which currently
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
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] bench [optimized] target(s) in [..]
[RUNNING] [..] (target/release/deps/foo-[..][EXE])
",
        )
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
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] bench [optimized] target(s) in [..]
[RUNNING] [..] (target/release/deps/foo-[..][EXE])",
        )
        .with_stdout_contains("test bar ... bench: [..]")
        .run();

    p.cargo("bench foo")
        .with_stderr(
            "[FINISHED] bench [optimized] target(s) in [..]
[RUNNING] [..] (target/release/deps/foo-[..][EXE])",
        )
        .with_stdout_contains("test foo ... bench: [..]")
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
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] bench [optimized] target(s) in [..]
[RUNNING] [..] (target/release/deps/foo-[..][EXE])
[RUNNING] [..] (target/release/deps/foo-[..][EXE])",
        )
        .with_stdout_contains_n("test [..] ... bench: [..]", 2)
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
        .with_stderr(
            "\
[COMPILING] syntax v0.0.1 ([CWD])
[FINISHED] bench [optimized] target(s) in [..]
[RUNNING] [..] (target/release/deps/syntax-[..][EXE])
[RUNNING] [..] (target/release/deps/bench-[..][EXE])",
        )
        .with_stdout_contains("test foo_bench ... bench: [..]")
        .with_stdout_contains("test bench ... bench: [..]")
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
        .with_stderr(
            "\
[COMPILING] syntax v0.0.1 ([CWD])
[FINISHED] bench [optimized] target(s) in [..]
[RUNNING] [..] (target/release/deps/syntax-[..][EXE])",
        )
        .with_stdout_contains("test bench ... bench: [..]")
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
                authors = []

                [lib]
                name = "bar"
                crate_type = ["dylib"]
            "#,
        )
        .file("bar/src/lib.rs", "pub fn baz() {}")
        .build();

    p.cargo("bench -v")
        .with_stderr(
            "\
[COMPILING] bar v0.0.1 ([CWD]/bar)
[RUNNING] [..] -C opt-level=3 [..]
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] [..] -C opt-level=3 [..]
[RUNNING] [..] -C opt-level=3 [..]
[RUNNING] [..] -C opt-level=3 [..]
[FINISHED] bench [optimized] target(s) in [..]
[RUNNING] `[..]target/release/deps/foo-[..][EXE] --bench`
[RUNNING] `[..]target/release/deps/bench-[..][EXE] --bench`",
        )
        .with_stdout_contains_n("test foo ... bench: [..]", 2)
        .run();

    p.root().move_into_the_past();
    p.cargo("bench -v")
        .with_stderr(
            "\
[FRESH] bar v0.0.1 ([CWD]/bar)
[FRESH] foo v0.0.1 ([CWD])
[FINISHED] bench [optimized] target(s) in [..]
[RUNNING] `[..]target/release/deps/foo-[..][EXE] --bench`
[RUNNING] `[..]target/release/deps/bench-[..][EXE] --bench`",
        )
        .with_stdout_contains_n("test foo ... bench: [..]", 2)
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
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] bench [optimized] target(s) in [..]
[RUNNING] [..] (target/release/deps/foo-[..][EXE])",
        )
        .with_stdout_contains("test foo ... bench: [..]")
        .run();

    p.cargo("bench")
        .with_stderr(
            "[FINISHED] bench [optimized] target(s) in [..]
[RUNNING] [..] (target/release/deps/foo-[..][EXE])",
        )
        .with_stdout_contains("test foo ... bench: [..]")
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
        .with_stderr(
            "\
[COMPILING] foo v6.6.6 ([CWD])
[RUNNING] `rustc [..]`
[RUNNING] `rustc [..]`
[RUNNING] `rustc [..]`
[FINISHED] bench [optimized] target(s) in [..]
[RUNNING] `[CWD]/target/release/deps/foo-[..][EXE] --bench`
[RUNNING] `[CWD]/target/release/deps/testb1-[..][EXE] --bench`",
        )
        .with_stdout_contains("test bench_bench1 ... bench: [..]")
        .with_stdout_contains("test bench_bench2 ... bench: [..]")
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
        .with_stderr(
            "\
[COMPILING] foo v0.1.0 ([..])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/debug/deps/b-[..][EXE])",
        )
        .with_stdout_contains("test foo ... ok")
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
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] bench [optimized] target(s) in [..]
[EXECUTABLE] benches src/lib.rs (target/release/deps/foo-[..][EXE])
[EXECUTABLE] benches/bbaz.rs (target/release/deps/bbaz-[..][EXE])
",
        )
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
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] bench [optimized] target(s) in [..]
",
        )
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
                assert_eq!("nope", hello())
            }
            "#,
        )
        .file(
            "benches/b1.rs",
            r#"
                #![feature(test)]
                extern crate test;
                #[bench]
                fn b1_fail(_b: &mut test::Bencher) { assert_eq!(1, 2); }
            "#,
        )
        .build();

    p.cargo("bench --no-fail-fast -- --test-threads=1")
        .with_status(101)
        .with_stderr(
            "\
[COMPILING] foo v0.5.0 [..]
[FINISHED] bench [..]
[RUNNING] unittests src/main.rs (target/release/deps/foo[..])
[ERROR] bench failed, to rerun pass `--bin foo`
[RUNNING] benches/b1.rs (target/release/deps/b1[..])
[ERROR] bench failed, to rerun pass `--bench b1`
[ERROR] 2 targets failed:
    `--bin foo`
    `--bench b1`
",
        )
        .with_stdout_contains("running 2 tests")
        .with_stdout_contains("test bench_hello [..]")
        .with_stdout_contains("test bench_nope [..]")
        .with_stdout_contains("test b1_fail [..]")
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
        .with_stderr_contains("[RUNNING] [..] (target/release/deps/bbaz-[..][EXE])")
        .with_stdout_contains("test bench_baz ... bench: [..]")
        .with_stderr_contains("[RUNNING] [..] (target/release/deps/bbar-[..][EXE])")
        .with_stdout_contains("test bench_bar ... bench: [..]")
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
        .with_stderr_contains("[RUNNING] [..] (target/release/deps/bar-[..][EXE])")
        .with_stdout_contains("test bench_bar ... bench: [..]")
        .with_stderr_contains("[RUNNING] [..] (target/release/deps/foo-[..][EXE])")
        .with_stdout_contains("test bench_foo ... bench: [..]")
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
        .with_stdout_contains(
            "\
running 1 test
test bar ... bench:           [..] ns/iter (+/- [..])",
        )
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
        .with_stdout_contains(
            "\
running 1 test
test bar ... bench:           [..] ns/iter (+/- [..])",
        )
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
        .with_stderr_contains("[RUNNING] [..] (target/release/deps/baz-[..][EXE])")
        .with_stdout_contains("test bench_baz ... bench: [..]")
        .with_stderr_contains("[RUNNING] [..] (target/release/deps/bar-[..][EXE])")
        .with_stdout_contains("test bench_bar ... bench: [..]")
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

    // The order in which bar and baz are built is not guaranteed
    p.cargo("bench -p '*z'")
        .with_stderr_contains("[RUNNING] [..] (target/release/deps/baz-[..][EXE])")
        .with_stdout_contains("test bench_baz ... bench: [..]")
        .with_stderr_does_not_contain("[RUNNING] [..] (target/release/deps/bar-[..][EXE])")
        .with_stdout_does_not_contain("test bench_bar ... bench: [..]")
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
        .with_stderr_contains(
            "\
[WARNING] path `[..]src/bench.rs` was erroneously implicitly accepted for benchmark `bench`,
please set bench.path in Cargo.toml",
        )
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
        .with_stderr_contains("[RUNNING] [..] (target/release/deps/baz-[..][EXE])")
        .with_stdout_contains("test bench_baz ... bench: [..]")
        .with_stderr_contains("[RUNNING] [..] (target/release/deps/bar-[..][EXE])")
        .with_stdout_contains("test bench_bar ... bench: [..]")
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
        .with_json(
            r#"
                {
                    "executable": "[..]/foo/target/release/deps/benchmark-[..][EXE]",
                    "features": [],
                    "filenames": "{...}",
                    "fresh": false,
                    "package_id": "foo 0.0.1 ([..])",
                    "manifest_path": "[..]",
                    "profile": "{...}",
                    "reason": "compiler-artifact",
                    "target": {
                        "crate_types": [ "bin" ],
                        "kind": [ "bench" ],
                        "doc": false,
                        "doctest": false,
                        "edition": "2015",
                        "name": "benchmark",
                        "src_path": "[..]/foo/benches/benchmark.rs",
                        "test": false
                    }
                }

                {"reason": "build-finished", "success": true}
            "#,
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
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `[..]CARGO_MANIFEST_DIR=[CWD][..] rustc[..]`
[FINISHED] bench [optimized] target(s) in [..]
[RUNNING] `[..]CARGO_MANIFEST_DIR=[CWD][..] [CWD]/target/release/deps/foo-[..][EXE] --bench`",
        )
        .run();
}
