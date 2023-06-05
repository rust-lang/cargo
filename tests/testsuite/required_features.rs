//! Tests for targets with `required-features`.

use cargo_test_support::install::{
    assert_has_installed_exe, assert_has_not_installed_exe, cargo_home,
};
use cargo_test_support::is_nightly;
use cargo_test_support::paths::CargoPathExt;
use cargo_test_support::project;

#[cargo_test]
fn build_bin_default_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [features]
                default = ["a"]
                a = []

                [[bin]]
                name = "foo"
                required-features = ["a"]
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                extern crate foo;

                #[cfg(feature = "a")]
                fn test() {
                    foo::foo();
                }

                fn main() {}
            "#,
        )
        .file("src/lib.rs", r#"#[cfg(feature = "a")] pub fn foo() {}"#)
        .build();

    p.cargo("build").run();
    assert!(p.bin("foo").is_file());

    p.cargo("build --no-default-features").run();

    p.cargo("build --bin=foo").run();
    assert!(p.bin("foo").is_file());

    p.cargo("build --bin=foo --no-default-features")
        .with_status(101)
        .with_stderr(
            "\
error: target `foo` in package `foo` requires the features: `a`
Consider enabling them by passing, e.g., `--features=\"a\"`
",
        )
        .run();
}

#[cargo_test]
fn build_bin_arg_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [features]
                a = []

                [[bin]]
                name = "foo"
                required-features = ["a"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build --features a").run();
    assert!(p.bin("foo").is_file());
}

#[cargo_test]
fn build_bin_multiple_required_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [features]
                default = ["a", "b"]
                a = []
                b = ["a"]
                c = []

                [[bin]]
                name = "foo_1"
                path = "src/foo_1.rs"
                required-features = ["b", "c"]

                [[bin]]
                name = "foo_2"
                path = "src/foo_2.rs"
                required-features = ["a"]
            "#,
        )
        .file("src/foo_1.rs", "fn main() {}")
        .file("src/foo_2.rs", "fn main() {}")
        .build();

    p.cargo("build").run();

    assert!(!p.bin("foo_1").is_file());
    assert!(p.bin("foo_2").is_file());

    p.cargo("build --features c").run();

    assert!(p.bin("foo_1").is_file());
    assert!(p.bin("foo_2").is_file());

    p.cargo("build --no-default-features").run();
}

#[cargo_test]
fn build_example_default_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [features]
                default = ["a"]
                a = []

                [[example]]
                name = "foo"
                required-features = ["a"]
            "#,
        )
        .file("examples/foo.rs", "fn main() {}")
        .build();

    p.cargo("build --example=foo").run();
    assert!(p.bin("examples/foo").is_file());

    p.cargo("build --example=foo --no-default-features")
        .with_status(101)
        .with_stderr(
            "\
error: target `foo` in package `foo` requires the features: `a`
Consider enabling them by passing, e.g., `--features=\"a\"`
",
        )
        .run();
}

#[cargo_test]
fn build_example_arg_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [features]
                a = []

                [[example]]
                name = "foo"
                required-features = ["a"]
            "#,
        )
        .file("examples/foo.rs", "fn main() {}")
        .build();

    p.cargo("build --example=foo --features a").run();
    assert!(p.bin("examples/foo").is_file());
}

