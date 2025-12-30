//! Tests for `cargo report rebuilds`.

use crate::prelude::*;
use crate::utils::cargo_process;

use cargo_test_support::basic_manifest;
use cargo_test_support::paths;
use cargo_test_support::project;
use cargo_test_support::str;

#[cargo_test]
fn gated_stable_channel() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/lib.rs", "")
        .build();

    p.cargo("report rebuilds")
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] unrecognized subcommand 'rebuilds'

Usage: cargo report [OPTIONS] <COMMAND>

For more information, try '--help'.

"#]])
        .run();
}

#[cargo_test]
fn gated_unstable_options() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/lib.rs", "")
        .build();

    p.cargo("report rebuilds")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] unrecognized subcommand 'rebuilds'

Usage: cargo report [OPTIONS] <COMMAND>

For more information, try '--help'.

"#]])
        .run();
}

#[cargo_test]
fn no_log() {
    cargo_process("report rebuilds -Zbuild-analysis")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] unrecognized subcommand 'rebuilds'

Usage: cargo report [OPTIONS] <COMMAND>

For more information, try '--help'.

"#]])
        .run();
}

#[cargo_test]
fn no_log_for_the_current_workspace() {
    let foo = project()
        .at("foo")
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/lib.rs", "")
        .build();

    foo.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .run();

    // one log file got generated.
    let _ = paths::log_file(0);

    let bar = project()
        .at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.0.0"))
        .file("src/lib.rs", "")
        .build();

    bar.cargo("report rebuilds -Zbuild-analysis")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] unrecognized subcommand 'rebuilds'

Usage: cargo report [OPTIONS] <COMMAND>

For more information, try '--help'.

"#]])
        .run();
}

#[cargo_test]
fn no_rebuild_data() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .run();

    p.cargo("report rebuilds -Zbuild-analysis")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] unrecognized subcommand 'rebuilds'

Usage: cargo report [OPTIONS] <COMMAND>

For more information, try '--help'.

"#]])
        .run();
}

#[cargo_test]
fn basic_rebuild() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .run();

    p.change_file("src/lib.rs", "// touched");

    p.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .run();

    p.cargo("report rebuilds -Zbuild-analysis")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] unrecognized subcommand 'rebuilds'

Usage: cargo report [OPTIONS] <COMMAND>

For more information, try '--help'.

"#]])
        .run();
}

#[cargo_test]
fn all_fresh() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .run();

    p.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .run();

    p.cargo("report rebuilds -Zbuild-analysis")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] unrecognized subcommand 'rebuilds'

Usage: cargo report [OPTIONS] <COMMAND>

For more information, try '--help'.

"#]])
        .run();
}

#[cargo_test]
fn with_dependencies() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            edition = "2021"

            [dependencies]
            dep = { path = "dep" }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "dep/Cargo.toml",
            r#"
            [package]
            name = "dep"
            edition = "2021"

            [dependencies]
            nested = { path = "../nested" }
            "#,
        )
        .file("dep/src/lib.rs", "")
        .file(
            "nested/Cargo.toml",
            r#"
            [package]
            name = "nested"
            edition = "2021"

            [dependencies]
            deep = { path = "../deep" }
            "#,
        )
        .file("nested/src/lib.rs", "")
        .file(
            "deep/Cargo.toml",
            r#"
            [package]
            name = "deep"
            edition = "2021"

            [dependencies]
            deeper = { path = "../deeper" }
            "#,
        )
        .file("deep/src/lib.rs", "")
        .file("deeper/Cargo.toml", &basic_manifest("deeper", "0.0.0"))
        .file("deeper/src/lib.rs", "")
        .build();

    p.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .run();

    p.change_file("deeper/src/lib.rs", "// touched");

    p.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .run();

    p.cargo("report rebuilds -Zbuild-analysis")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] unrecognized subcommand 'rebuilds'

Usage: cargo report [OPTIONS] <COMMAND>

For more information, try '--help'.

