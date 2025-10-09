//! Tests for the `cargo test` command.

use std::fs;

use crate::prelude::*;
use crate::utils::cargo_exe;
use cargo_test_support::registry::Package;
use cargo_test_support::{basic_bin_manifest, basic_lib_manifest, basic_manifest, project, str};
use cargo_test_support::{cross_compile, paths};
use cargo_test_support::{rustc_host, rustc_host_env, sleep_ms};
use cargo_util::paths::dylib_path_envvar;

use crate::utils::cross_compile::can_run_on_host as cross_compile_can_run_on_host;

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

    p.process(&p.bin("foo"))
        .with_stdout_data(str![[r#"
hello

"#]])
        .run();

    p.cargo("test")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/main.rs (target/debug/deps/foo-[HASH][EXE])

"#]])
        .with_stdout_data(str![[r#"
...
test test_hello ... ok
...
"#]])
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
                edition = "2015"

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
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] bar v0.0.1 ([ROOT]/foo/bar)
[RUNNING] `rustc [..]-C opt-level=3 [..]`
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[RUNNING] `rustc [..]-C opt-level=3 [..]`
[RUNNING] `rustc [..]-C opt-level=3 [..]`
[RUNNING] `rustc [..]-C opt-level=3 [..]`
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/foo/target/release/deps/foo-[HASH][EXE]`
[RUNNING] `[ROOT]/foo/target/release/deps/test-[HASH][EXE]`
[DOCTEST] foo
[RUNNING] `rustdoc [..]--test src/lib.rs[..]`

"#]])
        .with_stdout_data(
            str![[r#"
test test ... ok
test test ... ok
running 0 tests
...
"#]]
            .unordered(),
        )
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
            edition = "2015"
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

    p.process(&p.release_bin("foo")).with_stdout_data("").run();
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
                edition = "2015"
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
        .with_stdout_data(str![[r#"

running 1 test
.
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .with_stderr_data("")
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
                edition = "2015"
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

    p.cargo("test -q")
        .with_stdout_data("")
        .with_stderr_data("")
        .run();
}

#[cargo_test]
fn cargo_doc_test_quiet() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
                authors = []
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                /// ```
                /// let result = foo::add(2, 3);
                /// assert_eq!(result, 5);
                /// ```
                pub fn add(a: i32, b: i32) -> i32 {
                    a + b
                }

                /// ```
                /// let result = foo::div(10, 2);
                /// assert_eq!(result, 5);
                /// ```
                ///
                /// # Panics
                ///
                /// The function panics if the second argument is zero.
                ///
                /// ```rust,should_panic
                /// // panics on division by zero
                /// foo::div(10, 0);
                /// ```
                pub fn div(a: i32, b: i32) -> i32 {
                    if b == 0 {
                        panic!("Divide-by-zero error");
                    }

                    a / b
                }

                #[test] fn test_hello() {}
            "#,
        )
        .build();

    p.cargo("test -q")
        .with_stdout_data(str![[r#"

running 1 test
.
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s


running 3 tests
...
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .with_stderr_data("")
        .run();
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
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[RUNNING] `rustc [..] src/main.rs [..]`
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/foo/target/debug/deps/foo-[HASH][EXE] hello`

"#]])
        .with_stdout_data(str![[r#"
...
test test_hello ... ok
...
"#]])
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
        .with_stdout_data(
            str![[r#"
test bin_test ... ok
test lib_test ... ok
test test_test ... ok
...
"#]]
            .unordered(),
        )
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

    p.cargo("test")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/main.rs (target/debug/deps/foo-[HASH][EXE])
[ERROR] test failed, to rerun pass `--bin foo`

"#]])
        .with_stdout_data("...\n[..]NOPE![..]\n...")
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
            r#"#[test] fn test_hello() { assert!(false, "FALSE!") }"#,
        )
        .build();

    p.cargo("build").run();
    assert!(p.bin("foo").is_file());

    p.process(&p.bin("foo"))
        .with_stdout_data(str![[r#"
hello

"#]])
        .run();

    p.cargo("test")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/main.rs (target/debug/deps/foo-[HASH][EXE])
[RUNNING] tests/footest.rs (target/debug/deps/footest-[HASH][EXE])
[ERROR] test failed, to rerun pass `--test footest`

"#]])
        .with_stdout_data(
            str![[r#"
...
running 0 tests
...
running 1 test
test test_hello ... FAILED
...
[..]FALSE![..]
...

"#]]
            .unordered(),
        )
        .with_status(101)
        .run();
}

#[cargo_test]
fn cargo_test_failing_test_in_lib() {
    let p = project()
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file(
            "src/lib.rs",
            r#"#[test] fn test_hello() { assert!(false, "FALSE!") }"#,
        )
        .build();

    p.cargo("test")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/deps/foo-[HASH][EXE])
[ERROR] test failed, to rerun pass `--lib`

"#]])
        .with_stdout_data(str![[r#"
...
test test_hello ... FAILED
...
[..]FALSE![..]
...
"#]])
        .with_status(101)
        .run();
}

#[cargo_test]
fn test_with_lib_dep() {
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
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/deps/foo-[HASH][EXE])
[RUNNING] unittests src/main.rs (target/debug/deps/baz-[HASH][EXE])
[DOCTEST] foo

"#]])
        .with_stdout_data(
            str![[r#"
test lib_test ... ok
test bin_test ... ok
test [..] ... ok
...
"#]]
            .unordered(),
        )
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
                edition = "2015"
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
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] bar v0.0.1 ([ROOT]/bar)
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/deps/foo-[HASH][EXE])
[DOCTEST] foo

"#]])
        .with_stdout_data(
            str![[r#"
test bar_test ... ok
test [..] ... ok
...
"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn external_test_explicit() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
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
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/deps/foo-[HASH][EXE])
[RUNNING] src/test.rs (target/debug/deps/test-[HASH][EXE])
[DOCTEST] foo

"#]])
        .with_stdout_data(
            str![[r#"
test internal_test ... ok
test external_test ... ok
running 0 tests
...
"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn external_test_named_test() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
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
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/deps/foo-[HASH][EXE])
[RUNNING] tests/external.rs (target/debug/deps/external-[HASH][EXE])
[DOCTEST] foo

"#]])
        .with_stdout_data(
            str![[r#"
test internal_test ... ok
test external_test ... ok
running 0 tests
...
"#]]
            .unordered(),
        )
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
fn pass_through_escaped() {
    let p = project()
        .file(
            "src/lib.rs",
            "
            /// ```rust
            /// assert!(foo::foo());
            /// ```
            pub fn foo() -> bool {
                true
            }

            /// ```rust
            /// assert!(!foo::bar());
            /// ```
            pub fn bar() -> bool {
                false
            }

            #[test] fn test_foo() {
                assert!(foo());
            }
            #[test] fn test_bar() {
                assert!(!bar());
            }
        ",
        )
        .build();

    p.cargo("test -- bar")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/deps/foo-[HASH][EXE])
[DOCTEST] foo

"#]])
        .with_stdout_data(str![[r#"
...
running 1 test
test test_bar ... ok
...
"#]])
        .run();

    p.cargo("test -- foo")
        .with_stderr_data(str![[r#"
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/deps/foo-[HASH][EXE])
[DOCTEST] foo

"#]])
        .with_stdout_data(str![[r#"
...
running 1 test
test test_foo ... ok
...
"#]])
        .run();

    p.cargo("test -- foo bar")
        .with_stderr_data(str![[r#"
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/deps/foo-[HASH][EXE])
[DOCTEST] foo

"#]])
        .with_stdout_data(
            str![[r#"
running 2 tests
test test_foo ... ok
test test_bar ... ok
...
"#]]
            .unordered(),
        )
        .run();
}

// Unlike `pass_through_escaped`, doctests won't run when using `testname` as an optimization
#[cargo_test]
fn pass_through_testname() {
    let p = project()
        .file(
            "src/lib.rs",
            "
            /// ```rust
            /// assert!(foo::foo());
            /// ```
            pub fn foo() -> bool {
                true
            }

            /// ```rust
            /// assert!(!foo::bar());
            /// ```
            pub fn bar() -> bool {
                false
            }

            #[test] fn test_foo() {
                assert!(foo());
            }
            #[test] fn test_bar() {
                assert!(!bar());
            }
        ",
        )
        .build();

    p.cargo("test bar")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/deps/foo-[HASH][EXE])

"#]])
        .with_stdout_data(str![[r#"
...
running 1 test
test test_bar ... ok
...
"#]])
        .run();

    p.cargo("test foo")
        .with_stderr_data(str![[r#"
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/deps/foo-[HASH][EXE])

"#]])
        .with_stdout_data(str![[r#"
...
running 1 test
test test_foo ... ok
...
"#]])
        .run();

    p.cargo("test foo -- bar")
        .with_stderr_data(str![[r#"
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/deps/foo-[HASH][EXE])

"#]])
        .with_stdout_data(
            str![[r#"
running 2 tests
test test_bar ... ok
test test_foo ... ok
...
"#]]
            .unordered(),
        )
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
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/deps/foo-[HASH][EXE])
[RUNNING] unittests src/main.rs (target/debug/deps/foo-[HASH][EXE])
[DOCTEST] foo

"#]])
        .with_stdout_data(
            str![[r#"
test lib_test ... ok
test bin_test ... ok
running 0 tests
...
"#]]
            .unordered(),
        )
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
        .with_stderr_data(str![[r#"
[COMPILING] syntax v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/deps/syntax-[HASH][EXE])
[RUNNING] tests/test.rs (target/debug/deps/test-[HASH][EXE])
[DOCTEST] syntax

"#]])
        .with_stdout_data(
            str![[r#"
test foo_test ... ok
test test ... ok
test [..] ... ok
...
"#]]
            .unordered(),
        )
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
                edition = "2015"
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
        .with_stderr_data(str![[r#"
[COMPILING] syntax v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/main.rs (target/debug/deps/syntax-[HASH][EXE])

"#]])
        .with_stdout_data(str![[r#"
...
test test ... ok
...
"#]])
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
                edition = "2015"
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
        .with_stderr_data(str![[r#"
[COMPILING] syntax v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/main.rs (target/debug/deps/syntax-[HASH][EXE])

"#]])
        .with_stdout_data(str![[r#"
...
test test ... ok
...
"#]])
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
                edition = "2015"
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
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  binary target bin.name is required

"#]])
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
                edition = "2015"
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
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  benchmark target bench.name is required

"#]])
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
                edition = "2015"
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
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  test target test.name is required

"#]])
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
                edition = "2015"
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
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  example target example.name is required

"#]])
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

    p.cargo("test -v")
        .with_stdout_data(
            str![[r#"
test main_test ... ok
test test_test ... ok
...
"#]]
            .unordered(),
        )
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
                edition = "2015"
                authors = []

                [lib]
                name = "bar"
                crate-type = ["dylib"]
            "#,
        )
        .file("bar/src/lib.rs", "pub fn baz() {}")
        .build();

    p.cargo("test")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] bar v0.0.1 ([ROOT]/foo/bar)
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/deps/foo-[HASH][EXE])
[RUNNING] tests/test.rs (target/debug/deps/test-[HASH][EXE])

"#]])
        .with_stdout_data(
            str![[r#"
test foo ... ok
test foo ... ok
...
"#]]
            .unordered(),
        )
        .run();

    p.root().move_into_the_past();
    p.cargo("test")
        .with_stderr_data(str![[r#"
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/deps/foo-[HASH][EXE])
[RUNNING] tests/test.rs (target/debug/deps/test-[HASH][EXE])

"#]])
        .with_stdout_data(
            str![[r#"
test foo ... ok
test foo ... ok
...
"#]]
            .unordered(),
        )
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
                edition = "2015"
                authors = []
                build = "build.rs"
            "#,
        )
        .file("build.rs", "fn main() {}")
        .file("src/lib.rs", "#[test] fn foo() {}")
        .build();

    p.cargo("test")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/deps/foo-[HASH][EXE])
[DOCTEST] foo

"#]])
        .with_stdout_data(
            str![[r#"
test foo ... ok
running 0 tests
...
"#]]
            .unordered(),
        )
        .run();

    p.cargo("test")
        .with_stderr_data(str![[r#"
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/deps/foo-[HASH][EXE])
[DOCTEST] foo

"#]])
        .with_stdout_data(
            str![[r#"
test foo ... ok
running 0 tests
...
"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn test_then_build() {
    let p = project().file("src/lib.rs", "#[test] fn foo() {}").build();

    p.cargo("test")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/deps/foo-[HASH][EXE])
[DOCTEST] foo

"#]])
        .with_stdout_data(
            str![[r#"
test foo ... ok
running 0 tests
...
"#]]
            .unordered(),
        )
        .run();

    p.cargo("build")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn test_no_run() {
    let p = project()
        .file("src/lib.rs", "#[test] fn foo() { panic!() }")
        .build();

    p.cargo("test --no-run")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[EXECUTABLE] unittests src/lib.rs (target/debug/deps/foo-[HASH][EXE])

"#]])
        .run();
}

#[cargo_test]
fn test_no_run_emit_json() {
    let p = project()
        .file("src/lib.rs", "#[test] fn foo() { panic!() }")
        .build();

    p.cargo("test --no-run --message-format json")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
                edition = "2015"
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
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/bin2.rs (target/debug/deps/bin2-[HASH][EXE])

"#]])
        .with_stdout_data(str![[r#"
...
test test2 ... ok
...
"#]])
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
                edition = "2015"
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
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/mybin.rs (target/debug/deps/mybin-[HASH][EXE])

"#]])
        .with_stdout_data(str![[r#"
...
test test_in_bin ... ok
...
"#]])
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
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] tests/b.rs (target/debug/deps/b-[HASH][EXE])

"#]])
        .with_stdout_data(str![[r#"
...
test test_b ... ok
...
"#]])
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
                edition = "2015"
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
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/mybin.rs (target/debug/deps/mybin-[HASH][EXE])
[RUNNING] tests/mytest.rs (target/debug/deps/mytest-[HASH][EXE])

"#]])
        .with_stdout_data(str![[r#"
...
test test_in_test ... ok
...
"#]])
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
                edition = "2015"
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
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/mybin.rs (target/debug/deps/mybin-[HASH][EXE])
[RUNNING] benches/mybench.rs (target/debug/deps/mybench-[HASH][EXE])

"#]])
        .with_stdout_data(str![[r#"
...
test test_in_bench ... ok
...
"#]])
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
                edition = "2015"
                authors = []

                [[bin]]
                name = "mybin"
                path = "src/mybin.rs"

                [[example]]
                name = "myexm1"

                [[example]]
                name = "myexm2"
                test = true

                [profile.test]
                panic = "abort" # this should be ignored by default Cargo targets set.
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

    // Compiles myexm1 as normal binary (without --test), but does not run it.
    prj.cargo("test -v")
        .with_stderr_contains("[RUNNING] `rustc [..]myexm1.rs [..]--crate-type bin[..]")
        .with_stderr_contains("[RUNNING] `rustc [..]myexm2.rs [..]--test[..]")
        .with_stderr_does_not_contain("[RUNNING] [..]myexm1-[..]")
        // profile.test panic settings shouldn't be applied even to myexm1
        .with_stderr_line_without(&["[RUNNING] `rustc --crate-name myexm1"], &["panic=abort"])
        .with_stderr_contains("[RUNNING] [..]target/debug/examples/myexm2-[..]")
        .run();

    // Only tests myexm2.
    prj.cargo("test --tests")
        .with_stderr_does_not_contain("[RUNNING] [..]myexm1-[..]")
        .with_stderr_contains("[RUNNING] [..]target/debug/examples/myexm2-[..]")
        .run();

    // Tests all examples.
    prj.cargo("test --examples")
        .with_stderr_data(str![[r#"
...
[RUNNING] unittests examples/myexm1.rs (target/debug/examples/myexm1-[HASH][EXE])
[RUNNING] unittests examples/myexm2.rs (target/debug/examples/myexm2-[HASH][EXE])
...
"#]])
        .run();

    // Test an example, even without `test` set.
    prj.cargo("test --example myexm1")
        .with_stderr_data(str![[r#"
...
[RUNNING] unittests examples/myexm1.rs (target/debug/examples/myexm1-[HASH][EXE])
...
"#]])
        .run();

    // Tests all examples.
    prj.cargo("test --all-targets")
        .with_stderr_data(str![[r#"
...
[RUNNING] unittests examples/myexm1.rs (target/debug/examples/myexm1-[HASH][EXE])
[RUNNING] unittests examples/myexm2.rs (target/debug/examples/myexm2-[HASH][EXE])
...
"#]])
        .run();
}

#[cargo_test]
fn test_filtered_excludes_compiling_examples() {
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
                name = "mybin"
                test = false
            "#,
        )
        .file(
            "src/lib.rs",
            "#[cfg(test)] mod tests { #[test] fn test_in_lib() { } }",
        )
        .file(
            "src/bin/mybin.rs",
            "#[test] fn test_in_bin() { }
               fn main() { panic!(\"Don't execute me!\"); }",
        )
        .file("tests/mytest.rs", "#[test] fn test_in_test() { }")
        .file(
            "benches/mybench.rs",
            "#[test] fn test_in_bench() { assert!(false) }",
        )
        .file(
            "examples/myexm1.rs",
            "#[test] fn test_in_exm() { assert!(false) }
               fn main() { panic!(\"Don't execute me!\"); }",
        )
        .build();

    p.cargo("test -v test_in_")
        .with_stdout_data(str![[r#"

running 1 test
test tests::test_in_lib ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s


running 1 test
test test_in_test ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .with_stderr_data(
            str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo --edition=2015 src/lib.rs [..] --crate-type lib [..]`
[RUNNING] `rustc --crate-name foo --edition=2015 src/lib.rs [..] --test [..]`
[RUNNING] `rustc --crate-name mybin --edition=2015 src/bin/mybin.rs [..] --crate-type bin [..]`
[RUNNING] `rustc --crate-name mytest --edition=2015 tests/mytest.rs [..] --test [..]`
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/foo/target/debug/deps/foo-[HASH][EXE] test_in_`
[RUNNING] `[ROOT]/foo/target/debug/deps/mytest-[HASH][EXE] test_in_`

"#]]
            .unordered(),
        )
        .with_stderr_does_not_contain("[RUNNING][..]rustc[..]myexm1[..]")
        .with_stderr_does_not_contain("[RUNNING][..]deps/mybin-[..] test_in_")
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
                edition = "2015"
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

    p.cargo("test -- --no-capture")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] foo.rs (target/debug/deps/bar-[HASH][EXE])

"#]])
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
                edition = "2015"
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
                edition = "2015"
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
                edition = "2015"
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
        .with_stderr_data(str![[r#"
[LOCKING] 2 packages to latest compatible versions
[COMPILING] d1 v0.0.1 ([ROOT]/foo/d1)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/deps/d1-[HASH][EXE])
[RUNNING] unittests src/main.rs (target/debug/deps/d1-[HASH][EXE])

"#]])
        .with_stdout_data(
            str![[r#"
running 0 tests
running 0 tests
...
"#]]
            .unordered(),
        )
        .run();

    println!("d2");
    p.cargo("test -p d2")
        .with_stderr_data(str![[r#"
[COMPILING] d2 v0.0.1 ([ROOT]/foo/d2)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/deps/d2-[HASH][EXE])
[RUNNING] unittests src/main.rs (target/debug/deps/d2-[HASH][EXE])

"#]])
        .with_stdout_data(
            str![[r#"
running 0 tests
running 0 tests
...
"#]]
            .unordered(),
        )
        .run();

    println!("whole");
    p.cargo("test")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/deps/foo-[HASH][EXE])

"#]])
        .with_stdout_data(str![[r#"
...
running 0 tests
...
"#]])
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
                edition = "2015"
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
                edition = "2015"
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
                edition = "2015"
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
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
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
                edition = "2015"
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
                edition = "2015"
                authors = []

                [lib]
                name = "d1"
                path = "d1.rs"
            "#,
        )
        .file("d1/d1.rs", "");
    let p = p.build();

    p.cargo("test -p d1")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] d1 v0.0.1 ([ROOT]/foo/d1)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests d1.rs (target/debug/deps/d1-[HASH][EXE])
[DOCTEST] d1

"#]])
        .with_stdout_data(
            str![[r#"
running 0 tests
running 0 tests
...
"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn example_bin_same_name() {
    let p = project()
        .file("src/bin/foo.rs", r#"fn main() { println!("bin"); }"#)
        .file("examples/foo.rs", r#"fn main() { println!("example"); }"#)
        .build();

    p.cargo("test --no-run -v")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..]`
[RUNNING] `rustc [..]`
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[EXECUTABLE] `[ROOT]/foo/target/debug/deps/foo-[HASH][EXE]`

"#]])
        .run();

    assert!(!p.bin("foo").is_file());
    assert!(p.bin("examples/foo").is_file());

    p.process(&p.bin("examples/foo"))
        .with_stdout_data(str![[r#"
example

"#]])
        .run();

    p.cargo("run")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .with_stdout_data(str![[r#"
bin

"#]])
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
                edition = "2015"
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
        .with_stderr_data(
            str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] a v0.0.1 ([ROOT]/foo/a)
[RUNNING] `rustc --crate-name foo [..]`
[RUNNING] `rustc --crate-name a [..]`
[RUNNING] `rustc --crate-name ex [..] --extern a=[..]`
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
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
        .with_stderr_data(str![[r#"
[ERROR] no example target named `foo` in default-run packages

"#]])
        .run();
    p.cargo("run --bin foo")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no bin target named `foo` in default-run packages

"#]])
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
                edition = "2015"
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
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/deps/foo-[HASH][EXE])
[DOCTEST] foo

"#]])
        .with_stdout_data(
            str![[r#"
running 0 tests
test [..] ... ok
...
"#]]
            .unordered(),
        )
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
                edition = "2015"
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
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] tests/foo.rs (target/debug/deps/foo-[HASH][EXE])

"#]])
        .with_stdout_data(str![[r#"
...
running 0 tests
...
"#]])
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
                edition = "2015"
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
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[DOCTEST] foo

"#]])
        .with_stdout_data(str![[r#"
...
test [..] ... ok
...
"#]])
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
                edition = "2015"
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

    p.cargo("test")
        .with_stderr_data(str![[r#"
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
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
                edition = "2015"
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
                edition = "2015"
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
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] bar v0.0.1 ([ROOT]/foo/bar)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/deps/foo-[HASH][EXE])
[DOCTEST] foo

"#]])
        .with_stdout_data(
            str![[r#"
running 0 tests
test [..] ... ok
...
"#]]
            .unordered(),
        )
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
                edition = "2015"
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
                edition = "2015"
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
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/deps/foo-[HASH][EXE])
[RUNNING] tests/test_add_one.rs (target/debug/deps/test_add_one-[HASH][EXE])
[ERROR] test failed, to rerun pass `--test test_add_one`
[RUNNING] tests/test_sub_one.rs (target/debug/deps/test_sub_one-[HASH][EXE])
[DOCTEST] foo
[ERROR] 1 target failed:
    `--test test_add_one`

"#]])
        .with_stdout_data(str![[r#"
running 0 tests
test add_one_test ... ok
test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s
test sub_one_test ... ok
test [..] ... ok
...
"#]].unordered())
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
                edition = "2015"
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
                edition = "2015"
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
                edition = "2015"
                authors = []

                [lib]
                    name = "d2"
                    doctest = false
            "#,
        )
        .file("d2/src/lib.rs", "");
    let p = p.build();

    p.cargo("test -p d1 -p d2")
        .with_stderr_data(str![[r#"
...
[RUNNING] unittests src/lib.rs (target/debug/deps/d1-[HASH][EXE])
[RUNNING] unittests src/lib.rs (target/debug/deps/d2-[HASH][EXE])
...
"#]])
        .with_stdout_data(
            str![[r#"
running 0 tests
running 0 tests
...
"#]]
            .unordered(),
        )
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
        .with_stderr_data(str![[r#"
[DIRTY] foo v0.0.1 ([ROOT]/foo): the file `src/main.rs` has changed ([..])
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..] src/main.rs [..]`
[RUNNING] `rustc [..] src/main.rs [..]`
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[EXECUTABLE] `[ROOT]/foo/target/debug/deps/foo-[HASH][EXE]`
[EXECUTABLE] `[ROOT]/foo/target/debug/deps/foo-[HASH][EXE]`
[EXECUTABLE] `[ROOT]/foo/target/debug/deps/foo-[HASH][EXE]`

"#]])
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
                edition = "2015"
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
                edition = "2015"
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
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] a v0.0.1 ([ROOT]/foo/a)
[RUNNING] `rustc [..] a/src/lib.rs [..]`
[RUNNING] `rustc [..] a/src/lib.rs [..]`
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[EXECUTABLE] `[ROOT]/foo/target/debug/deps/a-[HASH][EXE]`

"#]])
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
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[DOCTEST] foo

"#]])
        .with_stdout_data(str![[r#"
...
test [..] ... ok
...
"#]])
        .run();
}

#[cargo_test]
fn doctest_with_library_paths() {
    let p = project();
    // Only link search directories within the target output directory are
    // propagated through to dylib_path_envvar() (see #3366).
    let dir1 = p.target_debug_dir().join("foo\\backslash");
    let dir2 = p.target_debug_dir().join("dir=containing=equal=signs");

    let p = p
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file(
            "build.rs",
            &format!(
                r##"
                    fn main() {{
                        println!(r#"cargo::rustc-link-search=native={}"#);
                        println!(r#"cargo::rustc-link-search={}"#);
                    }}
                "##,
                dir1.display(),
                dir2.display()
            ),
        )
        .file(
            "src/lib.rs",
            &format!(
                r##"
                    /// ```
                    /// foo::assert_search_path();
                    /// ```
                    pub fn assert_search_path() {{
                        let search_path = std::env::var_os("{}").unwrap();
                        let paths = std::env::split_paths(&search_path).collect::<Vec<_>>();
                        assert!(paths.contains(&r#"{}"#.into()));
                        assert!(paths.contains(&r#"{}"#.into()));
                    }}
                "##,
                dylib_path_envvar(),
                dir1.display(),
                dir2.display()
            ),
        )
        .build();

    p.cargo("test --doc").run();
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
                edition = "2015"
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
                edition = "2015"
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
        .with_stdout_data(str![[r#"
hello!

"#]])
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..]`
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/foo/target/debug/deps/foo-[HASH][EXE]`

"#]])
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
                edition = "2015"
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
                edition = "2015"
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
                edition = "2015"
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
        .with_stderr_data(str![[r#"
...
[DOCTEST] feature_a
[RUNNING] `rustdoc [..]--test [..]mock_serde_codegen[..]`
...
"#]])
        .run();

    p.cargo("test --verbose")
        .with_stderr_data(str![[r#"
...
[DOCTEST] foo
[RUNNING] `rustdoc [..]--test [..]feature_a[..]`
...
"#]])
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
                edition = "2015"
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
                edition = "2015"
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
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

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
        .with_stdout_data(
            str![[r#"
test foo_test ... ok
test bar_test ... ok
...
"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn test_all_exclude() {
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
        .file("bar/src/lib.rs", "#[test] pub fn bar() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "#[test] pub fn baz() { assert!(false); }")
        .build();

    p.cargo("test --workspace --exclude baz")
        .with_stdout_data(str![[r#"
...
running 1 test
test bar ... ok
...
"#]])
        .run();
}

#[cargo_test]
fn test_all_exclude_not_found() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [workspace]
                members = ["bar"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "#[test] pub fn bar() {}")
        .build();

    p.cargo("test --workspace --exclude baz")
        .with_stderr_data(str![[r#"
...
[WARNING] excluded package(s) `baz` not found in workspace `[ROOT]/foo`
...
"#]])
        .with_stdout_data(str![[r#"
...
running 1 test
test bar ... ok
...
"#]])
        .run();
}

#[cargo_test]
fn test_all_exclude_glob() {
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
        .file("bar/src/lib.rs", "#[test] pub fn bar() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "#[test] pub fn baz() { assert!(false); }")
        .build();

    p.cargo("test --workspace --exclude '*z'")
        .with_stdout_data(str![[r#"
...
running 1 test
test bar ... ok
...
"#]])
        .run();
}

#[cargo_test]
fn test_all_exclude_glob_not_found() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [workspace]
                members = ["bar"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "#[test] pub fn bar() {}")
        .build();

    p.cargo("test --workspace --exclude '*z'")
        .with_stderr_data(str![[r#"
...
[WARNING] excluded package pattern(s) `*z` not found in workspace `[ROOT]/foo`
...
"#]])
        .with_stdout_data(str![[r#"
...
running 1 test
test bar ... ok
...
"#]])
        .run();
}

#[cargo_test]
fn test_all_exclude_broken_glob() {
    let p = project().file("src/main.rs", "fn main() {}").build();

    p.cargo("test --workspace --exclude '[*z'")
        .with_status(101)
        .with_stderr_data(str![[r#"
...
[ERROR] cannot build glob pattern from `[*z`
...
"#]])
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
        .with_stdout_data(
            str![[r#"
running 1 test
test a ... ok
running 1 test
test b ... ok
...
"#]]
            .unordered(),
        )
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
        .with_stdout_data(
            str![[r#"
running 1 test
test a ... ok
running 1 test
test b ... ok
...
"#]]
            .unordered(),
        )
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
        .with_stderr_data(str![[r#"
[ERROR] package pattern(s) `*z` not found in workspace `[ROOT]/foo`
...
"#]])
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
        .with_stderr_data(str![[r#"
[ERROR] cannot build glob pattern from `[*z`
...
"#]])
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
                [package]
                name = "a"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                a = "0.1.0"
            "#,
        )
        .file("a/src/lib.rs", "#[test] fn a() {}")
        .build();

    Package::new("a", "0.1.0").publish();

    p.cargo("test --workspace")
        .with_stdout_data(str![[r#"
...
test a ... ok
...
"#]])
        .run();
}

#[cargo_test]
fn doctest_only_with_dev_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.1.0"
                edition = "2015"

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
        .with_stdout_data(
            str![[r#"
test bin_a ... ok
test bin_b ... ok
test test_a ... ok
test test_b ... ok
...
"#]]
            .unordered(),
        )
        .with_stderr_data(
            str![[r#"
[RUNNING] `rustc --crate-name a --edition=2015 examples/a.rs [..]`
[RUNNING] `rustc --crate-name b --edition=2015 examples/b.rs [..]`
...
"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn doctest_and_registry() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.1.0"
                edition = "2015"

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
                [package]
                name = "c"
                version = "0.1.0"
                edition = "2015"

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
    let rustc_host = rustc_host();
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

    let cargo = format!(
        "{}[EXE]",
        cargo_exe()
            .with_extension("")
            .to_str()
            .unwrap()
            .replace(rustc_host, "[HOST_TARGET]")
    );
    p.cargo("test --lib -- --no-capture")
        .with_stderr_contains(cargo)
        .with_stdout_data(str![[r#"
...
test env_test ... ok
...
"#]])
        .run();

    // Check that `cargo test` propagates the environment's $CARGO
    let cargo_exe = cargo_exe();
    let other_cargo_path = p.root().join(cargo_exe.file_name().unwrap());
    std::fs::hard_link(&cargo_exe, &other_cargo_path).unwrap();
    let stderr_other_cargo = format!(
        "{}[EXE]",
        other_cargo_path
            .with_extension("")
            .to_str()
            .unwrap()
            .replace(p.root().parent().unwrap().to_str().unwrap(), "[ROOT]")
    );
    p.process(other_cargo_path)
        .args(&["test", "--lib", "--", "--no-capture"])
        .with_stderr_contains(stderr_other_cargo)
        .with_stdout_data(str![[r#"
...
test env_test ... ok
...
"#]])
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
        .with_stdout_data(str![[r#"

running 1 test
test test_lib ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s


running 1 test
test test_a ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s


running 1 test
test test_z ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s
...
"#]])
        .run();
}

#[cargo_test]
fn cyclic_dev() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

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
fn cyclical_dep_with_missing_feature() {
    // Checks for error handling when a cyclical dev-dependency specify a
    // feature that doesn't exist.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dev-dependencies]
                foo = { path = ".", features = ["missing"] }
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to select a version for `foo`.
    ... required by package `foo v0.1.0 ([ROOT]/foo)`
versions that meet the requirements `*` are: 0.1.0

package `foo` depends on `foo` with feature `missing` but `foo` does not have that feature.


failed to select a version for `foo` which could resolve this conflict

"#]])
        .run();
}

#[cargo_test]
fn publish_a_crate_without_tests() {
    Package::new("testless", "0.1.0")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "testless"
                version = "0.1.0"
                edition = "2015"
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
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

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
                [package]
                name = "root"
                version = "0.1.0"
                edition = "2015"
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
                [package]
                name = "proc_macro_dep"
                version = "0.1.0"
                edition = "2015"
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
        .with_stdout_data(
            str![[r#"
test this_fails ... FAILED
test [..]this_works (line [..]) ... ok
...
"#]]
            .unordered(),
        )
        .with_stderr_data(str![[r#"
...
[ERROR] test failed, to rerun pass `--test integ`
...
"#]])
        .run();
}

#[cargo_test]
fn test_hint_workspace_virtual() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["a", "b", "c"]
            "#,
        )
        .file("a/Cargo.toml", &basic_manifest("a", "0.1.0"))
        .file("a/src/lib.rs", "#[test] fn t1() {}")
        .file("b/Cargo.toml", &basic_manifest("b", "0.1.0"))
        .file("b/src/lib.rs", "#[test] fn t1() {assert!(false)}")
        .file("c/Cargo.toml", &basic_manifest("c", "0.1.0"))
        .file(
            "c/src/lib.rs",
            r#"
                /// ```rust
                /// assert_eq!(1, 2);
                /// ```
                pub fn foo() {}
            "#,
        )
        .file(
            "c/src/main.rs",
            r#"
                fn main() {}

                #[test]
                fn from_main() { assert_eq!(1, 2); }
            "#,
        )
        .file(
            "c/tests/t1.rs",
            r#"
                #[test]
                fn from_int_test() { assert_eq!(1, 2); }
            "#,
        )
        .file(
            "c/examples/ex1.rs",
            r#"
                fn main() {}

                #[test]
                fn from_example() { assert_eq!(1, 2); }
            "#,
        )
        // This does not use #[bench] since it is unstable. #[test] works just
        // the same for our purpose of checking the hint.
        .file(
            "c/benches/b1.rs",
            r#"
                #[test]
                fn from_bench() { assert_eq!(1, 2); }
            "#,
        )
        .build();

    // This depends on Units being sorted so that `b` fails first.
    p.cargo("test")
        .with_stderr_data(
            str![[r#"
[COMPILING] c v0.1.0 ([ROOT]/foo/c)
[COMPILING] a v0.1.0 ([ROOT]/foo/a)
[COMPILING] b v0.1.0 ([ROOT]/foo/b)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/deps/a-[HASH][EXE])
[RUNNING] unittests src/lib.rs (target/debug/deps/b-[HASH][EXE])
[ERROR] test failed, to rerun pass `-p b --lib`

"#]]
            .unordered(),
        )
        .with_status(101)
        .run();
    p.cargo("test")
        .cwd("b")
        .with_stderr_data(str![[r#"
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs ([ROOT]/foo/target/debug/deps/b-[HASH][EXE])
[ERROR] test failed, to rerun pass `--lib`

"#]])
        .with_status(101)
        .run();
    p.cargo("test --no-fail-fast")
        .with_stderr_data(str![[r#"
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/deps/a-[HASH][EXE])
[RUNNING] unittests src/lib.rs (target/debug/deps/b-[HASH][EXE])
[ERROR] test failed, to rerun pass `-p b --lib`
[RUNNING] unittests src/lib.rs (target/debug/deps/c-[HASH][EXE])
[RUNNING] unittests src/main.rs (target/debug/deps/c-[HASH][EXE])
[ERROR] test failed, to rerun pass `-p c --bin c`
[RUNNING] tests/t1.rs (target/debug/deps/t1-[HASH][EXE])
[ERROR] test failed, to rerun pass `-p c --test t1`
[DOCTEST] a
[DOCTEST] b
[DOCTEST] c
[ERROR] doctest failed, to rerun pass `-p c --doc`
[ERROR] 4 targets failed:
    `-p b --lib`
    `-p c --bin c`
    `-p c --test t1`
    `-p c --doc`

"#]])
        .with_status(101)
        .run();
    // Check others that are not in the default set.
    p.cargo("test -p c --examples --benches --no-fail-fast")
        .with_stderr_data(str![[r#"
[COMPILING] c v0.1.0 ([ROOT]/foo/c)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/deps/c-[HASH][EXE])
[RUNNING] unittests src/main.rs (target/debug/deps/c-[HASH][EXE])
[ERROR] test failed, to rerun pass `-p c --bin c`
[RUNNING] benches/b1.rs (target/debug/deps/b1-[HASH][EXE])
[ERROR] test failed, to rerun pass `-p c --bench b1`
[RUNNING] unittests examples/ex1.rs (target/debug/examples/ex1-[HASH][EXE])
[ERROR] test failed, to rerun pass `-p c --example ex1`
[ERROR] 3 targets failed:
    `-p c --bin c`
    `-p c --bench b1`
    `-p c --example ex1`

"#]])
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
            edition = "2015"

            [workspace]
            members = ["a"]
            "#,
        )
        .file("src/lib.rs", "")
        .file("a/Cargo.toml", &basic_manifest("a", "0.1.0"))
        .file("a/src/lib.rs", "#[test] fn t1() {assert!(false)}")
        .build();

    p.cargo("test --workspace")
        .with_stderr_data(str![[r#"
...
[ERROR] test failed, to rerun pass `-p a --lib`
...
"#]])
        .with_status(101)
        .run();
    p.cargo("test -p a")
        .with_stderr_data(str![[r#"
...
[ERROR] test failed, to rerun pass `-p a --lib`
...
"#]])
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
                edition = "2015"
                authors = []

                [profile.test]
                opt-level = 1
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("test --lib -v --no-run --message-format=json")
        .with_stdout_data(
            str![[r#"
[
  {
    "executable": "[ROOT]/foo/target/debug/deps/foo-[HASH][EXE]",
    "features": [],
    "filenames": "{...}",
    "fresh": false,
    "manifest_path": "[ROOT]/foo/Cargo.toml",
    "package_id": "path+[ROOTURL]/foo#0.0.1",
    "profile": "{...}",
    "reason": "compiler-artifact",
    "target": {
      "crate_types": [
        "lib"
      ],
      "doc": true,
      "doctest": true,
      "edition": "2015",
      "kind": [
        "lib"
      ],
      "name": "foo",
      "src_path": "[ROOT]/foo/src/lib.rs",
      "test": true
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

#[cargo_test]
fn json_artifact_includes_executable_for_library_tests() {
    let p = project()
        .file("src/main.rs", "fn main() { }")
        .file("src/lib.rs", r#"#[test] fn lib_test() {}"#)
        .build();

    p.cargo("test --lib -v --no-run --message-format=json")
        .with_stdout_data(
            str![[r#"
[
  {
    "executable": "[ROOT]/foo/target/debug/deps/foo-[HASH][EXE]",
    "features": [],
    "filenames": "{...}",
    "fresh": false,
    "manifest_path": "[ROOT]/foo/Cargo.toml",
    "package_id": "path+[ROOTURL]/foo#0.0.1",
    "profile": "{...}",
    "reason": "compiler-artifact",
    "target": {
      "crate_types": [
        "lib"
      ],
      "doc": true,
      "doctest": true,
      "edition": "2015",
      "kind": [
        "lib"
      ],
      "name": "foo",
      "src_path": "[ROOT]/foo/src/lib.rs",
      "test": true
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

#[cargo_test]
fn json_diagnostic_includes_explanation() {
    let p = project()
        .file(
            "src/main.rs",
            "fn main() { const OH_NO: &'static mut usize = &mut 1; }",
        )
        .build();

    p.cargo("check --message-format=json")
        .with_stdout_data(
            str![[r#"
[
  {
    "manifest_path": "[ROOT]/foo/Cargo.toml",
    "message": {
      "$message_type": "diagnostic",
      "children": "{...}",
      "code": {
        "code": "E0764",
        "explanation": "{...}"
      },
      "level": "error",
      "message": "{...}",
      "rendered": "{...}",
      "spans": "{...}"
    },
    "package_id": "{...}",
    "reason": "compiler-message",
    "target": "{...}"
  },
  "{...}"
]
"#]]
            .is_json()
            .against_jsonlines(),
        )
        .with_status(101)
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
        .with_stdout_data(
            str![[r#"
[
  {
    "executable": "[ROOT]/foo/target/debug/deps/integration_test-[HASH][EXE]",
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
        "test"
      ],
      "name": "integration_test",
      "src_path": "[ROOT]/foo/tests/integration_test.rs",
      "test": true
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

#[cargo_test]
fn test_build_script_links() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
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
                edition = "2015"

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
        .with_stderr_data(str![[r#"
[WARNING] doc tests are not supported for crate type(s) `staticlib` in package `foo`
[ERROR] no library targets found in package `foo`

"#]])
        .run();

    p.cargo("test")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/deps/foo-[HASH][EXE])

"#]])
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
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/deps/foo-[HASH][EXE])
[DOCTEST] foo

"#]])
        .with_stdout_data(str![[r#"

running 1 test
test tests::it_works ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s


running 1 test
test src/lib.rs - foo (line 1) ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .run();

    p.cargo("test --lib")
        .with_stderr_data(str![[r#"
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/deps/foo-[HASH][EXE])

"#]])
        .with_stdout_data(str![[r#"

running 1 test
test tests::it_works ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .run();

    // This has been modified to attempt to diagnose spurious errors on CI.
    // For some reason, this is recompiling the lib when it shouldn't. If the
    // root cause is ever found, the changes here should be reverted.
    // See https://github.com/rust-lang/cargo/issues/6887
    p.cargo("test --doc -vv")
        .with_stderr_does_not_contain("[COMPILING] foo [..]")
        .with_stderr_data(str![[r#"
...
[DOCTEST] foo
...
"#]])
        .with_stdout_data(str![[r#"

running 1 test
test src/lib.rs - foo (line 1) ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .env("CARGO_LOG", "cargo=trace")
        .run();

    p.cargo("test --lib --doc")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] Can't mix --doc with other target selecting options

"#]])
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
        .with_stderr_data(str![[r#"
[ERROR] Can't skip running doc tests with --no-run

"#]])
        .run();
}

#[cargo_test]
fn test_all_targets_lib() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("test --all-targets")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/deps/foo-[HASH][EXE])

"#]])
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
                edition = "2015"

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
                edition = "2015"

                [dev-dependencies]
                devdep = "0.1"
            "#,
        )
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("test -p bar")
        .with_status(101)
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[ERROR] package `bar` cannot be tested because it requires dev-dependencies and is not a member of the workspace

"#]])
        .run();
}

#[cargo_test]
fn cargo_test_doctest_xcompile_ignores() {
    // Check ignore-TARGET syntax.
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
        .with_stdout_data(str![[r#"
...
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s
...
"#]])
        .run();
    #[cfg(target_arch = "x86_64")]
    p.cargo("test")
        .with_stdout_data(str![[r#"
...
test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s
...
"#]])
        .run();
}

#[cargo_test]
fn cargo_test_doctest_xcompile_runner() {
    if !cross_compile_can_run_on_host() {
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

    let config = paths::root().join(".cargo/config.toml");

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
        .with_stdout_data(str![[r#"
...
running 0 tests
...
"#]])
        .run();
    p.cargo(&format!("test --target {}", cross_compile::alternate()))
        .with_stdout_data(str![[r#"
...
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s
...
"#]])
        .with_stderr_data(str![[r#"
...
this is a runner
...
"#]])
        .run();
}

#[cargo_test]
fn cargo_test_doctest_xcompile_no_runner() {
    if !cross_compile_can_run_on_host() {
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
        .with_stdout_data(str![[r#"
...
running 0 tests
...
"#]])
        .run();
    p.cargo(&format!("test --target {}", cross_compile::alternate()))
        .with_stdout_data(str![[r#"
...
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s
...
"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zpanic-abort-tests in rustc is unstable")]
fn panic_abort_tests() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = 'foo'
                version = '0.1.0'
                edition = "2015"

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

    // This uses -j1 because of a race condition. Otherwise it will build the
    // two copies of `foo` in parallel, and which one is first is random. If
    // `--test` is first, then the first line with `[..]` will match, and the
    // second line with `--test` will fail.
    p.cargo("test -Z panic-abort-tests -v -j1")
        .with_stderr_data(
            str![[r#"
[RUNNING] `[..]--crate-name a [..]-C panic=abort[..]`
[RUNNING] `[..]--crate-name foo [..]-C panic=abort[..]`
[RUNNING] `[..]--crate-name foo [..]-C panic=abort[..]--test[..]`
...
"#]]
            .unordered(),
        )
        .masquerade_as_nightly_cargo(&["panic-abort-tests"])
        .run();
}

#[cargo_test] // Unlike with rustc, `rustdoc --test -Cpanic=abort` already works on stable
fn panic_abort_doc_tests() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = 'foo'
                version = '0.1.0'
                edition = "2015"

                [profile.dev]
                panic = 'abort'
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                //! ```should_panic
                //! panic!();
                //! ```
            "#,
        )
        .build();

    p.cargo("test --doc -Z panic-abort-tests -v")
        .with_stderr_data(
            str![[r#"
[RUNNING] `[..]rustc[..] --crate-name foo [..]-C panic=abort[..]`
[RUNNING] `[..]rustdoc[..] --crate-name foo [..]--test[..]-C panic=abort[..]`
...
"#]]
            .unordered(),
        )
        .masquerade_as_nightly_cargo(&["panic-abort-tests"])
        .run();
}

#[cargo_test(nightly, reason = "-Zpanic-abort-tests in rustc is unstable")]
fn panic_abort_only_test() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = 'foo'
                version = '0.1.0'
                edition = "2015"

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
        .with_stderr_data(str![[r#"
[WARNING] `panic` setting is ignored for `test` profile
...
"#]])
        .masquerade_as_nightly_cargo(&["panic-abort-tests"])
        .run();
}

#[cargo_test(nightly, reason = "-Zpanic-abort-tests in rustc is unstable")]
fn panic_abort_test_profile_inherits() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = 'foo'
                version = '0.1.0'
                edition = "2015"

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
        .masquerade_as_nightly_cargo(&["panic-abort-tests"])
        .with_status(0)
        .run();
}

#[cargo_test]
fn bin_env_for_test() {
    // Test for the `CARGO_BIN_EXE_` environment variables for tests.
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
        .with_stderr_data(
            str![[r#"
[DOCTEST] root_crate
[DOCTEST] nested_crate
[DOCTEST] deep_crate
...
"#]]
            .unordered(),
        )
        .with_stdout_data(
            str![[r#"
test test_unit_root_cwd ... ok
test test_unit_nested_cwd ... ok
test test_unit_deep_cwd ... ok
test test_integration_root_cwd ... ok
test test_integration_nested_cwd ... ok
test test_integration_deep_cwd ... ok
...
"#]]
            .unordered(),
        )
        .run();

    p.cargo("test -p root-crate --all")
        .with_stderr_data(str![[r#"
...
[DOCTEST] root_crate
...
"#]])
        .with_stdout_data(
            str![[r#"
test test_unit_root_cwd ... ok
test test_integration_root_cwd ... ok
...
"#]]
            .unordered(),
        )
        .run();

    p.cargo("test -p nested-crate --all")
        .with_stderr_data(str![[r#"
...
[DOCTEST] nested_crate
...
"#]])
        .with_stdout_data(
            str![[r#"
test test_unit_nested_cwd ... ok
test test_integration_nested_cwd ... ok
...
"#]]
            .unordered(),
        )
        .run();

    p.cargo("test -p deep-crate --all")
        .with_stderr_data(str![[r#"
...
[DOCTEST] deep_crate
...
"#]])
        .with_stdout_data(
            str![[r#"
test test_unit_deep_cwd ... ok
test test_integration_deep_cwd ... ok
...
"#]]
            .unordered(),
        )
        .run();

    p.cargo("test --all")
        .cwd("nested-crate")
        .with_stderr_data(str![[r#"
...
[DOCTEST] nested_crate
...
"#]])
        .with_stdout_data(
            str![[r#"
test test_unit_nested_cwd ... ok
test test_integration_nested_cwd ... ok
...
"#]]
            .unordered(),
        )
        .run();

    p.cargo("test --all")
        .cwd("very/deeply/nested/deep-crate")
        .with_stderr_data(str![[r#"
...
[DOCTEST] deep_crate
...
"#]])
        .with_stdout_data(
            str![[r#"
test test_unit_deep_cwd ... ok
test test_integration_deep_cwd ... ok
...
"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn execution_error() {
    // Checks the behavior when a test fails to launch.
    let p = project()
        .file(
            "tests/t1.rs",
            r#"
                #[test]
                fn foo() {}
            "#,
        )
        .build();
    let key = format!("CARGO_TARGET_{}_RUNNER", rustc_host_env());
    p.cargo("test")
        .env(&key, "does_not_exist")
        // The actual error is usually "no such file", but on Windows it has a
        // custom message. Since matching against the error string produced by
        // Rust is not very reliable, this just uses `[..]`.
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] tests/t1.rs (target/debug/deps/t1-[HASH][EXE])
[ERROR] test failed, to rerun pass `--test t1`

Caused by:
  could not execute process `does_not_exist [ROOT]/foo/target/debug/deps/t1-[HASH][EXE]` (never executed)

Caused by:
  [NOT_FOUND]

"#]])
        .with_status(101)
        .run();
}

#[cargo_test]
fn nonzero_exit_status() {
    // Tests for nonzero exit codes from tests.
    let p = project()
        .file(
            "tests/t1.rs",
            r#"
                #[test]
                fn t() { panic!("this is a normal error") }
            "#,
        )
        .file(
            "tests/t2.rs",
            r#"
                #[test]
                fn t() { std::process::exit(4) }
            "#,
        )
        .build();

    p.cargo("test --test t1")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] tests/t1.rs (target/debug/deps/t1-[HASH][EXE])
[ERROR] test failed, to rerun pass `--test t1`

"#]])
        .with_stdout_data(str![[r#"
...
this is a normal error
...
"#]])
        .with_status(101)
        .run();

    p.cargo("test --test t2")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] tests/t2.rs (target/debug/deps/t2-[HASH][EXE])
[ERROR] test failed, to rerun pass `--test t2`

Caused by:
  process didn't exit successfully: `[ROOT]/foo/target/debug/deps/t2-[HASH][EXE]` ([EXIT_STATUS]: 4)
[NOTE] test exited abnormally; to see the full output pass --no-capture to the harness.

"#]])
        .with_status(4)
        .run();

    p.cargo("test --test t2 -- --no-capture")
        .with_stderr_data(str![[r#"
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] tests/t2.rs (target/debug/deps/t2-[HASH][EXE])
[ERROR] test failed, to rerun pass `--test t2`

Caused by:
  process didn't exit successfully: `[ROOT]/foo/target/debug/deps/t2-[HASH][EXE] --no-capture` ([EXIT_STATUS]: 4)

"#]])
        .with_status(4)
        .run();

    // no-fail-fast always uses 101
    p.cargo("test --no-fail-fast")
        .with_stderr_data(str![[r#"
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] tests/t1.rs (target/debug/deps/t1-[HASH][EXE])
[ERROR] test failed, to rerun pass `--test t1`
[RUNNING] tests/t2.rs (target/debug/deps/t2-[HASH][EXE])
[ERROR] test failed, to rerun pass `--test t2`

Caused by:
  process didn't exit successfully: `[ROOT]/foo/target/debug/deps/t2-[HASH][EXE]` ([EXIT_STATUS]: 4)
[NOTE] test exited abnormally; to see the full output pass --no-capture to the harness.
[ERROR] 2 targets failed:
    `--test t1`
    `--test t2`

"#]])
        .with_status(101)
        .run();

    p.cargo("test --no-fail-fast -- --no-capture")
        .with_stderr_does_not_contain(
            "test exited abnormally; to see the full output pass --no-capture to the harness.",
        )
        .with_stderr_data(str![[r#"
[..]thread [..]panicked [..] tests/t1.rs[..]
[NOTE] run with `RUST_BACKTRACE=1` environment variable to display a backtrace
Caused by:
  process didn't exit successfully: `[ROOT]/foo/target/debug/deps/t2-[HASH][EXE] --no-capture` ([EXIT_STATUS]: 4)
...
"#]].unordered())
        .with_status(101)
        .run();
}

#[cargo_test]
fn cargo_test_print_env_verbose() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.1"))
        .file("src/lib.rs", "")
        .build();

    p.cargo("test -vv").with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `[..]CARGO_MANIFEST_DIR=[ROOT]/foo[..] rustc --crate-name foo[..]`
[RUNNING] `[..]CARGO_MANIFEST_DIR=[ROOT]/foo[..] rustc --crate-name foo[..]`
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[..]CARGO_MANIFEST_DIR=[ROOT]/foo[..] [ROOT]/foo/target/debug/deps/foo-[HASH][EXE]`
[DOCTEST] foo
[RUNNING] `[..]CARGO_MANIFEST_DIR=[ROOT]/foo[..] rustdoc --edition=2015 --crate-type lib --color auto --crate-name foo[..]`

"#]]).run();
}

#[cargo_test]
fn cargo_test_set_out_dir_env_var() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2021"
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                pub fn add(left: u64, right: u64) -> u64 {
                    left + right
                }
            "#,
        )
        .file(
            "build.rs",
            r#"
                fn main() {}
            "#,
        )
        .file(
            "tests/case.rs",
            r#"
                #[cfg(test)]
                pub mod tests {
                    #[test]
                    fn test_add() {
                        assert!(std::env::var("OUT_DIR").is_ok());
                        assert_eq!(foo::add(2, 5), 7);
                    }
                }
            "#,
        )
        .build();

    p.cargo("test").run();
    p.cargo("test --package foo --test case -- tests::test_add --exact --no-capture")
        .run();
}