#[cargo_test]
fn build_example_multiple_required_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [features]
                default = ["a", "b"]
                a = []
                b = ["a"]
                c = []

                [[example]]
                name = "foo_1"
                required-features = ["b", "c"]

                [[example]]
                name = "foo_2"
                required-features = ["a"]
            "#,
        )
        .file("examples/foo_1.rs", "fn main() {}")
        .file("examples/foo_2.rs", "fn main() {}")
        .build();

    p.cargo("build --example=foo_1")
        .with_status(101)
        .with_stderr(
            "\
error: target `foo_1` in package `foo` requires the features: `b`, `c`
Consider enabling them by passing, e.g., `--features=\"b c\"`
",
        )
        .run();
    p.cargo("build --example=foo_2").run();

    assert!(!p.bin("examples/foo_1").is_file());
    assert!(p.bin("examples/foo_2").is_file());

    p.cargo("build --example=foo_1 --features c").run();
    p.cargo("build --example=foo_2 --features c").run();

    assert!(p.bin("examples/foo_1").is_file());
    assert!(p.bin("examples/foo_2").is_file());

    p.cargo("build --example=foo_1 --no-default-features")
        .with_status(101)
        .with_stderr(
            "\
error: target `foo_1` in package `foo` requires the features: `b`, `c`
Consider enabling them by passing, e.g., `--features=\"b c\"`
",
        )
        .run();
    p.cargo("build --example=foo_2 --no-default-features")
        .with_status(101)
        .with_stderr(
            "\
error: target `foo_2` in package `foo` requires the features: `a`
Consider enabling them by passing, e.g., `--features=\"a\"`
",
        )
        .run();
}

#[cargo_test]
fn test_default_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [features]
                default = ["a"]
                a = []

                [[test]]
                name = "foo"
                required-features = ["a"]
            "#,
        )
        .file("tests/foo.rs", "#[test]\nfn test() {}")
        .build();

    p.cargo("test")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/debug/deps/foo-[..][EXE])",
        )
        .with_stdout_contains("test test ... ok")
        .run();

    p.cargo("test --no-default-features")
        .with_stderr("[FINISHED] test [unoptimized + debuginfo] target(s) in [..]")
        .with_stdout("")
        .run();

    p.cargo("test --test=foo")
        .with_stderr(
            "\
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/debug/deps/foo-[..][EXE])",
        )
        .with_stdout_contains("test test ... ok")
        .run();

    p.cargo("test --test=foo --no-default-features")
        .with_status(101)
        .with_stderr(
            "\
error: target `foo` in package `foo` requires the features: `a`
Consider enabling them by passing, e.g., `--features=\"a\"`
",
        )
        .run();
}

#[cargo_test]
fn test_arg_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [features]
                a = []

                [[test]]
                name = "foo"
                required-features = ["a"]
            "#,
        )
        .file("tests/foo.rs", "#[test]\nfn test() {}")
        .build();

    p.cargo("test --features a")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/debug/deps/foo-[..][EXE])",
        )
        .with_stdout_contains("test test ... ok")
        .run();
}

#[cargo_test]
fn test_multiple_required_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [features]
                default = ["a", "b"]
                a = []
                b = ["a"]
                c = []

                [[test]]
                name = "foo_1"
                required-features = ["b", "c"]

                [[test]]
                name = "foo_2"
                required-features = ["a"]
            "#,
        )
        .file("tests/foo_1.rs", "#[test]\nfn test() {}")
        .file("tests/foo_2.rs", "#[test]\nfn test() {}")
        .build();

    p.cargo("test")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/debug/deps/foo_2-[..][EXE])",
        )
        .with_stdout_contains("test test ... ok")
        .run();

    p.cargo("test --features c")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/debug/deps/foo_1-[..][EXE])
[RUNNING] [..] (target/debug/deps/foo_2-[..][EXE])",
        )
        .with_stdout_contains_n("test test ... ok", 2)
        .run();

    p.cargo("test --no-default-features")
        .with_stderr("[FINISHED] test [unoptimized + debuginfo] target(s) in [..]")
        .with_stdout("")
        .run();
}

#[cargo_test(nightly, reason = "bench")]
fn bench_default_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [features]
                default = ["a"]
                a = []

                [[bench]]
                name = "foo"
                required-features = ["a"]
            "#,
        )
        .file(
            "benches/foo.rs",
            r#"
            #![feature(test)]
            extern crate test;

            #[bench]
            fn bench(_: &mut test::Bencher) {
            }
            "#,
        )
        .build();

    p.cargo("bench")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] bench [optimized] target(s) in [..]
