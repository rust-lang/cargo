//! Tests for the `cargo fix` command, specifically targeting the logic around
//! running rustc multiple times to apply and verify fixes.
//!
//! These tests use a replacement of rustc ("rustc-fix-shim") which emits JSON
//! messages based on what the test is exercising. It uses an environment
//! variable `RUSTC_FIX_SHIM_SEQUENCE` which determines how it should behave
//! based on how many times `rustc` has run. It keeps track of how many times
//! rustc has run in a local file.
//!
//! For example, a sequence of `[Step::OneFix, Step::Error]` will emit one
//! suggested fix the first time `rustc` is run, and then the next time it is
//! run it will generate an error.
//!
//! The [`expect_fix_runs_rustc_n_times`] function handles setting everything
//! up, and verifying the results.

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use crate::prelude::*;
use crate::utils::tools;
use cargo_test_support::{Execs, basic_manifest, paths, project, str};

/// The action that the `rustc` shim should take in the current sequence of
/// events.
#[derive(Clone, Copy, PartialEq)]
#[repr(u8)]
enum Step {
    /// Exits with success with no messages.
    SuccessNoOutput = b'0',
    /// Emits one suggested fix.
    ///
    /// The suggested fix involves updating the number of the first line
    /// comment which starts as `// fix-count 0`.
    OneFix = b'1',
    /// Emits two suggested fixes which overlap, which rustfix can only apply
    /// one of them, and fails for the other.
    ///
    /// The suggested fix is the same as `Step::OneFix`, it just shows up
    /// twice.
    TwoFixOverlapping = b'2',
    /// Generates a warning without a suggestion.
    Warning = b'w',
    /// Generates an error message with no suggestion.
    Error = b'e',
    /// Emits one suggested fix and an error.
    OneFixError = b'f',
}

/// Verifies `cargo fix` behavior based on the given sequence of behaviors for
/// `rustc`.
///
/// - `sequence` is the sequence of behaviors for each call to `rustc`.
///   If rustc is called more often than the number of steps, then it will panic.
/// - `extra_execs` a callback that allows extra customization of the [`Execs`].
/// - `expected_stderr` is the expected output from cargo.
/// - `expected_lib_rs` is the expected contents of `src/lib.rs` after the
///   fixes have been applied. The file starts out with the content `//
///   fix-count 0`, and the number increases based on which suggestions are
///   applied.
fn expect_fix_runs_rustc_n_times(
    sequence: &[Step],
    extra_execs: impl FnOnce(&mut Execs),
    expected_stderr: impl IntoData,
    expected_lib_rs: &str,
) {
    let rustc = rustc_for_cargo_fix();
    let p = project().file("src/lib.rs", "// fix-count 0").build();

    let sequence_vec: Vec<_> = sequence.iter().map(|x| *x as u8).collect();
    let sequence_str = std::str::from_utf8(&sequence_vec).unwrap();

    let mut execs = p.cargo("fix --allow-no-vcs --lib");
    execs
        .env("RUSTC", &rustc)
        .env("RUSTC_FIX_SHIM_SEQUENCE", sequence_str)
        .with_stderr_data(expected_stderr);
    extra_execs(&mut execs);
    execs.run();
    let lib_rs = p.read_file("src/lib.rs");
    assert_eq!(expected_lib_rs, lib_rs);
    let count: usize = p.read_file("rustc-fix-shim-count").parse().unwrap();
    assert_eq!(sequence.len(), count);
}

