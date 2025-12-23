//! Tests for `cargo report sessions`.

mod help;

use crate::prelude::*;
use crate::utils::cargo_process;

use cargo_test_support::basic_manifest;
use cargo_test_support::project;
use cargo_test_support::str;

#[cargo_test]
fn gated_stable_channel() {
    cargo_process("report sessions")
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] unrecognized subcommand 'sessions'

Usage: cargo report [OPTIONS] <COMMAND>

For more information, try '--help'.

"#]])
        .run();
}

#[cargo_test]
fn gated_unstable_options() {
    cargo_process("report sessions")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] unrecognized subcommand 'sessions'

Usage: cargo report [OPTIONS] <COMMAND>

For more information, try '--help'.

"#]])
        .run();
}

#[cargo_test]
fn no_logs() {
    cargo_process("report sessions -Zbuild-analysis")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] unrecognized subcommand 'sessions'

Usage: cargo report [OPTIONS] <COMMAND>

For more information, try '--help'.

"#]])
        .run();
}

#[cargo_test]
fn no_logs_in_workspace() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/lib.rs", "")
        .build();

    p.cargo("report sessions -Zbuild-analysis")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] unrecognized subcommand 'sessions'

Usage: cargo report [OPTIONS] <COMMAND>

For more information, try '--help'.

"#]])
        .run();
}

#[cargo_test]
fn in_workspace() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .run();

    p.cargo("report sessions -Zbuild-analysis")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] unrecognized subcommand 'sessions'

Usage: cargo report [OPTIONS] <COMMAND>

For more information, try '--help'.

"#]])
        .run();
}

#[cargo_test]
fn outside_workspace() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .run();

    // cd to outside the workspace, should show all sessions
    cargo_process("report sessions -Zbuild-analysis")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] unrecognized subcommand 'sessions'

Usage: cargo report [OPTIONS] <COMMAND>

For more information, try '--help'.

"#]])
        .run();
}

#[cargo_test]
fn with_limit_1_and_extra_logs() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/lib.rs", "")
        .build();

    // Generate 3 build sessions
    for i in 0..3 {
        p.change_file("src/lib.rs", &format!("pub fn foo{i}() {{}}"));
        p.cargo("check -Zbuild-analysis")
            .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
            .masquerade_as_nightly_cargo(&["build-analysis"])
            .run();
    }

    p.cargo("report sessions --limit 1 -Zbuild-analysis")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] unrecognized subcommand 'sessions'

Usage: cargo report [OPTIONS] <COMMAND>

For more information, try '--help'.

"#]])
        .run();
}

#[cargo_test]
fn with_limit_5_but_not_enough_logs() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/lib.rs", "")
        .build();

    // Generate 2 build sessions
    for i in 0..2 {
        p.change_file("src/lib.rs", &format!("pub fn foo{i}() {{}}"));
        p.cargo("check -Zbuild-analysis")
            .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
            .masquerade_as_nightly_cargo(&["build-analysis"])
            .run();
    }

    p.cargo("report sessions --limit 5 -Zbuild-analysis")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] unrecognized subcommand 'sessions'

Usage: cargo report [OPTIONS] <COMMAND>

For more information, try '--help'.

"#]])
        .run();
}

#[cargo_test]
fn existing_logs_from_other_workspaces() {
    let foo = project()
        .at("foo")
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/lib.rs", "")
        .build();

    foo.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .run();

    let bar = project()
        .at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.0.0"))
        .file("src/lib.rs", "")
        .build();

    bar.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .run();

    // In foo workspace, should only show foo sessions by default
    foo.cargo("report sessions -Zbuild-analysis")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] unrecognized subcommand 'sessions'

Usage: cargo report [OPTIONS] <COMMAND>

For more information, try '--help'.

"#]])
        .run();
}

#[cargo_test]
fn with_manifest_path() {
    let foo = project()
        .at("foo")
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/lib.rs", "")
        .build();

    foo.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .run();

    let bar = project()
        .at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.0.0"))
        .file("src/lib.rs", "")
        .build();

    bar.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .run();

    foo.cargo("report sessions --manifest-path ../bar/Cargo.toml -Zbuild-analysis")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] unrecognized subcommand 'sessions'

Usage: cargo report [OPTIONS] <COMMAND>

For more information, try '--help'.

"#]])
        .run();
}