[RUNNING] [..] (target/release/deps/foo-[..][EXE])",
        )
        .with_stdout_contains("test bench ... bench: [..]")
        .run();

    p.cargo("bench --no-default-features")
        .with_stderr("[FINISHED] bench [optimized] target(s) in [..]".to_string())
        .with_stdout("")
        .run();

    p.cargo("bench --bench=foo")
        .with_stderr(
            "\
[FINISHED] bench [optimized] target(s) in [..]
[RUNNING] [..] (target/release/deps/foo-[..][EXE])",
        )
        .with_stdout_contains("test bench ... bench: [..]")
        .run();

    p.cargo("bench --bench=foo --no-default-features")
        .with_status(101)
        .with_stderr(
            "\
error: target `foo` in package `foo` requires the features: `a`
Consider enabling them by passing, e.g., `--features=\"a\"`
",
        )
        .run();
}

#[cargo_test(nightly, reason = "bench")]
fn bench_arg_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [features]
                a = []

                [[bench]]
                name = "foo"
                required-features = ["a"]
            "#,
        )
        .file(
            "benches/foo.rs",
            r#"
            #![feature(test)]
            extern crate test;

            #[bench]
            fn bench(_: &mut test::Bencher) {
            }
            "#,
        )
        .build();

    p.cargo("bench --features a")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] bench [optimized] target(s) in [..]
[RUNNING] [..] (target/release/deps/foo-[..][EXE])",
        )
        .with_stdout_contains("test bench ... bench: [..]")
        .run();
}

#[cargo_test(nightly, reason = "bench")]
fn bench_multiple_required_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [features]
                default = ["a", "b"]
                a = []
                b = ["a"]
                c = []

                [[bench]]
                name = "foo_1"
                required-features = ["b", "c"]

                [[bench]]
                name = "foo_2"
                required-features = ["a"]
            "#,
        )
        .file(
            "benches/foo_1.rs",
            r#"
            #![feature(test)]
            extern crate test;

            #[bench]
            fn bench(_: &mut test::Bencher) {
            }
            "#,
        )
        .file(
            "benches/foo_2.rs",
            r#"
            #![feature(test)]
            extern crate test;

            #[bench]
            fn bench(_: &mut test::Bencher) {
            }
            "#,
        )
        .build();

    p.cargo("bench")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] bench [optimized] target(s) in [..]
[RUNNING] [..] (target/release/deps/foo_2-[..][EXE])",
        )
        .with_stdout_contains("test bench ... bench: [..]")
        .run();

    p.cargo("bench --features c")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] bench [optimized] target(s) in [..]
[RUNNING] [..] (target/release/deps/foo_1-[..][EXE])
[RUNNING] [..] (target/release/deps/foo_2-[..][EXE])",
        )
        .with_stdout_contains_n("test bench ... bench: [..]", 2)
        .run();

    p.cargo("bench --no-default-features")
        .with_stderr("[FINISHED] bench [optimized] target(s) in [..]")
        .with_stdout("")
        .run();
}

