//! Tests for edition setting.

use cargo::core::Edition;
use cargo_test_support::{basic_lib_manifest, project};

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
        .with_stderr(&format!(
            "\
[ERROR] failed to parse manifest at `[..]/foo/Cargo.toml`

Caused by:
  feature `edition{next}` is required

  The package requires the Cargo feature called `edition{next}`, \
  but that feature is not stabilized in this version of Cargo (1.[..]).
  Consider trying a newer version of Cargo (this may require the nightly release).
  See https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#edition-{next} \
  for more information about the status of this feature.
",
            next = next
        ))
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
        .with_stderr(
            "\
[CHECKING] foo [..]
[FINISHED] [..]
",
        )
        .run();
}