"#]])
        .run();

    p.cargo("report rebuilds -Zbuild-analysis -vv")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] unrecognized subcommand 'rebuilds'

Usage: cargo report [OPTIONS] <COMMAND>

For more information, try '--help'.

"#]])
        .run();
}

#[cargo_test]
fn multiple_root_causes() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["pkg1", "pkg2", "pkg3", "pkg4", "pkg5", "pkg6"]
            resolver = "2"
            "#,
        )
        .file("pkg1/Cargo.toml", &basic_manifest("pkg1", "0.0.0"))
        .file("pkg1/src/lib.rs", "")
        .file("pkg2/Cargo.toml", &basic_manifest("pkg2", "0.0.0"))
        .file("pkg2/src/lib.rs", "")
        .file("pkg3/Cargo.toml", &basic_manifest("pkg3", "0.0.0"))
        .file("pkg3/src/lib.rs", "")
        .file("pkg4/Cargo.toml", &basic_manifest("pkg4", "0.0.0"))
        .file(
            "pkg4/src/lib.rs",
            "fn f() { let _ = option_env!(\"__CARGO_TEST_MY_FOO\");}",
        )
        .file("pkg5/Cargo.toml", &basic_manifest("pkg5", "0.0.0"))
        .file("pkg5/src/lib.rs", "")
        .file("pkg6/Cargo.toml", &basic_manifest("pkg6", "0.0.0"))
        .file("pkg6/src/lib.rs", "")
        .build();

    p.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .run();

    p.change_file(
        "pkg1/Cargo.toml",
        r#"
        [package]
        name = "pkg1"
        edition = "2021"

        [features]
        feat = []
        "#,
    );
    p.change_file("pkg2/src/lib.rs", "// touched");
    p.change_file(
        "pkg3/Cargo.toml",
        r#"
        [package]
        name = "pkg3"
        edition = "2024"
        "#,
    );
    p.change_file("pkg5/src/lib.rs", "// touched");
    p.change_file("pkg6/src/lib.rs", "// touched");

    p.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .env("__CARGO_TEST_MY_FOO", "1")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .run();

    p.cargo("report rebuilds -Zbuild-analysis")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] unrecognized subcommand 'rebuilds'

Usage: cargo report [OPTIONS] <COMMAND>

For more information, try '--help'.

"#]])
        .run();

    p.cargo("report rebuilds -Zbuild-analysis --verbose")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] unrecognized subcommand 'rebuilds'

Usage: cargo report [OPTIONS] <COMMAND>

For more information, try '--help'.

"#]])
        .run();
}

#[cargo_test]
fn shared_dep_cascading() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["foo", "bar"]
            resolver = "2"

            [workspace.dependencies]
            common = { path = "common" }
            "#,
        )
        .file(
            "foo/Cargo.toml",
            r#"
            [package]
            name = "foo"
            edition = "2021"

            [dependencies]
            common = { workspace = true }
            "#,
        )
        .file("foo/src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
            [package]
            name = "bar"
            edition = "2021"

            [dependencies]
            common = { workspace = true }
            "#,
        )
        .file("bar/src/lib.rs", "")
        .file("common/Cargo.toml", &basic_manifest("common", "0.0.0"))
        .file("common/src/lib.rs", "")
        .build();

    p.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .run();

    p.change_file("common/src/lib.rs", "// touched");

    p.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .run();

    p.cargo("report rebuilds -Zbuild-analysis")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] unrecognized subcommand 'rebuilds'

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

    p.change_file("src/lib.rs", "// touched");
    p.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .run();

    cargo_process("report rebuilds -Zbuild-analysis")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] unrecognized subcommand 'rebuilds'

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

    bar.change_file("src/lib.rs", "// touched");
    bar.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .run();

    foo.cargo("report rebuilds --manifest-path ../bar/Cargo.toml -Zbuild-analysis")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] unrecognized subcommand 'rebuilds'

Usage: cargo report [OPTIONS] <COMMAND>

For more information, try '--help'.

"#]])
        .run();
}