#[cargo_test]
fn install_default_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [features]
                default = ["a"]
                a = []

                [[bin]]
                name = "foo"
                required-features = ["a"]

                [[example]]
                name = "foo"
                required-features = ["a"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("examples/foo.rs", "fn main() {}")
        .build();

    p.cargo("install --path .").run();
    assert_has_installed_exe(cargo_home(), "foo");
    p.cargo("uninstall foo").run();

    p.cargo("install --path . --no-default-features")
        .with_stderr(
            "\
[INSTALLING] foo v0.0.1 ([..])
[FINISHED] release [optimized] target(s) in [..]
[WARNING] none of the package's binaries are available for install using the selected features
  bin \"foo\" requires the features: `a`
  example \"foo\" requires the features: `a`
Consider enabling some of the needed features by passing, e.g., `--features=\"a\"`
",
        )
        .run();
    assert_has_not_installed_exe(cargo_home(), "foo");

    p.cargo("install --path . --bin=foo").run();
    assert_has_installed_exe(cargo_home(), "foo");
    p.cargo("uninstall foo").run();

    p.cargo("install --path . --bin=foo --no-default-features")
        .with_status(101)
        .with_stderr(
            "\
[INSTALLING] foo v0.0.1 ([..])
[ERROR] failed to compile `foo v0.0.1 ([..])`, intermediate artifacts can be found at \
    `[..]target`.
To reuse those artifacts with a future compilation, set the environment \
variable `CARGO_TARGET_DIR` to that path.

Caused by:
  target `foo` in package `foo` requires the features: `a`
  Consider enabling them by passing, e.g., `--features=\"a\"`
",
        )
        .run();
    assert_has_not_installed_exe(cargo_home(), "foo");

    p.cargo("install --path . --example=foo").run();
    assert_has_installed_exe(cargo_home(), "foo");
    p.cargo("uninstall foo").run();

    p.cargo("install --path . --example=foo --no-default-features")
        .with_status(101)
        .with_stderr(
            "\
[INSTALLING] foo v0.0.1 ([..])
[ERROR] failed to compile `foo v0.0.1 ([..])`, intermediate artifacts can be found at \
    `[..]target`.\nTo reuse those artifacts with a future compilation, set the environment \
    variable `CARGO_TARGET_DIR` to that path.

Caused by:
  target `foo` in package `foo` requires the features: `a`
  Consider enabling them by passing, e.g., `--features=\"a\"`
",
        )
        .run();
    assert_has_not_installed_exe(cargo_home(), "foo");
}

#[cargo_test]
fn install_arg_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [features]
                a = []

                [[bin]]
                name = "foo"
                required-features = ["a"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("install --features a").run();
    assert_has_installed_exe(cargo_home(), "foo");
    p.cargo("uninstall foo").run();
}

