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

    // First invocation
    p.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let _ = get_log(0);

    // Second invocation
    p.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let _ = get_log(1);
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

    assert_e2e().eq(
        &get_log(0),
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
  },
  "{...}"
]
"#]]
        .is_json()
        .against_jsonlines(),
    );
}

#[cargo_test]
fn log_msg_timing_info() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2015"

                [dependencies]
                bar = { path = "bar" }
            "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.0"))
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .run();

    assert_e2e().eq(
        &get_log(0),
        str![[r#"
[
  {
    "reason": "build-started",
    "...": "{...}"
  },
  {
    "duration": "{...}",
    "mode": "check",
    "package_id": "path+[ROOTURL]/foo/bar#0.0.0",
    "reason": "timing-info",
    "rmeta_time": "{...}",
    "run_id": "[..]T[..]Z-[..]",
    "target": "{...}",
    "timestamp": "[..]T[..]Z"
  },
  {
    "duration": "{...}",
    "mode": "check",
    "package_id": "path+[ROOTURL]/foo#0.0.0",
    "reason": "timing-info",
    "rmeta_time": "{...}",
    "run_id": "[..]T[..]Z-[..]",
    "target": "{...}",
    "timestamp": "[..]T[..]Z"
  }
]
"#]]
        .is_json()
        .against_jsonlines(),
    );
}

#[cargo_test]
fn log_rebuild_reason_fresh_build() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    // Fresh builds do NOT log rebuild-reason
    // Only build-started and timing-info are logged
    assert_e2e().eq(
        &get_log(0),
        str![[r#"
[
  {
    "...": "{...}",
    "reason": "build-started"
  },
  {
    "...": "{...}",
    "reason": "timing-info"
  }
]
"#]]
        .is_json()
        .against_jsonlines(),
    );
}

#[cargo_test]
fn log_rebuild_reason_file_changed() {
    // Test that changing a file logs the appropriate rebuild reason
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/lib.rs", "")
        .build();

    p.cargo("check").run();

    // Change source file
    p.change_file("src/lib.rs", "//! comment");

    p.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    // File changes SHOULD log rebuild-reason
    assert_e2e().eq(
        &get_log(0),
        str![[r#"
[
  {
    "...": "{...}",
    "reason": "build-started"
  },
  {
    "cause": {
      "dirty_reason": "fs-status-outdated",
      "fs_status": "stale-item",
      "reference": "[ROOT]/foo/target/debug/.fingerprint/foo-[HASH]/dep-lib-foo",
      "reference_mtime": "{...}",
      "stale": "[ROOT]/foo/src/lib.rs",
      "stale_item": "changed-file",
      "stale_mtime": "{...}"
    },
    "mode": "check",
    "package_id": "path+[ROOTURL]/foo#0.0.0",
    "reason": "rebuild",
    "run_id": "[..]T[..]Z-[..]",
    "target": "{...}",
    "timestamp": "[..]T[..]Z"
  },
  {
    "...": "{...}",
    "reason": "timing-info"
  }
]
"#]]
        .is_json()
        .against_jsonlines(),
    );
}

#[cargo_test]
fn log_rebuild_reason_no_rebuild() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/lib.rs", "")
        .build();

    // First build
    p.cargo("check").run();

    // Second build without changes
    p.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    // Should NOT contain any rebuild-reason messages since nothing rebuilt
    assert_e2e().eq(
        &get_log(0),
        str![[r#"
[
  {
    "reason": "build-started",
    "...": "{...}"
  }
]
"#]]
        .is_json()
        .against_jsonlines(),
    );
}

/// This also asserts the number of log files is exactly the same as `idx + 1`.
fn get_log(idx: usize) -> String {
    let cargo_home = paths::cargo_home();
    let log_dir = cargo_home.join("log");

    let entries = std::fs::read_dir(&log_dir).unwrap();
    let mut log_files: Vec<_> = entries
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("jsonl"))
        .collect();

    // Sort them to get chronological order
    log_files.sort_unstable_by(|a, b| a.file_name().to_str().cmp(&b.file_name().to_str()));

    assert_eq!(
        idx + 1,
        log_files.len(),
        "unexpected number of log files: {}, expected {}",
        log_files.len(),
        idx + 1
    );

    std::fs::read_to_string(log_files[idx].path()).unwrap()
}
