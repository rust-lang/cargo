//! Tests for targets with `rust-version`.

use cargo_test_support::is_nightly;
use cargo_test_support::{project, registry::Package};

#[cargo_test]
fn rust_version_satisfied() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            rust-version = "1.1.1"
            [[bin]]
            name = "foo"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build").run();
    p.cargo("build --ignore-rust-version").run();
}

#[cargo_test]
fn rust_version_bad_caret() {
    project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            rust-version = "^1.43"
            [[bin]]
            name = "foo"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build()
        .cargo("build")
        .with_status(101)
        .with_stderr(
            "error: failed to parse manifest at `[..]`\n\n\
             Caused by:\n  `rust-version` must be a value like \"1.32\"",
        )
        .run();
}

#[cargo_test]
fn rust_version_bad_pre_release() {
    project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            rust-version = "1.43-beta.1"
            [[bin]]
            name = "foo"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build()
        .cargo("build")
        .with_status(101)
        .with_stderr(
            "error: failed to parse manifest at `[..]`\n\n\
             Caused by:\n  `rust-version` must be a value like \"1.32\"",
        )
        .run();
}

#[cargo_test]
fn rust_version_bad_nonsense() {
    project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            rust-version = "foodaddle"
            [[bin]]
            name = "foo"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build()
        .cargo("build")
        .with_status(101)
        .with_stderr(
            "error: failed to parse manifest at `[..]`\n\n\
             Caused by:\n  `rust-version` must be a value like \"1.32\"",
        )
        .run();
}

#[cargo_test]
fn rust_version_too_high() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            rust-version = "1.9876.0"
            [[bin]]
            name = "foo"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "error: package `foo v0.0.1 ([..])` cannot be built because it requires \
             rustc 1.9876.0 or newer, while the currently active rustc version is [..]",
        )
        .run();
    p.cargo("build --ignore-rust-version").run();
}

#[cargo_test]
fn rust_version_dependency_fails() {
    Package::new("bar", "0.0.1")
        .rust_version("1.2345.0")
        .file("src/lib.rs", "fn other_stuff() {}")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            [dependencies]
            bar = "0.0.1"
        "#,
        )
        .file("src/main.rs", "fn main(){}")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "    Updating `[..]` index\n \
             Downloading crates ...\n  \
             Downloaded bar v0.0.1 (registry `[..]`)\n\
             error: package `bar v0.0.1` cannot be built because it requires \
             rustc 1.2345.0 or newer, while the currently active rustc version is [..]",
        )
        .run();
    p.cargo("build --ignore-rust-version").run();
}

fn check_min_rust_version(minor: u32, manifest: &str, error: &str) {
    let bad = manifest.replace("MINOR", &(minor - 1).to_string());
    let p = project()
        .file("Cargo.toml", &bad)
        .file("src/lib.rs", "")
        .build();
    p.cargo("check")
        .with_status(101)
        .with_stderr(&format!(
            "error: failed to parse manifest at `[ROOT]/foo/Cargo.toml`\n\
             \n\
             Caused by:\n  \
             rust-version `1.{}` is older than the first version required for {}\n  \
             This requires a version of at least `1.{}`.",
            minor - 1,
            error,
            minor
        ))
        .run();
    let good = manifest.replace("MINOR", &minor.to_string());
    p.change_file("Cargo.toml", &good);
    p.cargo("check").run();
}

#[cargo_test]
fn check_min_rust_version_edition() {
    check_min_rust_version(
        31,
        r#"
            [package]
            name = "foo"
            version = "0.1.0"
            rust-version = "1.MINOR"
            edition = "2018"
        "#,
        "the specified edition `2018`",
    );
}

#[cargo_test]
fn check_min_rust_version_named_profile() {
    check_min_rust_version(
        57,
        r#"
            [package]
            name = "foo"
            version = "0.1.0"
            rust-version = "1.MINOR"

            [profile.foo]
            inherits = "dev"
        "#,
        "custom named profiles (profile `foo`)",
    );
}

#[cargo_test]
fn check_min_rust_version_profile() {
    if !is_nightly() {
        // Remove when 1.59 hits stable.
        return;
    }
    check_min_rust_version(
        59,
        r#"
            [package]
            name = "foo"
            version = "0.1.0"
            rust-version = "1.MINOR"

            [profile.dev]
            strip = "debuginfo"
        "#,
        "the `strip` profile option (in profile `dev`)",
    );
}

#[cargo_test]
fn check_min_rust_version_features() {
    if !is_nightly() {
        // Remove when 1.60 hits stable.
        return;
    }
    Package::new("bar", "1.0.0").feature("feat", &[]).publish();

    check_min_rust_version(
        60,
        r#"
            [package]
            name = "foo"
            version = "0.1.0"
            rust-version = "1.MINOR"

            [dependencies]
            bar = { version="1.0", optional=true }

            [features]
            f1 = ["dep:bar"]
        "#,
        "namespaced features (feature `f1` with value `dep:bar`)",
    );

    check_min_rust_version(
        60,
        r#"
            [package]
            name = "foo"
            version = "0.1.0"
            rust-version = "1.MINOR"

            [dependencies]
            bar = { version="1.0", optional=true }

            [features]
            f1 = ["bar?/feat"]
        "#,
        "weak dependency features (feature `f1` with value `bar?/feat`)",
    );
}