#[cargo_test]
fn install_multiple_required_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [features]
                default = ["a", "b"]
                a = []
                b = ["a"]
                c = []

                [[bin]]
                name = "foo_1"
                path = "src/foo_1.rs"
                required-features = ["b", "c"]

                [[bin]]
                name = "foo_2"
                path = "src/foo_2.rs"
                required-features = ["a"]

                [[example]]
                name = "foo_3"
                path = "src/foo_3.rs"
                required-features = ["b", "c"]

                [[example]]
                name = "foo_4"
                path = "src/foo_4.rs"
                required-features = ["a"]
            "#,
        )
        .file("src/foo_1.rs", "fn main() {}")
        .file("src/foo_2.rs", "fn main() {}")
        .file("src/foo_3.rs", "fn main() {}")
        .file("src/foo_4.rs", "fn main() {}")
        .build();

    p.cargo("install --path .").run();
    assert_has_not_installed_exe(cargo_home(), "foo_1");
    assert_has_installed_exe(cargo_home(), "foo_2");
    assert_has_not_installed_exe(cargo_home(), "foo_3");
    assert_has_not_installed_exe(cargo_home(), "foo_4");
    p.cargo("uninstall foo").run();

    p.cargo("install --path . --bins --examples").run();
    assert_has_not_installed_exe(cargo_home(), "foo_1");
    assert_has_installed_exe(cargo_home(), "foo_2");
    assert_has_not_installed_exe(cargo_home(), "foo_3");
    assert_has_installed_exe(cargo_home(), "foo_4");
    p.cargo("uninstall foo").run();

    p.cargo("install --path . --features c").run();
    assert_has_installed_exe(cargo_home(), "foo_1");
    assert_has_installed_exe(cargo_home(), "foo_2");
    assert_has_not_installed_exe(cargo_home(), "foo_3");
    assert_has_not_installed_exe(cargo_home(), "foo_4");
    p.cargo("uninstall foo").run();

    p.cargo("install --path . --features c --bins --examples")
        .run();
    assert_has_installed_exe(cargo_home(), "foo_1");
    assert_has_installed_exe(cargo_home(), "foo_2");
    assert_has_installed_exe(cargo_home(), "foo_3");
    assert_has_installed_exe(cargo_home(), "foo_4");
    p.cargo("uninstall foo").run();

    p.cargo("install --path . --no-default-features")
        .with_stderr(
            "\
[INSTALLING] foo v0.0.1 ([..])
[FINISHED] release [optimized] target(s) in [..]
[WARNING] none of the package's binaries are available for install using the selected features
  bin \"foo_1\" requires the features: `b`, `c`
  bin \"foo_2\" requires the features: `a`
  example \"foo_3\" requires the features: `b`, `c`
  example \"foo_4\" requires the features: `a`
Consider enabling some of the needed features by passing, e.g., `--features=\"b c\"`
",
        )
        .run();
    p.cargo("install --path . --no-default-features --bins")
        .with_stderr(
            "\
[INSTALLING] foo v0.0.1 ([..])
[WARNING] Target filter `bins` specified, but no targets matched. This is a no-op
[FINISHED] release [optimized] target(s) in [..]
[WARNING] none of the package's binaries are available for install using the selected features
  bin \"foo_1\" requires the features: `b`, `c`
  bin \"foo_2\" requires the features: `a`
  example \"foo_3\" requires the features: `b`, `c`
  example \"foo_4\" requires the features: `a`
Consider enabling some of the needed features by passing, e.g., `--features=\"b c\"`
",
        )
        .run();
    p.cargo("install --path . --no-default-features --examples")
        .with_stderr(
            "\
[INSTALLING] foo v0.0.1 ([..])
[WARNING] Target filter `examples` specified, but no targets matched. This is a no-op
[FINISHED] release [optimized] target(s) in [..]
[WARNING] none of the package's binaries are available for install using the selected features
  bin \"foo_1\" requires the features: `b`, `c`
  bin \"foo_2\" requires the features: `a`
  example \"foo_3\" requires the features: `b`, `c`
  example \"foo_4\" requires the features: `a`
Consider enabling some of the needed features by passing, e.g., `--features=\"b c\"`
",
        )
        .run();
    p.cargo("install --path . --no-default-features --bins --examples")
        .with_stderr(
            "\
[INSTALLING] foo v0.0.1 ([..])
[WARNING] Target filters `bins`, `examples` specified, but no targets matched. This is a no-op
[FINISHED] release [optimized] target(s) in [..]
[WARNING] none of the package's binaries are available for install using the selected features
  bin \"foo_1\" requires the features: `b`, `c`
  bin \"foo_2\" requires the features: `a`
  example \"foo_3\" requires the features: `b`, `c`
  example \"foo_4\" requires the features: `a`
Consider enabling some of the needed features by passing, e.g., `--features=\"b c\"`
",
        )
        .run();
    assert_has_not_installed_exe(cargo_home(), "foo_1");
    assert_has_not_installed_exe(cargo_home(), "foo_2");
    assert_has_not_installed_exe(cargo_home(), "foo_3");
    assert_has_not_installed_exe(cargo_home(), "foo_4");
}

#[cargo_test]
fn dep_feature_in_toml() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                bar = { path = "bar", features = ["a"] }

                [[bin]]
                name = "foo"
                required-features = ["bar/a"]

                [[example]]
                name = "foo"
                required-features = ["bar/a"]

                [[test]]
                name = "foo"
                required-features = ["bar/a"]

                [[bench]]
                name = "foo"
                required-features = ["bar/a"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("examples/foo.rs", "fn main() {}")
        .file("tests/foo.rs", "#[test]\nfn test() {}")
        .file(
            "benches/foo.rs",
            r#"
            #![feature(test)]
            extern crate test;

            #[bench]
            fn bench(_: &mut test::Bencher) {
            }
            "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                authors = []

                [features]
                a = []
            "#,
        )
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("build").run();

    // bin
    p.cargo("build --bin=foo").run();
    assert!(p.bin("foo").is_file());

    // example
    p.cargo("build --example=foo").run();
    assert!(p.bin("examples/foo").is_file());

    // test
    p.cargo("test --test=foo")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/debug/deps/foo-[..][EXE])",
        )
        .with_stdout_contains("test test ... ok")
        .run();

    // bench
    if is_nightly() {
        p.cargo("bench --bench=foo")
            .with_stderr(
                "\
[COMPILING] bar v0.0.1 ([CWD]/bar)
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] bench [optimized] target(s) in [..]
[RUNNING] [..] (target/release/deps/foo-[..][EXE])",
            )
            .with_stdout_contains("test bench ... bench: [..]")
            .run();
    }

    // install
    p.cargo("install").run();
    assert_has_installed_exe(cargo_home(), "foo");
    p.cargo("uninstall foo").run();
}