/// Returns the path to the rustc replacement executable.
fn rustc_for_cargo_fix() -> PathBuf {
    static FIX_SHIM: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();

    let mut lock = FIX_SHIM.get_or_init(|| Default::default()).lock().unwrap();
    if let Some(path) = &*lock {
        return path.clone();
    }

    let p = project()
        .at(paths::global_root().join("rustc-fix-shim"))
        .file("Cargo.toml", &basic_manifest("rustc-fix-shim", "1.0.0"))
        .file(
            "src/main.rs",
            r##"
fn main() {
    if std::env::var_os("CARGO_PKG_NAME").is_none() {
        // Handle things like rustc -Vv
        let r = std::process::Command::new("rustc")
            .args(std::env::args_os().skip(1))
            .status();
        std::process::exit(r.unwrap().code().unwrap_or(2));
    }

    // Keep track of which step in the sequence that needs to run.
    let successful_count = std::fs::read_to_string("rustc-fix-shim-count")
        .map(|c| c.parse().unwrap())
        .unwrap_or(0);
    std::fs::write("rustc-fix-shim-count", format!("{}", successful_count + 1)).unwrap();
    // The sequence tells us which behavior we should have.
    let seq = std::env::var("RUSTC_FIX_SHIM_SEQUENCE").unwrap();
    if successful_count >= seq.len() {
        panic!("rustc called too many times count={}, \
                make sure to update the Step sequence", successful_count);
    }
    match seq.as_bytes()[successful_count] {
        b'0' => return,
        b'1' => {
            output_suggestion(successful_count + 1);
        }
        b'2' => {
            output_suggestion(successful_count + 1);
            output_suggestion(successful_count + 2);
        }
        b'w' => {
            output_message("warning", successful_count + 1);
        }
        b'e' => {
            output_message("error", successful_count + 1);
            std::process::exit(1);
        }
        b'f' => {
            output_suggestion(successful_count + 1);
            output_message("error", successful_count + 2);
            std::process::exit(1);
        }
        _ => panic!("unexpected sequence"),
    }
}

fn output_suggestion(count: usize) {
    let json = format!(
        r#"{{
            "$message_type": "diagnostic",
            "message": "rustc fix shim comment {count}",
            "code": null,
            "level": "warning",
            "spans":
            [
                {{
                    "file_name": "src/lib.rs",
                    "byte_start": 13,
                    "byte_end": 14,
                    "line_start": 1,
                    "line_end": 1,
                    "column_start": 14,
                    "column_end": 15,
                    "is_primary": true,
                    "text":
                    [
                        {{
                            "text": "// fix-count 0",
                            "highlight_start": 14,
                            "highlight_end": 15
                        }}
                    ],
                    "label": "increase this number",
                    "suggested_replacement": null,
                    "suggestion_applicability": null,
                    "expansion": null
                }}
            ],
            "children":
            [
                {{
                    "message": "update the number here",
                    "code": null,
                    "level": "help",
                    "spans":
                    [
                        {{
                            "file_name": "src/lib.rs",
                            "byte_start": 13,
                            "byte_end": 14,
                            "line_start": 1,
                            "line_end": 1,
                            "column_start": 14,
                            "column_end": 15,
                            "is_primary": true,
                            "text":
                            [
                                {{
                                    "text": "// fix-count 0",
                                    "highlight_start": 14,
                                    "highlight_end": 15
                                }}
                            ],
                            "label": null,
                            "suggested_replacement": "{count}",
                            "suggestion_applicability": "MachineApplicable",
                            "expansion": null
                        }}
                    ],
                    "children": [],
                    "rendered": null
                }}
            ],
            "rendered": "rustc fix shim comment {count}"
        }}"#,
    )
    .replace("\n", "");
    eprintln!("{json}");
}

fn output_message(level: &str, count: usize) {
    let json = format!(
        r#"{{
    "$message_type": "diagnostic",
    "message": "rustc fix shim {level} count={count}",
    "code": null,
    "level": "{level}",
    "spans":
    [
        {{
            "file_name": "src/lib.rs",
            "byte_start": 0,
            "byte_end": 0,
            "line_start": 1,
            "line_end": 1,
            "column_start": 1,
            "column_end": 1,
            "is_primary": true,
            "text":
            [
                {{
                    "text": "// fix-count 0",
                    "highlight_start": 1,
                    "highlight_end": 4
                }}
            ],
            "label": "forced error",
            "suggested_replacement": null,
            "suggestion_applicability": null,
            "expansion": null
        }}
    ],
    "children": [],
    "rendered": "rustc fix shim {level} count={count}"
}}"#,
    )
    .replace("\n", "");
    eprintln!("{json}");
}
            "##,
        )
        .build();
    p.cargo("build").run();
    let path = p.bin("rustc-fix-shim");
    *lock = Some(path.clone());
    path
}

#[cargo_test]
fn fix_no_suggestions() {
    // No suggested fixes.
    expect_fix_runs_rustc_n_times(
        &[Step::SuccessNoOutput],
        |_execs| {},
        str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
        "// fix-count 0",
    );
}

#[cargo_test]
fn fix_one_suggestion() {
    // One suggested fix, with a successful verification, no output.
    expect_fix_runs_rustc_n_times(
        &[Step::OneFix, Step::SuccessNoOutput],
        |_execs| {},
        str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FIXED] src/lib.rs (1 fix)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
        "// fix-count 1",
    );
}

#[cargo_test]
fn fix_one_overlapping() {
    // Two suggested fixes, where one fails, then the next step returns no suggestions.
    expect_fix_runs_rustc_n_times(
        &[Step::TwoFixOverlapping, Step::SuccessNoOutput],
        |_execs| {},
        str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FIXED] src/lib.rs (1 fix)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
        "// fix-count 2",
    );
}

#[cargo_test]
fn fix_overlapping_max() {
    // Rustc repeatedly spits out suggestions that overlap, which should hit
    // the limit of 4 attempts. It should show the output from the 5th attempt.
    expect_fix_runs_rustc_n_times(
        &[
            Step::TwoFixOverlapping,
            Step::TwoFixOverlapping,
            Step::TwoFixOverlapping,
            Step::TwoFixOverlapping,
            Step::TwoFixOverlapping,
        ],
        |_execs| {},
        str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[WARNING] error applying suggestions to `src/lib.rs`

The full error message was:

> cannot replace slice of data that was already replaced

This likely indicates a bug in either rustc or cargo itself,
and we would appreciate a bug report! You're likely to see
a number of compiler warnings after this message which cargo
attempted to fix but failed. If you could open an issue at
https://github.com/rust-lang/rust/issues
quoting the full output of this command we'd be very appreciative!
Note that you may be able to make some more progress in the near-term
fixing code with the `--broken-code` flag

[FIXED] src/lib.rs (4 fixes)
rustc fix shim comment 5
rustc fix shim comment 6
[WARNING] `foo` (lib) generated 2 warnings (run `cargo fix --lib -p foo` to apply 2 suggestions)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
        "// fix-count 5",
    );
}

