use cargotest::support::{basic_lib_manifest, execs, project};
use cargotest::ChannelChanger;
use hamcrest::assert_that;

#[test]
fn profile_override_gated() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [profile.dev.build-override]
            opt-level = 3
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("build").masquerade_as_nightly_cargo(),
        execs().with_status(101).with_stderr(
            "\
error: failed to parse manifest at `[..]`

Caused by:
  feature `profile-overrides` is required

consider adding `cargo-features = [\"profile-overrides\"]` to the manifest
",
        ),
    );

    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [profile.dev.overrides."*"]
            opt-level = 3
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("build").masquerade_as_nightly_cargo(),
        execs().with_status(101).with_stderr(
            "\
error: failed to parse manifest at `[..]`

Caused by:
  feature `profile-overrides` is required

consider adding `cargo-features = [\"profile-overrides\"]` to the manifest
",
        ),
    );
}

#[test]
fn profile_override_basic() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["profile-overrides"]

            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = {path = "bar"}

            [profile.dev]
            opt-level = 1

            [profile.dev.overrides.bar]
            opt-level = 3
        "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_lib_manifest("bar"))
        .file("bar/src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("build -v").masquerade_as_nightly_cargo(),
        execs().with_status(0).with_stderr(
            "[COMPILING] bar [..]
[RUNNING] `rustc --crate-name bar [..] -C opt-level=3 [..]`
[COMPILING] foo [..]
[RUNNING] `rustc --crate-name foo [..] -C opt-level=1 [..]`
[FINISHED] dev [optimized + debuginfo] target(s) in [..]",
        ),
    );
}

#[test]
fn profile_override_bad_name() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["profile-overrides"]

            [package]
            name = "foo"
            version = "0.0.1"

            [dependencies]
            bar = {path = "bar"}

            [profile.dev.overrides.bart]
            opt-level = 3

            [profile.dev.overrides.no-suggestion]
            opt-level = 3
        "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_lib_manifest("bar"))
        .file("bar/src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("build").masquerade_as_nightly_cargo(),
        execs().with_status(0).with_stderr_contains(
            "\
[WARNING] package `bart` for profile override not found

Did you mean `bar`?
[WARNING] package `no-suggestion` for profile override not found
[COMPILING] [..]
",
        ),
    );
}

#[test]
fn profile_override_dev_release_only() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["profile-overrides"]

            [package]
            name = "foo"
            version = "0.0.1"

            [dependencies]
            bar = {path = "bar"}

            [profile.test.overrides.bar]
            opt-level = 3
        "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_lib_manifest("bar"))
        .file("bar/src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("build").masquerade_as_nightly_cargo(),
        execs().with_status(101).with_stderr_contains(
            "\
Caused by:
  Profile overrides may only be specified for `dev` or `release` profile, not `test`.
",
        ),
    );
}

#[test]
fn profile_override_bad_settings() {
    let bad_values = [
        (
            "panic = \"abort\"",
            "`panic` may not be specified in a profile override.",
        ),
        (
            "lto = true",
            "`lto` may not be specified in a profile override.",
        ),
        (
            "rpath = true",
            "`rpath` may not be specified in a profile override.",
        ),
        ("overrides = {}", "Profile overrides cannot be nested."),
    ];
    for &(ref snippet, ref expected) in bad_values.iter() {
        let p = project("foo")
            .file(
                "Cargo.toml",
                &format!(
                    r#"
                cargo-features = ["profile-overrides"]

                [package]
                name = "foo"
                version = "0.0.1"

                [dependencies]
                bar = {{path = "bar"}}

                [profile.dev.overrides.bar]
                {}
            "#,
                    snippet
                ),
            )
            .file("src/lib.rs", "")
            .file("bar/Cargo.toml", &basic_lib_manifest("bar"))
            .file("bar/src/lib.rs", "")
            .build();

        assert_that(
            p.cargo("build").masquerade_as_nightly_cargo(),
            execs()
                .with_status(101)
                .with_stderr_contains(format!("Caused by:\n  {}", expected)),
        );
    }
}