#[cargo_test]
fn dep_feature_in_cmd_line() {
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

                [[bin]]
                name = "foo"
                required-features = ["bar/a"]

                [[example]]
                name = "foo"
                required-features = ["bar/a"]

                [[test]]
                name = "foo"
                required-features = ["bar/a"]

                [[bench]]
                name = "foo"
                required-features = ["bar/a"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("examples/foo.rs", "fn main() {}")
        .file(
            "tests/foo.rs",
            r#"
            #[test]
            fn bin_is_built() {
                let s = format!("target/debug/foo{}", std::env::consts::EXE_SUFFIX);
                let p = std::path::Path::new(&s);
                assert!(p.exists(), "foo does not exist");
            }
            "#,
        )
        .file(
            "benches/foo.rs",
            r#"
            #![feature(test)]
            extern crate test;

            #[bench]
            fn bench(_: &mut test::Bencher) {
            }
            "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                authors = []

                [features]
                a = []
            "#,
        )
        .file("bar/src/lib.rs", "")
        .build();

    // This is a no-op
    p.cargo("build").with_stderr("[FINISHED] dev [..]").run();
    assert!(!p.bin("foo").is_file());

    // bin
    p.cargo("build --bin=foo")
        .with_status(101)
        .with_stderr(
            "\
error: target `foo` in package `foo` requires the features: `bar/a`
Consider enabling them by passing, e.g., `--features=\"bar/a\"`
",
        )
        .run();

    p.cargo("build --bin=foo --features bar/a").run();
    assert!(p.bin("foo").is_file());

    // example
    p.cargo("build --example=foo")
        .with_status(101)
        .with_stderr(
            "\
error: target `foo` in package `foo` requires the features: `bar/a`
Consider enabling them by passing, e.g., `--features=\"bar/a\"`
",
        )
        .run();

    p.cargo("build --example=foo --features bar/a").run();
    assert!(p.bin("examples/foo").is_file());

    // test
    // This is a no-op, since no tests are enabled
    p.cargo("test")
        .with_stderr("[FINISHED] test [unoptimized + debuginfo] target(s) in [..]")
        .with_stdout("")
        .run();

    // Delete the target directory so this can check if the main.rs gets built.
    p.build_dir().rm_rf();
    p.cargo("test --test=foo --features bar/a")
        .with_stderr(
            "\
[COMPILING] bar v0.0.1 ([CWD]/bar)
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/debug/deps/foo-[..][EXE])",
        )
        .with_stdout_contains("test bin_is_built ... ok")
        .run();

    // bench
    if is_nightly() {
        p.cargo("bench")
            .with_stderr("[FINISHED] bench [optimized] target(s) in [..]")
            .with_stdout("")
            .run();

        p.cargo("bench --bench=foo --features bar/a")
            .with_stderr(
                "\
[COMPILING] bar v0.0.1 ([CWD]/bar)
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] bench [optimized] target(s) in [..]
[RUNNING] [..] (target/release/deps/foo-[..][EXE])",
            )
            .with_stdout_contains("test bench ... bench: [..]")
            .run();
    }

    // install
    p.cargo("install --path .")
        .with_stderr(
            "\
[INSTALLING] foo v0.0.1 ([..])
[FINISHED] release [optimized] target(s) in [..]
[WARNING] none of the package's binaries are available for install using the selected features
  bin \"foo\" requires the features: `bar/a`
  example \"foo\" requires the features: `bar/a`
Consider enabling some of the needed features by passing, e.g., `--features=\"bar/a\"`
",
        )
        .run();
    assert_has_not_installed_exe(cargo_home(), "foo");

    p.cargo("install --features bar/a").run();
    assert_has_installed_exe(cargo_home(), "foo");
    p.cargo("uninstall foo").run();
}

