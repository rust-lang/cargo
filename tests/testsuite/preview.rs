//! Tests for preview features and options.

use cargo_test_support::registry::Package;
use cargo_test_support::{basic_manifest, project};

#[cargo_test]
fn option_config() {
    // This is out_dir::out_dir_is_a_file, but without nightly.

    let p = project()
        .file("src/main.rs", r#"fn main() { println!("Hello, World!") }"#)
        .file(
            ".cargo/config.toml",
            r#"
                [enable-preview]
                "--out-dir" = true
        "#,
        )
        .file("out", "")
        .build();

    p.cargo("build --out-dir out")
        .with_status(101)
        .with_stderr_contains("[ERROR] failed to create directory [..]")
        .run();
}

#[cargo_test]
fn option_out_dir() {
    // This is out_dir::out_dir_is_a_file, but without nightly.

    let p = project()
        .file("src/main.rs", r#"fn main() { println!("Hello, World!") }"#)
        .file("out", "")
        .build();

    p.cargo("build --enable-preview=--out-dir --out-dir out")
        .with_status(101)
        .with_stderr_contains("[ERROR] failed to create directory [..]")
        .run();
}

#[cargo_test]
fn feature_config() {
    // This is patch::from_config, but without nightly.

    Package::new("bar", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                bar = "0.1.0"
            "#,
        )
        .file(
            ".cargo/config.toml",
            r#"
                [patch.crates-io]
                bar = { path = 'bar' }

                [enable-preview]
                patch-in-config = true
            "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.1"))
        .file("bar/src/lib.rs", r#""#)
        .build();

    p.cargo("build")
        .with_stderr(
            "\
[UPDATING] `[ROOT][..]` index
[COMPILING] bar v0.1.1 ([..])
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn feature_patch_in_config() {
    // This is patch::from_config, but without nightly.

    Package::new("bar", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                bar = "0.1.0"
            "#,
        )
        .file(
            ".cargo/config.toml",
            r#"
                [patch.crates-io]
                bar = { path = 'bar' }
            "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.1"))
        .file("bar/src/lib.rs", r#""#)
        .build();

    p.cargo("build --enable-preview=patch-in-config")
        .with_stderr(
            "\
[UPDATING] `[ROOT][..]` index
[COMPILING] bar v0.1.1 ([..])
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}
