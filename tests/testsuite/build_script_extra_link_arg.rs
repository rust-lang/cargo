//! Tests for additional link arguments.

// NOTE: Many of these tests use `without_status()` when passing bogus flags
// because MSVC link.exe just gives a warning on unknown flags (how helpful!),
// and other linkers will return an error.

use cargo_test_support::prelude::*;
use cargo_test_support::registry::Package;
use cargo_test_support::str;
use cargo_test_support::{basic_bin_manifest, basic_lib_manifest, basic_manifest, project};

#[cargo_test]
fn build_script_extra_link_arg_bin() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo::rustc-link-arg-bins=--this-is-a-bogus-flag");
                }
            "#,
        )
        .build();

    p.cargo("build -v")
        .without_status()
        .with_stderr_data(
            "\
...
[RUNNING] `rustc --crate-name foo [..]-C link-arg=--this-is-a-bogus-flag[..]
...",
        )
        .run();
}

#[cargo_test]
fn build_script_extra_link_arg_bin_single() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]

                name = "foobar"
                version = "0.5.0"
                edition = "2015"
                authors = ["wycats@example.com"]

                [[bin]]
                name = "foo"
                [[bin]]
                name = "bar"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo::rustc-link-arg-bins=--bogus-flag-all");
                    println!("cargo::rustc-link-arg-bin=foo=--bogus-flag-foo");
                    println!("cargo::rustc-link-arg-bin=bar=--bogus-flag-bar");
                }
            "#,
        )
        .build();

    p.cargo("build -v")
        .without_status()
        .with_stderr_data(
            "\
...
[RUNNING] `rustc --crate-name foo [..]-C link-arg=--bogus-flag-all -C link-arg=--bogus-flag-foo[..]
...",
        )
        .with_stderr_data(
            "\
...
[RUNNING] `rustc --crate-name bar [..]-C link-arg=--bogus-flag-all -C link-arg=--bogus-flag-bar[..]
...",
        )
        .run();
}

#[cargo_test]
fn build_script_extra_link_arg() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo::rustc-link-arg=--this-is-a-bogus-flag");
                }
            "#,
        )
        .build();

    p.cargo("build -v")
        .without_status()
        .with_stderr_data(
            "\
...
[RUNNING] `rustc --crate-name foo [..]-C link-arg=--this-is-a-bogus-flag[..]
...",
        )
        .run();
}

#[cargo_test]
fn link_arg_missing_target() {
    // Errors when a given target doesn't exist.
    let p = project()
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"fn main() { println!("cargo::rustc-link-arg-cdylib=--bogus"); }"#,
        )
        .build();

    // TODO: Uncomment this if cdylib restriction is re-added (see
    // cdylib_link_arg_transitive below).
    //     p.cargo("check")
    //         .with_status(101)
    //         .with_stderr("\
    // [COMPILING] foo [..]
    // error: invalid instruction `cargo::rustc-link-arg-cdylib` from build script of `foo v0.0.1 ([ROOT]/foo)`
    // The package foo v0.0.1 ([ROOT]/foo) does not have a cdylib target.
    // ")
    //         .run();

    p.change_file(
        "build.rs",
        r#"fn main() { println!("cargo::rustc-link-arg-bins=--bogus"); }"#,
    );

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[ERROR] invalid instruction `cargo::rustc-link-arg-bins` from build script of `foo v0.0.1 ([ROOT]/foo)`
The package foo v0.0.1 ([ROOT]/foo) does not have a bin target.

"#]])
        .run();

    p.change_file(
        "build.rs",
        r#"fn main() { println!("cargo::rustc-link-arg-bin=abc=--bogus"); }"#,
    );

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[ERROR] invalid instruction `cargo::rustc-link-arg-bin` from build script of `foo v0.0.1 ([ROOT]/foo)`
The package foo v0.0.1 ([ROOT]/foo) does not have a bin target with the name `abc`.

"#]])
        .run();

    p.change_file(
        "build.rs",
        r#"fn main() { println!("cargo::rustc-link-arg-bin=abc"); }"#,
    );

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[ERROR] invalid instruction `cargo::rustc-link-arg-bin=abc` from build script of `foo v0.0.1 ([ROOT]/foo)`
The instruction should have the form cargo::rustc-link-arg-bin=BIN=ARG

"#]])
        .run();
}

