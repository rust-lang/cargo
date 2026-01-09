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
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] the `cargo report rebuilds` command is unstable, and only available on the nightly channel of Cargo, but this is the `stable` channel
See https://doc.rust-lang.org/book/appendix-07-nightly-rust.html for more information about Rust release channels.
See https://github.com/rust-lang/cargo/issues/15844 for more information about the `cargo report rebuilds` command.

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
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] the `cargo report rebuilds` command is unstable, pass `-Z build-analysis` to enable it
See https://github.com/rust-lang/cargo/issues/15844 for more information about the `cargo report rebuilds` command.

"#]])
        .run();
}

#[cargo_test]
fn no_log() {
    cargo_process("report rebuilds -Zbuild-analysis")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no sessions found
  |
  = [NOTE] run command with `-Z build-analysis` to generate log files

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
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no sessions found for workspace at `[ROOT]/bar`
  |
  = [NOTE] run command with `-Z build-analysis` to generate log files

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
        .with_stderr_data(str![[r#"
Session: [..]
Status: 0 units rebuilt, 0 cached, 1 new


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
        .with_stderr_data(str![[r#"
Session: [..]
Status: 1 unit rebuilt, 0 cached, 0 new

Rebuild impact:
  root rebuilds: 1 unit
  cascading:     0 units

Root rebuilds:
  0. foo@0.0.0 (check): file modified: src/lib.rs
     impact: no cascading rebuilds

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

    // Second build without changes
    p.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .run();

    p.cargo("report rebuilds -Zbuild-analysis")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_stderr_data(str![[r#"
Session: [..]
Status: 0 units rebuilt, 1 cached, 0 new


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
        .with_stderr_data(str![[r#"
Session: [..]
Status: 5 units rebuilt, 0 cached, 0 new

Rebuild impact:
  root rebuilds: 1 unit
  cascading:     4 units

Root rebuilds:
  0. deeper@0.0.0 (check): file modified: deeper/src/lib.rs
     impact: 4 dependent units rebuilt

[NOTE] pass `-vv` to show all affected rebuilt unit lists

"#]])
        .run();

    p.cargo("report rebuilds -Zbuild-analysis -vv")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_stderr_data(str![[r#"
Session: [..]
Status: 5 units rebuilt, 0 cached, 0 new

Rebuild impact:
  root rebuilds: 1 unit
  cascading:     4 units

Root rebuilds:
  0. deeper@0.0.0 (check): file modified: deeper/src/lib.rs
     impact: 4 dependent units rebuilt
       - deep@0.0.0 (check)
       - dep@0.0.0 (check)
       - foo@0.0.0 (check)
       - nested@0.0.0 (check)

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
        .with_stderr_data(str![[r#"
Session: [..]
Status: 6 units rebuilt, 0 cached, 0 new

Rebuild impact:
  root rebuilds: 6 units
  cascading:     0 units

Root rebuilds: (top 5 of 6 by impact)
  0. pkg1@0.0.0 (check): declared features changed: [] -> ["feat"]
     impact: no cascading rebuilds
  1. pkg2@0.0.0 (check): file modified: pkg2/src/lib.rs
     impact: no cascading rebuilds
  2. pkg3@0.0.0 (check): target configuration changed
     impact: no cascading rebuilds
  3. pkg4@0.0.0 (check): environment variable changed (__CARGO_TEST_MY_FOO): <unset> -> 1
     impact: no cascading rebuilds
  4. pkg5@0.0.0 (check): file modified: pkg5/src/lib.rs
     impact: no cascading rebuilds

[NOTE] pass `--verbose` to show all root rebuilds

"#]])
        .run();

    p.cargo("report rebuilds -Zbuild-analysis --verbose")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_stderr_data(str![[r#"
Session: [..]
Status: 6 units rebuilt, 0 cached, 0 new

Rebuild impact:
  root rebuilds: 6 units
  cascading:     0 units

Root rebuilds:
  0. pkg1@0.0.0 (check): declared features changed: [] -> ["feat"]
     impact: no cascading rebuilds
  1. pkg2@0.0.0 (check): file modified: pkg2/src/lib.rs
     impact: no cascading rebuilds
  2. pkg3@0.0.0 (check): target configuration changed
     impact: no cascading rebuilds
  3. pkg4@0.0.0 (check): environment variable changed (__CARGO_TEST_MY_FOO): <unset> -> 1
     impact: no cascading rebuilds
  4. pkg5@0.0.0 (check): file modified: pkg5/src/lib.rs
     impact: no cascading rebuilds
  5. pkg6@0.0.0 (check): file modified: pkg6/src/lib.rs
     impact: no cascading rebuilds

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
        .with_stderr_data(str![[r#"
Session: [..]
Status: 3 units rebuilt, 0 cached, 0 new

Rebuild impact:
  root rebuilds: 1 unit
  cascading:     2 units

Root rebuilds:
  0. common@0.0.0 (check): file modified: common/src/lib.rs
     impact: 2 dependent units rebuilt

[NOTE] pass `-vv` to show all affected rebuilt unit lists

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
        .with_stderr_data(str![[r#"
Session: [..]
Status: 1 unit rebuilt, 0 cached, 0 new

Rebuild impact:
  root rebuilds: 1 unit
  cascading:     0 units

Root rebuilds:
  0. foo@0.0.0 (check): file modified: foo/src/lib.rs
     impact: no cascading rebuilds

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
        .with_stderr_data(str![[r#"
Session: [..]
Status: 1 unit rebuilt, 0 cached, 0 new

Rebuild impact:
  root rebuilds: 1 unit
  cascading:     0 units

Root rebuilds:
  0. bar@0.0.0 (check): file modified: src/lib.rs
     impact: no cascading rebuilds

"#]])
        .run();
}

#[cargo_test]
fn with_session_id() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/lib.rs", "")
        .build();

    // First session: fresh build (1 new unit)
    p.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .run();

    let first_log = paths::log_file(0);
    let first_session_id = first_log.file_stem().unwrap().to_str().unwrap();

    p.change_file("src/lib.rs", "// touched");

    // Second session: rebuild (1 unit rebuilt)
    p.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .run();

    let _ = paths::log_file(1);

    // With --id, should use the first session (not the most recent second)
    p.cargo(&format!(
        "report rebuilds --id {first_session_id} -Zbuild-analysis"
    ))
    .masquerade_as_nightly_cargo(&["build-analysis"])
    .with_stderr_data(str![[r#"
Session: [..]
Status: 0 units rebuilt, 0 cached, 1 new


"#]])
    .run();
}

#[cargo_test]
fn session_id_not_found() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .run();

    p.cargo("report rebuilds --id 20260101T000000000Z-0000000000000000 -Zbuild-analysis")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] session `20260101T000000000Z-0000000000000000` not found for workspace at `[ROOT]/foo`
  |
  = [NOTE] run `cargo report sessions` to list available sessions

"#]])
        .run();
}

#[cargo_test]
fn invalid_session_id_format() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/lib.rs", "")
        .build();

    p.cargo("report rebuilds --id invalid-session-id -Zbuild-analysis")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] expect run ID in format `20060724T012128000Z-<16-char-hex>`, got `invalid-session-id`

"#]])
        .run();
}
