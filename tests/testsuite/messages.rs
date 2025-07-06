//! General tests specifically about diagnostics and other messages.
//!
//! Tests for message caching can be found in `cache_messages`.

use crate::prelude::*;
use cargo_test_support::compare::assert_e2e;
use cargo_test_support::{Project, process, project};
use cargo_util::ProcessError;

/// Captures the actual diagnostics displayed by rustc. This is done to avoid
/// relying on the exact message formatting in rustc.
pub fn raw_rustc_output(project: &Project, path: &str, extra: &[&str]) -> String {
    let mut proc = process("rustc");
    if cfg!(windows) {
        // Sanitize in case the caller wants to do direct string comparison with Cargo's output.
        proc.arg(path.replace('/', "\\"));
    } else {
        proc.arg(path);
    }
    let rustc_output = match proc
        .arg("--crate-type=lib")
        .args(extra)
        .cwd(project.root())
        .exec_with_output()
    {
        Ok(output) => output.stderr,
        Err(e) => e.downcast::<ProcessError>().unwrap().stderr.unwrap(),
    };
    // Do a little dance to remove rustc's "warnings emitted" message and the subsequent newline.
    let stderr = std::str::from_utf8(&rustc_output).expect("utf8");
    let mut lines = stderr.lines();
    let mut result = String::new();
    while let Some(line) = lines.next() {
        if line.contains("warning emitted")
            || line.contains("warnings emitted")
            || line.contains("aborting due to")
        {
            // Eat blank line.
            match lines.next() {
                None | Some("") => continue,
                Some(s) => panic!("unexpected str {}", s),
            }
        }
        result.push_str(line);
        result.push('\n');
    }
    result
}

fn redact_rustc_message(msg: &str) -> impl IntoData {
    use snapbox::filter::{Filter, FilterPaths};
    let assert = assert_e2e();
    let redactions = assert.redactions();
    let msg = redactions.redact(msg);
    FilterPaths.filter(msg.into())
}

#[cargo_test]
fn deduplicate_messages_basic() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
                pub fn foo() {
                    let x = 1;
                }
            "#,
        )
        .build();
    let rustc_message = raw_rustc_output(&p, "src/lib.rs", &[]);
    let expected_output = format!(
        "{}\
[WARNING] `foo` (lib) generated 1 warning[..]
[WARNING] `foo` (lib test) generated 1 warning (1 duplicate)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[EXECUTABLE] unittests src/lib.rs (target/debug/deps/foo-[HASH][EXE])
",
        rustc_message
    );
    p.cargo("test --no-run -j1")
        .with_stderr_data(redact_rustc_message(&format!(
            "\
[COMPILING] foo v0.0.1 ([ROOT]/foo)
{}",
            expected_output
        )))
        .run();
    // Run again, to check for caching behavior.
    p.cargo("test --no-run -j1")
        .with_stderr_data(redact_rustc_message(&expected_output))
        .run();
}

#[cargo_test]
fn deduplicate_messages_mismatched_warnings() {
    // One execution prints 1 warning, the other prints 2 where there is an overlap.
    let p = project()
        .file(
            "src/lib.rs",
            r#"
                pub fn foo() {
                    let x = 1;
                }

                #[test]
                fn t1() {
                    let MY_VALUE = 1;
                    assert_eq!(MY_VALUE, 1);
                }
            "#,
        )
        .build();
    let lib_output = raw_rustc_output(&p, "src/lib.rs", &[]);
    let mut lib_test_output = raw_rustc_output(&p, "src/lib.rs", &["--test"]);
    // Remove the duplicate warning.
    let start = lib_test_output.find(&lib_output).expect("same warning");
    lib_test_output.replace_range(start..start + lib_output.len(), "");
    let expected_output = format!(
        "\
{}\
[WARNING] `foo` (lib) generated 1 warning[..]
{}\
[WARNING] `foo` (lib test) generated 2 warnings (1 duplicate)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[EXECUTABLE] unittests src/lib.rs (target/debug/deps/foo-[HASH][EXE])
",
        lib_output, lib_test_output
    );
    p.cargo("test --no-run -j1")
        .with_stderr_data(redact_rustc_message(&format!(
            "\
[COMPILING] foo v0.0.1 ([ROOT]/foo)
{}",
            expected_output
        )))
        .run();
    // Run again, to check for caching behavior.
    p.cargo("test --no-run -j1")
        .with_stderr_data(redact_rustc_message(&expected_output))
        .run();
}

#[cargo_test]
fn deduplicate_errors() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
                this should not compile
            "#,
        )
        .build();
    let rustc_message = raw_rustc_output(&p, "src/lib.rs", &[]);
    p.cargo("test -j1")
        .with_status(101)
        .with_stderr_data(redact_rustc_message(&format!(
            "\
[COMPILING] foo v0.0.1 ([ROOT]/foo)
{}[ERROR] could not compile `foo` (lib) due to 1 previous error
",
            rustc_message
        )))
        .run();
}
