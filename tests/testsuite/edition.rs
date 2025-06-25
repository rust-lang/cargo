//! Tests for edition setting.

use crate::prelude::*;
use cargo::core::Edition;
use cargo_test_support::{basic_lib_manifest, project, str};

#[cargo_test]
fn edition_works_for_build_script() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = 'foo'
                version = '0.1.0'
                edition = '2018'

                [build-dependencies]
                a = { path = 'a' }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                fn main() {
                    a::foo();
                }
            "#,
        )
        .file("a/Cargo.toml", &basic_lib_manifest("a"))
        .file("a/src/lib.rs", "pub fn foo() {}")
        .build();

    p.cargo("check -v").run();
}

#[cargo_test]
fn edition_unstable_gated() {
    // During the period where a new edition is coming up, but not yet stable,
    // this test will verify that it cannot be used on stable. If there is no
    // next edition, it does nothing.
    let next = match Edition::LATEST_UNSTABLE {
        Some(next) => next,
        None => {
            eprintln!("Next edition is currently not available, skipping test.");
            return;
        }
    };
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "{}"
            "#,
                next
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(format!(
            "\
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  feature `edition{next}` is required

  The package requires the Cargo feature called `edition{next}`, but that feature is not stabilized in this version of Cargo (1.[..]).
  Consider trying a newer version of Cargo (this may require the nightly release).
  See https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#edition-{next} for more information about the status of this feature.
"))
        .run();
}

#[cargo_test(nightly, reason = "fundamentally always nightly")]
fn edition_unstable() {
    // During the period where a new edition is coming up, but not yet stable,
    // this test will verify that it can be used with `cargo-features`. If
    // there is no next edition, it does nothing.
    let next = match Edition::LATEST_UNSTABLE {
        Some(next) => next,
        None => {
            eprintln!("Next edition is currently not available, skipping test.");
            return;
        }
    };
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                cargo-features = ["edition{next}"]

                [package]
                name = "foo"
                version = "0.1.0"
                edition = "{next}"
            "#,
                next = next
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["always_nightly"])
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn unset_edition_with_unset_rust_version() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = 'foo'
                version = '0.1.0'
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -v")
        .with_stderr_data(str![[r#"
[WARNING] no edition set: defaulting to the 2015 edition while the latest is [..]
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[RUNNING] `rustc [..] --edition=2015 [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn unset_edition_works_with_no_newer_compatible_edition() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = 'foo'
                version = '0.1.0'
                rust-version = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -v")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[RUNNING] `rustc [..] --edition=2015 [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn unset_edition_works_on_old_msrv() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = 'foo'
                version = '0.1.0'
                rust-version = "1.50"  # contains 2018 edition
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -v")
        .with_stderr_data(str![[r#"
[WARNING] no edition set: defaulting to the 2015 edition while 2018 is compatible with `rust-version`
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[RUNNING] `rustc [..] --edition=2015 [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn future_edition_is_gated() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                edition = "future"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  feature `unstable-editions` is required

  The package requires the Cargo feature called `unstable-editions`, but that feature is not stabilized in this version of Cargo ([..]).
  Consider trying a newer version of Cargo (this may require the nightly release).
  See https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#unstable-editions for more information about the status of this feature.

"#]])
        .run();

    // Repeat on nightly.
    p.cargo("check")
        .masquerade_as_nightly_cargo(&["unstable-editions"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  feature `unstable-editions` is required

  The package requires the Cargo feature called `unstable-editions`, but that feature is not stabilized in this version of Cargo ([..]).
  Consider adding `cargo-features = ["unstable-editions"]` to the top of Cargo.toml (above the [package] table) to tell Cargo you are opting in to use this unstable feature.
  See https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#unstable-editions for more information about the status of this feature.

"#]])
        .run();
}

#[cargo_test(nightly, reason = "future edition is always unstable")]
fn future_edition_works() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["unstable-editions"]

                [package]
                name = "foo"
                edition = "future"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["unstable-editions"])
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