#[cargo_test]
fn fix_verification_failed() {
    // One suggested fix, with an error in the verification step.
    // This should cause `cargo fix` to back out the changes.
    expect_fix_runs_rustc_n_times(
        &[Step::OneFix, Step::Error],
        |_execs| {},
        str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[WARNING] failed to automatically apply fixes suggested by rustc to crate `foo`

after fixes were automatically applied the compiler reported errors within these files:

  * src/lib.rs

This likely indicates a bug in either rustc or cargo itself,
and we would appreciate a bug report! You're likely to see
a number of compiler warnings after this message which cargo
attempted to fix but failed. If you could open an issue at
https://github.com/rust-lang/rust/issues
quoting the full output of this command we'd be very appreciative!
Note that you may be able to make some more progress in the near-term
fixing code with the `--broken-code` flag

The following errors were reported:
rustc fix shim error count=2
Original diagnostics will follow.

rustc fix shim comment 1
[WARNING] `foo` (lib) generated 1 warning (run `cargo fix --lib -p foo` to apply 1 suggestion)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
        "// fix-count 0",
    );
}

#[cargo_test]
fn fix_verification_failed_clippy() {
    // This is the same as `fix_verification_failed_clippy`, except it checks
    // the error message has the customization for the clippy URL and
    // subcommand.
    expect_fix_runs_rustc_n_times(
        &[Step::OneFix, Step::Error],
        |execs| {
            execs.env("RUSTC_WORKSPACE_WRAPPER", tools::wrapped_clippy_driver());
        },
        str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[WARNING] failed to automatically apply fixes suggested by rustc to crate `foo`

after fixes were automatically applied the compiler reported errors within these files:

  * src/lib.rs

This likely indicates a bug in either rustc or cargo itself,
and we would appreciate a bug report! You're likely to see
a number of compiler warnings after this message which cargo
attempted to fix but failed. If you could open an issue at
https://github.com/rust-lang/rust-clippy/issues
quoting the full output of this command we'd be very appreciative!
Note that you may be able to make some more progress in the near-term
fixing code with the `--broken-code` flag

The following errors were reported:
rustc fix shim error count=2
Original diagnostics will follow.

rustc fix shim comment 1
[WARNING] `foo` (lib) generated 1 warning (run `cargo clippy --fix --lib -p foo` to apply 1 suggestion)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
        "// fix-count 0",
    );
}

#[cargo_test]
fn warnings() {
    // Only emits warnings.
    expect_fix_runs_rustc_n_times(
        &[Step::Warning],
        |_execs| {},
        str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
rustc fix shim warning count=1
[WARNING] `foo` (lib) generated 1 warning
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
        "// fix-count 0",
    );
}

#[cargo_test]
fn starts_with_error() {
    // The source code doesn't compile to start with.
    expect_fix_runs_rustc_n_times(
        &[Step::Error],
        |execs| {
            execs.with_status(101);
        },
        str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
rustc fix shim error count=1
[ERROR] could not compile `foo` (lib) due to 1 previous error

"#]],
        "// fix-count 0",
    );
}

#[cargo_test]
fn broken_code_no_suggestions() {
    // --broken-code with no suggestions
    expect_fix_runs_rustc_n_times(
        &[Step::Error],
        |execs| {
            execs.arg("--broken-code").with_status(101);
        },
        str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
rustc fix shim error count=1
[ERROR] could not compile `foo` (lib) due to 1 previous error

"#]],
        "// fix-count 0",
    );
}

#[cargo_test]
fn broken_code_one_suggestion() {
    // --broken-code where there is an error and a suggestion.
    expect_fix_runs_rustc_n_times(
        &[Step::OneFixError, Step::Error],
        |execs| {
            execs.arg("--broken-code").with_status(101);
        },
        str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[WARNING] failed to automatically apply fixes suggested by rustc to crate `foo`

after fixes were automatically applied the compiler reported errors within these files:

  * src/lib.rs

This likely indicates a bug in either rustc or cargo itself,
and we would appreciate a bug report! You're likely to see
a number of compiler warnings after this message which cargo
attempted to fix but failed. If you could open an issue at
https://github.com/rust-lang/rust/issues
quoting the full output of this command we'd be very appreciative!
Note that you may be able to make some more progress in the near-term
fixing code with the `--broken-code` flag

The following errors were reported:
rustc fix shim error count=2
Original diagnostics will follow.

rustc fix shim comment 1
rustc fix shim error count=2
[WARNING] `foo` (lib) generated 1 warning
[ERROR] could not compile `foo` (lib) due to 1 previous error; 1 warning emitted

"#]],
        "// fix-count 1",
    );
}
