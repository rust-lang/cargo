use cargo_test_support::{basic_lib_manifest, is_nightly, paths, project};

#[cargo_test]
fn profile_config_gated() {
    let p = project()
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

    p.cargo("build -v")
        .with_stderr_contains(
            "\
[WARNING] profiles in config files require `-Z config-profile` command-line option
",
        )
        .with_stderr_contains("[..]-C debuginfo=2[..]")
        .run();
}

#[cargo_test]
fn profile_config_validate_warnings() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["profile-overrides"]

            [package]
            name = "foo"
            version = "0.0.1"
            "#,
        )
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

    p.cargo("build -Z config-profile")
        .masquerade_as_nightly_cargo()
        .with_stderr_unordered(
            "\
[WARNING] unused key `profile.asdf` in config file `[..].cargo/config`
[WARNING] unused key `profile.test` in config file `[..].cargo/config`
[WARNING] unused key `profile.dev.bad-key` in config file `[..].cargo/config`
[WARNING] unused key `profile.dev.overrides.bar.bad-key-bar` in config file `[..].cargo/config`
[WARNING] unused key `profile.dev.build-override.bad-key-bo` in config file `[..].cargo/config`
[COMPILING] foo [..]
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn profile_config_error_paths() {
    let p = project()
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

    p.cargo("build -Z config-profile")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[CWD]/Cargo.toml`

Caused by:
  error in [..].cargo/config: `profile.dev.rpath` expected true/false, but found a string
",
        )
        .run();
}

#[cargo_test]
fn profile_config_validate_errors() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["profile-overrides"]

            [package]
            name = "foo"
            version = "0.0.1"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            r#"
            [profile.dev.overrides.foo]
            panic = "abort"
        "#,
        )
        .build();

    p.cargo("build -Z config-profile")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[ERROR] config profile `profile.dev` is not valid

Caused by:
  `panic` may not be specified in a profile override.
",
        )
        .run();
}

#[cargo_test]
fn profile_config_syntax_errors() {
    let p = project()
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

    p.cargo("build -Z config-profile")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at [..]

Caused by:
  error in [..].cargo/config: `profile.dev.codegen-units` expected an integer, but found a string
",
        )
        .run();
}

#[cargo_test]
fn profile_config_override_spec_multiple() {
    let p = project()
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
        .file(
            "bar/Cargo.toml",
            r#"
            cargo-features = ["profile-overrides"]

            [package]
            name = "bar"
            version = "0.5.0"
        "#,
        )
        .file("bar/src/lib.rs", "")
        .build();

    // Unfortunately this doesn't tell you which file, hopefully it's not too
    // much of a problem.
    p.cargo("build -v -Z config-profile")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[ERROR] multiple profile overrides in profile `dev` match package `bar v0.5.0 ([..])`
found profile override specs: bar, bar:0.5.0",
        )
        .run();
}

#[cargo_test]
fn profile_config_all_options() {
    if !is_nightly() {
        // May be removed once 1.34 is stable (added support for incremental-LTO).
        return;
    }

    // Ensure all profile options are supported.
    let p = project()
        .file("src/main.rs", "fn main() {}")
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

    p.cargo("build --release -v -Z config-profile")
        .masquerade_as_nightly_cargo()
        .env_remove("CARGO_INCREMENTAL")
        .with_stderr(
            "\
[COMPILING] foo [..]
[RUNNING] `rustc --crate-name foo [..] \
            -C opt-level=1 \
            -C panic=abort \
            -C lto \
            -C codegen-units=2 \
            -C debuginfo=2 \
            -C debug-assertions=on \
            -C overflow-checks=off [..]\
            -C rpath [..]\
            -C incremental=[..]
[FINISHED] release [optimized + debuginfo] [..]
",
        )
        .run();
}

#[cargo_test]
fn profile_config_override_precedence() {
    // Config values take precedence over manifest values.
    let p = project()
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
        .file(
            "bar/Cargo.toml",
            r#"
            cargo-features = ["profile-overrides"]

            [package]
            name = "bar"
            version = "0.0.1"
            "#,
        )
        .file("bar/src/lib.rs", "")
        .file(
            ".cargo/config",
            r#"
            [profile.dev.overrides.bar]
            opt-level = 2
        "#,
        )
        .build();

    p.cargo("build -v -Z config-profile")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[COMPILING] bar [..]
[RUNNING] `rustc --crate-name bar [..] -C opt-level=2 -C codegen-units=2 [..]
[COMPILING] foo [..]
[RUNNING] `rustc --crate-name foo [..]-C codegen-units=2 [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]",
        )
        .run();
}

#[cargo_test]
fn profile_config_no_warn_unknown_override() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["profile-overrides"]

            [package]
            name = "foo"
            version = "0.0.1"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            r#"
            [profile.dev.overrides.bar]
            codegen-units = 4
        "#,
        )
        .build();

    p.cargo("build -Z config-profile")
        .masquerade_as_nightly_cargo()
        .with_stderr_does_not_contain("[..]warning[..]")
        .run();
}

#[cargo_test]
fn profile_config_mixed_types() {
    let p = project()
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

    p.cargo("build -v -Z config-profile")
        .masquerade_as_nightly_cargo()
        .with_stderr_contains("[..]-C opt-level=3 [..]")
        .run();
}
