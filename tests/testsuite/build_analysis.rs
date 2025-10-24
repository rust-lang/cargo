//! Tests for `-Zbuild-analysis`.

use crate::prelude::*;

use cargo_test_support::basic_manifest;
use cargo_test_support::compare::assert_e2e;
use cargo_test_support::paths;
use cargo_test_support::project;
use cargo_test_support::str;

#[cargo_test]
fn gated() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_stderr_data(str![[r#"
[WARNING] ignoring 'build.analysis' config, pass `-Zbuild-analysis` to enable it
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn one_logfile_per_invocation() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/lib.rs", "")
        .build();

    let cargo_home = paths::cargo_home();
    let log_dir = cargo_home.join("log");

    // First invocation
    p.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    assert!(log_dir.exists());
    let entries = std::fs::read_dir(&log_dir).unwrap();
    let log_files: Vec<_> = entries
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("jsonl"))
        .collect();

    assert_eq!(log_files.len(), 1);

    // Second invocation
    p.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let entries = std::fs::read_dir(&log_dir).unwrap();
    let log_files: Vec<_> = entries
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("jsonl"))
        .collect();

    assert_eq!(log_files.len(), 2);
}

#[cargo_test]
fn log_msg_build_started() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .run();

    let cargo_home = paths::cargo_home();
    let log_dir = cargo_home.join("log");
    assert!(log_dir.exists());

    let entries = std::fs::read_dir(&log_dir).unwrap();
    let log_file = entries
        .filter_map(Result::ok)
        .find(|e| e.path().extension().and_then(|s| s.to_str()) == Some("jsonl"))
        .unwrap();

    let content = std::fs::read_to_string(log_file.path()).unwrap();

    assert_e2e().eq(
        &content,
        str![[r#"
[
  {
    "cwd": "[ROOT]/foo",
    "host": "[HOST_TARGET]",
    "jobs": "{...}",
    "profile": "dev",
    "reason": "build-started",
    "run_id": "[..]T[..]Z-[..]",
    "rustc_version": "1.[..]",
    "rustc_version_verbose": "{...}",
    "target_dir": "[ROOT]/foo/target",
    "timestamp": "[..]T[..]Z",
    "workspace_root": "[ROOT]/foo"
  }
]
"#]]
        .is_json()
        .against_jsonlines(),
    );
}