#[cargo_test]
fn test_skips_compiling_bin_with_missing_required_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [features]
                a = []

                [[bin]]
                name = "bin_foo"
                path = "src/bin/foo.rs"
                required-features = ["a"]
            "#,
        )
        .file("src/bin/foo.rs", "extern crate bar; fn main() {}")
        .file("tests/foo.rs", "")
        .file("benches/foo.rs", "")
        .build();

    p.cargo("test")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/debug/deps/foo-[..][EXE])",
        )
        .with_stdout_contains("running 0 tests")
        .run();

    p.cargo("test --features a -j 1")
        .with_status(101)
        .with_stderr_contains(
            "\
[COMPILING] foo v0.0.1 ([CWD])
error[E0463]: can't find crate for `bar`",
        )
        .run();

    if is_nightly() {
        p.cargo("bench")
            .with_stderr(
                "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] bench [optimized] target(s) in [..]
[RUNNING] [..] (target/release/deps/foo-[..][EXE])",
            )
            .with_stdout_contains("running 0 tests")
            .run();

        p.cargo("bench --features a -j 1")
            .with_status(101)
            .with_stderr_contains(
                "\
[COMPILING] foo v0.0.1 ([CWD])
error[E0463]: can't find crate for `bar`",
            )
            .run();
    }
}

#[cargo_test]
fn run_default() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [features]
                default = []
                a = []

                [[bin]]
                name = "foo"
                required-features = ["a"]
            "#,
        )
        .file("src/lib.rs", "")
        .file("src/main.rs", "extern crate foo; fn main() {}")
        .build();

    p.cargo("run")
        .with_status(101)
        .with_stderr(
            "\
error: target `foo` in package `foo` requires the features: `a`
Consider enabling them by passing, e.g., `--features=\"a\"`
",
        )
        .run();

    p.cargo("run --features a").run();
}

#[cargo_test]
fn run_default_multiple_required_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [features]
                default = ["a"]
                a = []
                b = []

                [[bin]]
                name = "foo1"
                path = "src/foo1.rs"
                required-features = ["a"]

                [[bin]]
                name = "foo3"
                path = "src/foo3.rs"
                required-features = ["b"]

                [[bin]]
                name = "foo2"
                path = "src/foo2.rs"
                required-features = ["b"]
            "#,
        )
        .file("src/lib.rs", "")
        .file("src/foo1.rs", "extern crate foo; fn main() {}")
        .file("src/foo3.rs", "extern crate foo; fn main() {}")
        .file("src/foo2.rs", "extern crate foo; fn main() {}")
        .build();

    p.cargo("run")
        .with_status(101)
        .with_stderr(
            "\
error: `cargo run` could not determine which binary to run[..]
available binaries: foo1, foo2, foo3",
        )
        .run();
}

#[cargo_test]
fn renamed_required_features() {
    // Test that required-features uses renamed package feature names.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2018"

            [[bin]]
            name = "x"
            required-features = ["a1/f1"]

            [dependencies]
            a1 = {path="a1", package="a"}
            a2 = {path="a2", package="a"}
            "#,
        )
        .file(
            "src/bin/x.rs",
            r#"
            fn main() {
                a1::f();
                a2::f();
            }
            "#,
        )
        .file(
            "a1/Cargo.toml",
            r#"
            [package]
            name = "a"
            version = "0.1.0"

            [features]
            f1 = []
            "#,
        )
        .file(
            "a1/src/lib.rs",
            r#"
            pub fn f() {
                if cfg!(feature="f1") {
                    println!("a1 f1");
                }
            }
            "#,
        )
        .file(
            "a2/Cargo.toml",
            r#"
              [package]
             name = "a"
             version = "0.2.0"

             [features]
             f2 = []
            "#,
        )
        .file(
            "a2/src/lib.rs",
            r#"
            pub fn f() {
                if cfg!(feature="f2") {
                    println!("a2 f2");
                }
            }
            "#,
        )
        .build();

    p.cargo("run")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] target `x` in package `foo` requires the features: `a1/f1`