#[cargo_test]
fn cdylib_link_arg_transitive() {
    // There was an unintended regression in 1.50 where rustc-link-arg-cdylib
    // arguments from dependencies were being applied in the parent package.
    // Previously it was silently ignored.
    // See https://github.com/rust-lang/cargo/issues/9562
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [lib]
                crate-type = ["cdylib"]

                [dependencies]
                bar = {path="bar"}
            "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "1.0.0"))
        .file("bar/src/lib.rs", "")
        .file(
            "bar/build.rs",
            r#"
                fn main() {
                    println!("cargo::rustc-link-arg-cdylib=--bogus");
                }
            "#,
        )
        .build();
    p.cargo("build -v")
        .without_status()
        .with_stderr_data(
            "\
...
[COMPILING] bar v1.0.0 ([ROOT]/foo/bar)
[RUNNING] `rustc --crate-name build_script_build --edition=2015 bar/build.rs [..]
[RUNNING] `[ROOT]/foo/target/debug/build/bar-[HASH]/build-script-build`
[WARNING] bar@1.0.0: cargo::rustc-link-arg-cdylib was specified in the build script of bar v1.0.0 \
([ROOT]/foo/bar), but that package does not contain a cdylib target

Allowing this was an unintended change in the 1.50 release, and may become an error in \
the future. For more information, see <https://github.com/rust-lang/cargo/issues/9562>.
[RUNNING] `rustc --crate-name bar --edition=2015 bar/src/lib.rs [..]
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo --edition=2015 src/lib.rs [..]-C link-arg=--bogus[..]`
...",
        )
        .run();
}

#[cargo_test]
fn link_arg_transitive_not_allowed() {
    // Verify that transitive dependencies don't pass link args.
    //
    // Note that rustc-link-arg doesn't have any errors or warnings when it is
    // unused. Perhaps that could be more aggressive, but it is difficult
    // since it could be used for test binaries.
    Package::new("bar", "1.0.0")
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo::rustc-link-arg=--bogus");
                }
            "#,
        )
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [lib]
                crate-type = ["cdylib"]

                [dependencies]
                bar = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build -v")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[COMPILING] bar v1.0.0
[RUNNING] `rustc --crate-name build_script_build [..]
[RUNNING] `[ROOT]/foo/target/debug/build/bar-[HASH]/build-script-build`
[RUNNING] `rustc --crate-name bar [..]
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo --edition=2015 src/lib.rs [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .with_stderr_does_not_contain("--bogus")
        .run();
}

#[cargo_test]
fn link_arg_with_doctest() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
                //! ```
                //! let x = 5;
                //! assert_eq!(x, 5);
                //! ```
            "#,
        )
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo::rustc-link-arg=--this-is-a-bogus-flag");
                }
            "#,
        )
        .build();

    p.cargo("test --doc -v")
        .without_status()
        .with_stderr_data(
            "\
...
[RUNNING] `rustdoc [..]--crate-name foo [..]-C link-arg=--this-is-a-bogus-flag[..]
...",
        )
        .run();
}

#[cargo_test]
fn build_script_extra_link_arg_tests() {
    let p = project()
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file("src/lib.rs", "")
        .file("tests/test_foo.rs", "")
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo::rustc-link-arg-tests=--this-is-a-bogus-flag");
                }
            "#,
        )
        .build();

    p.cargo("test -v")
        .without_status()
        .with_stderr_data(
            "\
...
[RUNNING] `rustc --crate-name test_foo [..]-C link-arg=--this-is-a-bogus-flag[..]
...",
        )
        .run();
}

#[cargo_test]
fn build_script_extra_link_arg_benches() {
    let p = project()
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file("src/lib.rs", "")
        .file("benches/bench_foo.rs", "")
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo::rustc-link-arg-benches=--this-is-a-bogus-flag");
                }
            "#,
        )
        .build();

    p.cargo("bench -v")
        .without_status()
        .with_stderr_data(
            "\
...
[RUNNING] `rustc --crate-name bench_foo [..]-C link-arg=--this-is-a-bogus-flag[..]
...",
        )
        .run();
}

#[cargo_test]
fn build_script_extra_link_arg_examples() {
    let p = project()
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file("src/lib.rs", "")
        .file("examples/example_foo.rs", "fn main() {}")
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo::rustc-link-arg-examples=--this-is-a-bogus-flag");
                }
            "#,
        )
        .build();

    p.cargo("build -v --examples")
        .without_status()
        .with_stderr_data(
            "\
...
[RUNNING] `rustc --crate-name example_foo [..]-C link-arg=--this-is-a-bogus-flag[..]
...",
        )
        .run();
}

#[cargo_test]
fn cdylib_both_forms() {
    // Cargo accepts two different forms for the cdylib link instruction,
    // which have the same meaning.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [lib]
                crate-type = ["cdylib"]
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo::rustc-cdylib-link-arg=--bogus-flag-one");
                    println!("cargo::rustc-link-arg-cdylib=--bogus-flag-two");
                }
            "#,
        )
        .build();
    p.cargo("build -v")
        .without_status()
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[RUNNING] `rustc --crate-name build_script_build [..]
[RUNNING] `[ROOT]/foo/target/debug/build/foo-[HASH]/build-script-build`
[RUNNING] `rustc --crate-name foo [..]--crate-type cdylib [..]-C link-arg=--bogus-flag-one -C link-arg=--bogus-flag-two[..]
...
"#]])
        .run();
}

// https://github.com/rust-lang/cargo/issues/12663
#[cargo_test]
fn cdylib_extra_link_args_should_not_apply_to_unit_tests() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.5.0"
                edition = "2015"

                [lib]
                crate-type = ["lib", "cdylib"]
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                #[test]
                fn noop() {}
            "#,
        )
        .file(
            "build.rs",
            r#"
                fn main() {
                    // This would fail if cargo passed `-lhack` to building the test because `hack` doesn't exist.
                    println!("cargo::rustc-link-arg-cdylib=-lhack");
                }
            "#,
        )
        .build();

    p.cargo("test --lib").run();
}
