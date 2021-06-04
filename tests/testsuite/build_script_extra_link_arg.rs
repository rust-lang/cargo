//! Tests for additional link arguments.

// NOTE: Many of these tests use `without_status()` when passing bogus flags
// because MSVC link.exe just gives a warning on unknown flags (how helpful!),
// and other linkers will return an error.

use cargo_test_support::registry::Package;
use cargo_test_support::{basic_bin_manifest, basic_manifest, project};

#[cargo_test]
fn build_script_extra_link_arg_bin() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo:rustc-link-arg-bins=--this-is-a-bogus-flag");
                }
            "#,
        )
        .build();

    p.cargo("build -v")
        .masquerade_as_nightly_cargo()
        .without_status()
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name foo [..]-C link-arg=--this-is-a-bogus-flag[..]",
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
                    println!("cargo:rustc-link-arg-bins=--bogus-flag-all");
                    println!("cargo:rustc-link-arg-bin=foo=--bogus-flag-foo");
                    println!("cargo:rustc-link-arg-bin=bar=--bogus-flag-bar");
                }
            "#,
        )
        .build();

    p.cargo("build -v")
        .masquerade_as_nightly_cargo()
        .without_status()
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name foo [..]-C link-arg=--bogus-flag-all -C link-arg=--bogus-flag-foo[..]",
        )
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name bar [..]-C link-arg=--bogus-flag-all -C link-arg=--bogus-flag-bar[..]",
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
                    println!("cargo:rustc-link-arg=--this-is-a-bogus-flag");
                }
            "#,
        )
        .build();

    p.cargo("build -v")
        .masquerade_as_nightly_cargo()
        .without_status()
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name foo [..]-C link-arg=--this-is-a-bogus-flag[..]",
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
            r#"fn main() { println!("cargo:rustc-link-arg-cdylib=--bogus"); }"#,
        )
        .build();

    // TODO: Uncomment this if cdylib restriction is re-added (see
    // cdylib_link_arg_transitive below).
    //     p.cargo("check")
    //         .with_status(101)
    //         .with_stderr("\
    // [COMPILING] foo [..]
    // error: invalid instruction `cargo:rustc-link-arg-cdylib` from build script of `foo v0.0.1 ([ROOT]/foo)`
    // The package foo v0.0.1 ([ROOT]/foo) does not have a cdylib target.
    // ")
    //         .run();

    p.change_file(
        "build.rs",
        r#"fn main() { println!("cargo:rustc-link-arg-bins=--bogus"); }"#,
    );

    p.cargo("check")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr("\
[COMPILING] foo [..]
error: invalid instruction `cargo:rustc-link-arg-bins` from build script of `foo v0.0.1 ([ROOT]/foo)`
The package foo v0.0.1 ([ROOT]/foo) does not have a bin target.
")
        .run();

    p.change_file(
        "build.rs",
        r#"fn main() { println!("cargo:rustc-link-arg-bin=abc=--bogus"); }"#,
    );

    p.cargo("check")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[COMPILING] foo [..]
error: invalid instruction `cargo:rustc-link-arg-bin` from build script of `foo v0.0.1 ([ROOT]/foo)`
The package foo v0.0.1 ([ROOT]/foo) does not have a bin target with the name `abc`.
",
        )
        .run();

    p.change_file(
        "build.rs",
        r#"fn main() { println!("cargo:rustc-link-arg-bin=abc"); }"#,
    );

    p.cargo("check")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[COMPILING] foo [..]
error: invalid instruction `cargo:rustc-link-arg-bin=abc` from build script of `foo v0.0.1 ([ROOT]/foo)`
The instruction should have the form cargo:rustc-link-arg-bin=BIN=ARG
",
        )
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
                    println!("cargo:rustc-link-arg-cdylib=--bogus");
                }
            "#,
        )
        .build();
    p.cargo("build -v")
        .without_status()
        .with_stderr_contains(
            "\
[COMPILING] bar v1.0.0 [..]
[RUNNING] `rustc --crate-name build_script_build bar/build.rs [..]
[RUNNING] `[..]build-script-build[..]
warning: cargo:rustc-link-arg-cdylib was specified in the build script of bar v1.0.0 \
([ROOT]/foo/bar), but that package does not contain a cdylib target

Allowing this was an unintended change in the 1.50 release, and may become an error in \
the future. For more information, see <https://github.com/rust-lang/cargo/issues/9562>.
[RUNNING] `rustc --crate-name bar bar/src/lib.rs [..]
[COMPILING] foo v0.1.0 [..]
[RUNNING] `rustc --crate-name foo src/lib.rs [..]-C link-arg=--bogus[..]`
",
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
                    println!("cargo:rustc-link-arg=--bogus");
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

                [lib]
                crate-type = ["cdylib"]

                [dependencies]
                bar = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build -v")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[UPDATING] [..]
[DOWNLOADING] [..]
[DOWNLOADED] [..]
[COMPILING] bar v1.0.0
[RUNNING] `rustc --crate-name build_script_build [..]
[RUNNING] `[..]/build-script-build[..]
[RUNNING] `rustc --crate-name bar [..]
[COMPILING] foo v0.1.0 [..]
[RUNNING] `rustc --crate-name foo src/lib.rs [..]
[FINISHED] dev [..]
",
        )
        .with_stderr_does_not_contain("--bogus")
        .run();
}