Consider enabling them by passing, e.g., `--features=\"a1/f1\"`
",
        )
        .run();

    p.cargo("build --features a1/f1").run();
    p.rename_run("x", "x_with_f1").with_stdout("a1 f1").run();

    p.cargo("build --features a1/f1,a2/f2").run();
    p.rename_run("x", "x_with_f1_f2")
        .with_stdout("a1 f1\na2 f2")
        .run();
}

#[cargo_test]
fn truncated_install_warning_message() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
    [package]
    name = "foo"
    version = "0.1.0"
    edition = "2021"

    [features]
    feature1 = []
    feature2 = []
    feature3 = []
    feature4 = []
    feature5 = []

    [[bin]]
    name = "foo1"
    required-features = ["feature1", "feature2", "feature3"]

    [[bin]]
    name = "foo2"
    required-features = ["feature2"]

    [[bin]]
    name = "foo3"
    required-features = ["feature3"]

    [[bin]]
    name = "foo4"
    required-features = ["feature4", "feature1"]

    [[bin]]
    name = "foo5"
    required-features = ["feature1", "feature2", "feature3", "feature4", "feature5"]

    [[bin]]
    name = "foo6"
    required-features = ["feature1", "feature2", "feature3", "feature4", "feature5"]

    [[bin]]
    name = "foo7"
    required-features = ["feature1", "feature2", "feature3", "feature4", "feature5"]

    [[bin]]
    name = "foo8"
    required-features = ["feature1", "feature2", "feature3", "feature4", "feature5"]

    [[bin]]
    name = "foo9"
    required-features = ["feature1", "feature2", "feature3", "feature4", "feature5"]

    [[bin]]
    name = "foo10"
    required-features = ["feature1", "feature2", "feature3", "feature4", "feature5"]

    [[example]]
    name = "example1"
    required-features = ["feature1", "feature2"]
    "#,
        )
        .file("src/bin/foo1.rs", "fn main() {}")
        .file("src/bin/foo2.rs", "fn main() {}")
        .file("src/bin/foo3.rs", "fn main() {}")
        .file("src/bin/foo4.rs", "fn main() {}")
        .file("src/bin/foo5.rs", "fn main() {}")
        .file("src/bin/foo6.rs", "fn main() {}")
        .file("src/bin/foo7.rs", "fn main() {}")
        .file("src/bin/foo8.rs", "fn main() {}")
        .file("src/bin/foo9.rs", "fn main() {}")
        .file("src/bin/foo10.rs", "fn main() {}")
        .file("examples/example1.rs", "fn main() {}")
        .build();

    p.cargo("install --path .").with_stderr("\
[INSTALLING] foo v0.1.0 ([..])
[FINISHED] release [optimized] target(s) in [..]
[WARNING] none of the package's binaries are available for install using the selected features
  bin \"foo1\" requires the features: `feature1`, `feature2`, `feature3`
  bin \"foo2\" requires the features: `feature2`
  bin \"foo3\" requires the features: `feature3`
  bin \"foo4\" requires the features: `feature4`, `feature1`
  bin \"foo5\" requires the features: `feature1`, `feature2`, `feature3`, `feature4`, `feature5`
  bin \"foo6\" requires the features: `feature1`, `feature2`, `feature3`, `feature4`, `feature5`
  bin \"foo7\" requires the features: `feature1`, `feature2`, `feature3`, `feature4`, `feature5`
4 more targets also requires features not enabled. See them in the Cargo.toml file.
Consider enabling some of the needed features by passing, e.g., `--features=\"feature1 feature2 feature3\"`").run();
}
