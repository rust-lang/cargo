//! Tests for -Zextra-link-arg.

use cargo_test_support::{basic_bin_manifest, project};

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

    p.cargo("build -Zextra-link-arg -v")
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

    p.cargo("build -Zextra-link-arg -v")
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

    p.cargo("build -Zextra-link-arg -v")
        .masquerade_as_nightly_cargo()
        .without_status()
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name foo [..]-C link-arg=--this-is-a-bogus-flag[..]",
        )
        .run();
}

#[cargo_test]
fn build_script_extra_link_arg_warn_without_flag() {
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
        .with_status(0)
        .with_stderr_contains("warning: cargo:rustc-link-arg requires -Zextra-link-arg flag")
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

    p.cargo("check")
        .with_status(101)
        .with_stderr("\
[COMPILING] foo [..]
error: invalid instruction `cargo:rustc-link-arg-cdylib` from build script of `foo v0.0.1 ([ROOT]/foo)`
The package foo v0.0.1 ([ROOT]/foo) does not have a cdylib target.
")
        .run();

    p.change_file(
        "build.rs",
        r#"fn main() { println!("cargo:rustc-link-arg-bins=--bogus"); }"#,
    );

    p.cargo("check -Zextra-link-arg")
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

    p.cargo("check -Zextra-link-arg")
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

    p.cargo("check -Zextra-link-arg")
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
