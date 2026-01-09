//! Tests for `cargo report timings`.

mod help;

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

    p.cargo("report timings")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] the `cargo report timings` command is unstable, and only available on the nightly channel of Cargo, but this is the `stable` channel
See https://doc.rust-lang.org/book/appendix-07-nightly-rust.html for more information about Rust release channels.
See https://github.com/rust-lang/cargo/issues/15844 for more information about the `cargo report timings` command.

"#]])
        .run();
}

#[cargo_test]
fn gated_unstable_options() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/lib.rs", "")
        .build();

    p.cargo("report timings")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] the `cargo report timings` command is unstable, pass `-Z build-analysis` to enable it
See https://github.com/rust-lang/cargo/issues/15844 for more information about the `cargo report timings` command.

"#]])
        .run();
}

#[cargo_test]
fn no_log() {
    cargo_process("report timings -Zbuild-analysis")
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

    bar.cargo("report timings -Zbuild-analysis")
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
fn invalid_log() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .run();

    // Put some junks in the log file.
    std::fs::write(paths::log_file(0), "}|x| hello world").unwrap();

    p.cargo("report timings -Zbuild-analysis")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to analyze log at `[ROOT]/home/.cargo/log/[..]T[..]Z-[..].jsonl`

Caused by:
  no timing data found in log

"#]])
        .run();
}

#[cargo_test]
fn empty_log() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .run();

    // Truncate the log file.
    std::fs::File::create(paths::log_file(0)).unwrap();

    // If the make-up log file was picked, the command would have failed.
    p.cargo("report timings -Zbuild-analysis")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to analyze log at `[ROOT]/home/.cargo/log/[..]T[..]Z-[..].jsonl`

Caused by:
  no timing data found in log

"#]])
        .run();
}

#[cargo_test]
fn prefer_latest() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .run();

    // Put some junks in the first log file.
    std::fs::write(paths::log_file(0), "}|x| hello world").unwrap();

    p.change_file("src/lib.rs", "pub fn foo() {}");
    p.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .run();

    // the second log file got generated.
    let _ = paths::log_file(1);

    // if it had picked the corrupted first log file, it would have failed.
    p.cargo("report timings -Zbuild-analysis")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_stderr_data(str![[r#"
      Timing report saved to [ROOT]/foo/target/cargo-timings/cargo-timing-[..]T[..]Z-[..].html

"#]])
        .run();

    assert_eq!(p.glob("**/cargo-timing-*.html").count(), 1);
}

#[cargo_test]
fn prefer_workspace() {
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

    bar.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .run();

    // Put some junks in the newest log file.
    std::fs::write(paths::log_file(1), "}|x| hello world").unwrap();

    // Back to foo, if it had picked the corrupted log file, it would have failed.
    foo.cargo("report timings -Zbuild-analysis")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_stderr_data(str![[r#"
      Timing report saved to [ROOT]/foo/target/cargo-timings/cargo-timing-[..]T[..]Z-[..].html

"#]])
        .run();

    assert_eq!(foo.glob("**/cargo-timing-*.html").count(), 1);
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

    // cd to outside the workspace, it should
    // * retrieve the latest log
    // * save the report in a temp directory
    cargo_process("report timings -Zbuild-analysis")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_stderr_data(str![[r#"
      Timing report saved to [..]/cargo-timing-[..]T[..]Z-[..].html

"#]])
        .run();

    // Have no timing HTML under target directory
    assert_eq!(p.glob("**/cargo-timing-*.html").count(), 0);
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

    foo.cargo("report timings --manifest-path ../bar/Cargo.toml -Zbuild-analysis")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_stderr_data(str![[r#"
      Timing report saved to [ROOT]/bar/target/cargo-timings/cargo-timing-[..]T[..]-[..].html

"#]])
        .run();
}

#[cargo_test(nightly, reason = "rustc --json=timings is unstable")]
fn with_section_timings() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/lib.rs", "pub fn foo() {}")
        .build();

    p.cargo("check -Zbuild-analysis -Zsection-timings")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis", "section-timings"])
        .run();

    p.cargo("report timings -Zbuild-analysis")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_stderr_data(str![[r#"
      Timing report saved to [ROOT]/foo/target/cargo-timings/cargo-timing-[..]T[..]Z-[..].html

"#]])
        .run();

    assert_eq!(p.glob("**/cargo-timing-*.html").count(), 1);
}

#[cargo_test]
fn with_multiple_targets() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            edition = "2021"
            "#,
        )
        .file("src/lib.rs", "pub fn lib_fn() {}")
        .file("src/main.rs", "fn main() {}")
        .file("src/bin/extra.rs", "fn main() {}")
        .file("examples/ex1.rs", "fn main() {}")
        .file("tests/t1.rs", "#[test] fn test1() {}")
        .build();

    p.cargo("check --all-targets -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .run();

    p.cargo("report timings -Zbuild-analysis")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_stderr_data(str![[r#"
      Timing report saved to [ROOT]/foo/target/cargo-timings/cargo-timing-[..]T[..]Z-[..].html

"#]])
        .run();

    assert_eq!(p.glob("**/cargo-timing-*.html").count(), 1);
}

#[cargo_test]
fn with_session_id() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .run();

    let first_log = paths::log_file(0);
    let first_session_id = first_log.file_stem().unwrap().to_str().unwrap();

    p.change_file("src/lib.rs", "pub fn foo() {}");
    p.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .run();

    let second_log = paths::log_file(1);
    let second_session_id = second_log.file_stem().unwrap().to_str().unwrap();

    // With --id, should use the first session (not the most recent second)
    p.cargo(&format!(
        "report timings --id {first_session_id} -Zbuild-analysis"
    ))
    .masquerade_as_nightly_cargo(&["build-analysis"])
    .with_stderr_data(str![[r#"
      Timing report saved to [ROOT]/foo/target/cargo-timings/cargo-timing-[..]T[..]Z-[..].html

"#]])
    .run();

    let timing_files: Vec<_> = p.glob("**/cargo-timing-*.html").collect();
    assert_eq!(timing_files.len(), 1);
    let timing_file = timing_files[0].as_ref().unwrap();
    let filename = timing_file.file_name().unwrap().to_str().unwrap();
    assert!(
        filename.contains(first_session_id),
        "Expected timing file to contain first session ID {first_session_id}, got {filename}"
    );
    assert!(
        !filename.contains(second_session_id),
        "Should not contain second session ID {second_session_id}, got {filename}"
    );
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

    p.cargo("report timings --id 20260101T000000000Z-0000000000000000 -Zbuild-analysis")
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

    p.cargo("report timings --id invalid-session-id -Zbuild-analysis")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] expect run ID in format `20060724T012128000Z-<16-char-hex>`, got `invalid-session-id`

"#]])
        .run();
}
