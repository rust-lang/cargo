use cargotest::support::{basic_lib_manifest, execs, paths, project};
use cargotest::ChannelChanger;
use hamcrest::assert_that;

#[test]
fn profile_config_gated() {
    let p = project("foo")
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            r#"
            [profile.dev]
            debug = 1
        "#,
        )
        .build();

    assert_that(
        p.cargo("build -v"),
        execs().with_status(0).with_stderr_contains(
            "\
[WARNING] profile in config `[..]foo[/].cargo[/]config` requires `-Z config-profile` command-line option
",
        ).with_stderr_contains("[..]-C debuginfo=2[..]"),
    );
}

#[test]
fn profile_config_validate_warnings() {
    let p = project("foo")
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            r#"
            [profile.test]
            opt-level = 3

            [profile.asdf]
            opt-level = 3

            [profile.dev]
            bad-key = true

            [profile.dev.build-override]
            bad-key-bo = true

            [profile.dev.overrides.bar]
            bad-key-bar = true
        "#,
        )
        .build();

    assert_that(
        p.cargo("build -v -Z config-profile")
            .masquerade_as_nightly_cargo(),
        execs()
            .with_status(0)
            .with_stderr_contains(
                "\
[WARNING] profile `test` in config `[..]foo[/].cargo[/]config` is not supported
",
            )
            .with_stderr_contains(
                "\
[WARNING] profile `asdf` in config `[..]foo[/].cargo[/]config` is not supported
",
            )
            .with_stderr_contains(
                "\
[WARNING] unused profile key `bad-key` in config `[..]foo[/].cargo[/]config`
[WARNING] unused profile key `bad-key-bo` in config `[..]foo[/].cargo[/]config`
[WARNING] unused profile key `bad-key-bar` in config `[..]foo[/].cargo[/]config`
",
            ),
    );
}

#[test]
fn profile_config_error_paths() {
    // Due to how it's implemented, we are uncertain where a merged error
    // comes from.
    let p = project("foo")
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            r#"
            [profile.dev]
            opt-level = 3
        "#,
        )
        .file(
            paths::home().join(".cargo/config"),
            r#"
            [profile.dev]
            rpath = "foo"
            "#,
        )
        .build();

    assert_that(
        p.cargo("build -Z config-profile")
            .masquerade_as_nightly_cargo(),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]foo[/]Cargo.toml`

Caused by:
  error in config profile, possible locations: [..]foo[/].cargo[/]config, [..]home[/].cargo[/]config

Caused by:
  invalid type: string \"foo\", expected a boolean for key `rpath`
",
        ),
    );
}

#[test]
fn profile_config_validate_errors() {
    let p = project("foo")
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            r#"
            [profile.dev.overrides.foo]
            panic = "abort"
        "#,
        )
        .build();

    assert_that(
        p.cargo("build -Z config-profile")
            .masquerade_as_nightly_cargo(),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]foo[/]Cargo.toml`

Caused by:
  error in config profile `[..]foo[/].cargo[/]config`

Caused by:
  `panic` may not be specified in a profile override.
",
        ),
    );
}

#[test]
fn profile_config_syntax_errors() {
    let p = project("foo")
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            r#"
            [profile.dev]
            codegen-units = "foo"
        "#,
        )
        .build();

    assert_that(
        p.cargo("build -Z config-profile")
            .masquerade_as_nightly_cargo(),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse manifest at [..]

Caused by:
  error in config profile `[..]foo[/].cargo[/]config`

Caused by:
  invalid type: string \"foo\", expected u32 for key `codegen-units`
",
        ),
    );
}

#[test]
fn profile_config_override_spec_multiple() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["profile-overrides"]

            [package]
            name = "foo"
            version = "0.0.1"

            [dependencies]
            bar = { path = "bar" }
            "#,
        )
        .file(
            ".cargo/config",
            r#"
            [profile.dev.overrides.bar]
            opt-level = 3

            [profile.dev.overrides."bar:0.5.0"]
            opt-level = 3
        "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_lib_manifest("bar"))
        .file("bar/src/lib.rs", "")
        .build();

    // Unfortunately this doesn't tell you which file, hopefully it's not too
    // much of a problem.
    assert_that(
        p.cargo("build -v -Z config-profile")
            .masquerade_as_nightly_cargo(),
        execs().with_status(101).with_stderr(
            "\
[ERROR] multiple profile overrides in profile `dev` match package `bar v0.5.0 ([..])`
found profile override specs: bar, bar:0.5.0",
        ),
    );
}

#[test]
fn profile_config_all_options() {
    // Ensure all profile options are supported.
    let p = project("foo")
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            r#"
        [profile.release]
        opt-level = 1
        debug = true
        debug-assertions = true
        overflow-checks = false
        rpath = true
        lto = true
        codegen-units = 2
        panic = "abort"
        incremental = true
        "#,
        )
        .build();

    assert_that(
        p.cargo("build --release -v -Z config-profile")
            .masquerade_as_nightly_cargo(),
        execs().with_status(0).with_stderr(
            "\
[COMPILING] foo [..]
[RUNNING] `rustc --crate-name foo [..] \
            -C opt-level=1 \
            -C panic=abort \
            -C codegen-units=2 \
            -C debuginfo=2 \
            -C debug-assertions=on \
            -C overflow-checks=off [..]\
            -C rpath [..]
[FINISHED] release [optimized + debuginfo] [..]
",
        ),
    );
}

#[test]
fn profile_config_override_precedence() {
    // Config values take precedence over manifest values.
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

            [profile.dev]
            codegen-units = 2

            [profile.dev.overrides.bar]
            opt-level = 3
        "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_lib_manifest("bar"))
        .file("bar/src/lib.rs", "")
        .file(
            ".cargo/config",
            r#"
            [profile.dev.overrides.bar]
            opt-level = 2
        "#,
        )
        .build();

    assert_that(
        p.cargo("build -v -Z config-profile")
            .masquerade_as_nightly_cargo(),
        execs().with_status(0).with_stderr(
            "\
[COMPILING] bar [..]
[RUNNING] `rustc --crate-name bar [..] -C opt-level=2 -C codegen-units=2 [..]
[COMPILING] foo [..]
[RUNNING] `rustc --crate-name foo [..]-C codegen-units=2 [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]",
        ),
    );
}

#[test]
fn profile_config_no_warn_unknown_override() {
    let p = project("foo")
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            r#"
            [profile.dev.overrides.bar]
            codegen-units = 4
        "#,
        )
        .build();

    assert_that(
        p.cargo("build -Z config-profile")
            .masquerade_as_nightly_cargo(),
        execs()
            .with_status(0)
            .with_stderr_does_not_contain("[..]warning[..]"),
    );
}

#[test]
fn profile_config_mixed_types() {
    let p = project("foo")
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            r#"
            [profile.dev]
            opt-level = 3
        "#,
        )
        .file(
            paths::home().join(".cargo/config"),
            r#"
            [profile.dev]
            opt-level = 's'
            "#,
        )
        .build();

    assert_that(
        p.cargo("build -v -Z config-profile")
            .masquerade_as_nightly_cargo(),
        execs()
            .with_status(0)
            .with_stderr_contains("[..]-C opt-level=3 [..]"),
    );
}
