//! Tests for future-incompat-report messages
//!
//! Note that these tests use the -Zfuture-incompat-test for rustc.
//! This causes rustc to treat *every* lint as future-incompatible.
//! This is done because future-incompatible lints are inherently
//! ephemeral, but we don't want to continually update these tests.
//! So we pick some random lint that will likely always be the same
//! over time.

use super::config::write_config_toml;
use cargo_test_support::registry::Package;
use cargo_test_support::{basic_manifest, project, Project};

// An arbitrary lint (unused_variables) that triggers a lint.
// We use a special flag to force it to generate a report.
const FUTURE_EXAMPLE: &'static str = "fn main() { let x = 1; }";
// Some text that will be displayed when the lint fires.
const FUTURE_OUTPUT: &'static str = "[..]unused_variables[..]";

fn simple_project() -> Project {
    project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/main.rs", FUTURE_EXAMPLE)
        .build()
}

#[cargo_test(
    nightly,
    reason = "-Zfuture-incompat-test requires nightly (permanently)"
)]
fn output_on_stable() {
    let p = simple_project();

    p.cargo("check")
        .env("RUSTFLAGS", "-Zfuture-incompat-test")
        .with_stderr_contains(FUTURE_OUTPUT)
        .with_stderr_contains("[..]cargo report[..]")
        .run();
}

// This feature is stable, and should not be gated
#[cargo_test]
fn no_gate_future_incompat_report() {
    let p = simple_project();

    p.cargo("check --future-incompat-report")
        .with_status(0)
        .run();

    p.cargo("report future-incompatibilities --id foo")
        .with_stderr_contains("error: no reports are currently available")
        .with_status(101)
        .run();
}

#[cargo_test(
    nightly,
    reason = "-Zfuture-incompat-test requires nightly (permanently)"
)]
fn test_zero_future_incompat() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/main.rs", "fn main() {}")
        .build();

    // No note if --future-incompat-report is not specified.
    p.cargo("check")
        .env("RUSTFLAGS", "-Zfuture-incompat-test")
        .with_stderr(
            "\
[CHECKING] foo v0.0.0 [..]
[FINISHED] [..]
",
        )
        .run();

    p.cargo("check --future-incompat-report")
        .env("RUSTFLAGS", "-Zfuture-incompat-test")
        .with_stderr(
            "\
[FINISHED] [..]
note: 0 dependencies had future-incompatible warnings
",
        )
        .run();
}

#[cargo_test(
    nightly,
    reason = "-Zfuture-incompat-test requires nightly (permanently)"
)]
fn test_single_crate() {
    let p = simple_project();

    for command in &["build", "check", "rustc", "test"] {
        let check_has_future_compat = || {
            p.cargo(command)
                .env("RUSTFLAGS", "-Zfuture-incompat-test")
                .with_stderr_contains(FUTURE_OUTPUT)
                .with_stderr_contains("warning: the following packages contain code that will be rejected by a future version of Rust: foo v0.0.0 [..]")
                .with_stderr_does_not_contain("[..]incompatibility[..]")
                .run();
        };

        // Check that we show a message with no [future-incompat-report] config section
        write_config_toml("");
        check_has_future_compat();

        // Check that we show a message with `frequency = "always"`
        write_config_toml(
            "\
[future-incompat-report]
frequency = 'always'
",
        );
        check_has_future_compat();

        // Check that we do not show a message with `frequency = "never"`
        write_config_toml(
            "\
[future-incompat-report]
frequency = 'never'
",
        );
        p.cargo(command)
            .env("RUSTFLAGS", "-Zfuture-incompat-test")
            .with_stderr_contains(FUTURE_OUTPUT)
            .with_stderr_does_not_contain("[..]rejected[..]")
            .with_stderr_does_not_contain("[..]incompatibility[..]")
            .run();

        // Check that passing `--future-incompat-report` overrides `frequency = 'never'`
        p.cargo(command).arg("--future-incompat-report")
            .env("RUSTFLAGS", "-Zfuture-incompat-test")
            .with_stderr_contains(FUTURE_OUTPUT)
            .with_stderr_contains("warning: the following packages contain code that will be rejected by a future version of Rust: foo v0.0.0 [..]")
            .with_stderr_contains("  - foo@0.0.0[..]")
            .run();
    }
}

#[cargo_test(
    nightly,
    reason = "-Zfuture-incompat-test requires nightly (permanently)"
)]
fn test_multi_crate() {
    Package::new("first-dep", "0.0.1")
        .file("src/lib.rs", FUTURE_EXAMPLE)
        .publish();
    Package::new("second-dep", "0.0.2")
        .file("src/lib.rs", FUTURE_EXAMPLE)
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"

                [dependencies]
                first-dep = "*"
                second-dep = "*"
              "#,
        )
        .file("src/lib.rs", "")
        .build();

    for command in &["build", "check", "rustc", "test"] {
        p.cargo(command)
            .env("RUSTFLAGS", "-Zfuture-incompat-test")
            .with_stderr_does_not_contain(FUTURE_OUTPUT)
            .with_stderr_contains("warning: the following packages contain code that will be rejected by a future version of Rust: first-dep v0.0.1, second-dep v0.0.2")
            // Check that we don't have the 'triggers' message shown at the bottom of this loop,
            // and that we don't explain how to show a per-package report
            .with_stderr_does_not_contain("[..]triggers[..]")
            .with_stderr_does_not_contain("[..]--package[..]")
            .with_stderr_does_not_contain("[..]-p[..]")
            .run();

        p.cargo(command).arg("--future-incompat-report")
            .env("RUSTFLAGS", "-Zfuture-incompat-test")
            .with_stderr_contains("warning: the following packages contain code that will be rejected by a future version of Rust: first-dep v0.0.1, second-dep v0.0.2")
            .with_stderr_contains("  - first-dep@0.0.1")
            .with_stderr_contains("  - second-dep@0.0.2")
            .run();

        p.cargo("report future-incompatibilities").arg("--package").arg("first-dep@0.0.1")
            .with_stdout_contains("The package `first-dep v0.0.1` currently triggers the following future incompatibility lints:")
            .with_stdout_contains(FUTURE_OUTPUT)
            .with_stdout_does_not_contain("[..]second-dep-0.0.2/src[..]")
            .run();

        p.cargo("report future-incompatibilities").arg("--package").arg("second-dep@0.0.2")
            .with_stdout_contains("The package `second-dep v0.0.2` currently triggers the following future incompatibility lints:")
            .with_stdout_contains(FUTURE_OUTPUT)
            .with_stdout_does_not_contain("[..]first-dep-0.0.1/src[..]")
            .run();
    }

    // Test that passing the correct id via '--id' doesn't generate a warning message
    let output = p
        .cargo("check")
        .env("RUSTFLAGS", "-Zfuture-incompat-test")
        .exec_with_output()
        .unwrap();

    // Extract the 'id' from the stdout. We are looking
    // for the id in a line of the form "run `cargo report future-incompatibilities --id yZ7S`"
    // which is generated by Cargo to tell the user what command to run
    // This is just to test that passing the id suppresses the warning mesasge. Any users needing
    // access to the report from a shell script should use the `--future-incompat-report` flag
    let stderr = std::str::from_utf8(&output.stderr).unwrap();

    // Find '--id <ID>' in the output
    let mut iter = stderr.split(' ');
    iter.find(|w| *w == "--id").unwrap();
    let id = iter
        .next()
        .unwrap_or_else(|| panic!("Unexpected output:\n{}", stderr));
    // Strip off the trailing '`' included in the output
    let id: String = id.chars().take_while(|c| *c != '`').collect();

    p.cargo(&format!("report future-incompatibilities --id {}", id))
        .with_stdout_contains("The package `first-dep v0.0.1` currently triggers the following future incompatibility lints:")
        .with_stdout_contains("The package `second-dep v0.0.2` currently triggers the following future incompatibility lints:")
        .run();

    // Test without --id, and also the full output of the report.
    let output = p
        .cargo("report future-incompat")
        .exec_with_output()
        .unwrap();
    let output = std::str::from_utf8(&output.stdout).unwrap();
    assert!(output.starts_with("The following warnings were discovered"));
    let mut lines = output
        .lines()
        // Skip the beginning of the per-package information.
        .skip_while(|line| !line.starts_with("The package"));
    for expected in &["first-dep v0.0.1", "second-dep v0.0.2"] {
        assert_eq!(
            &format!(
                "The package `{}` currently triggers the following future incompatibility lints:",
                expected
            ),
            lines.next().unwrap(),
            "Bad output:\n{}",
            output
        );
        let mut count = 0;
        while let Some(line) = lines.next() {
            if line.is_empty() {
                break;
            }
            count += 1;
        }
        assert!(count > 0);
    }
    assert_eq!(lines.next(), None);
}

#[cargo_test(
    nightly,
    reason = "-Zfuture-incompat-test requires nightly (permanently)"
)]
fn color() {
    let p = simple_project();

    p.cargo("check")
        .env("RUSTFLAGS", "-Zfuture-incompat-test")
        .masquerade_as_nightly_cargo(&["future-incompat-test"])
        .run();

    p.cargo("report future-incompatibilities")
        .with_stdout_does_not_contain("[..]\x1b[[..]")
        .run();

    p.cargo("report future-incompatibilities")
        .env("CARGO_TERM_COLOR", "always")
        .with_stdout_contains("[..]\x1b[[..]")
        .run();
}

#[cargo_test(
    nightly,
    reason = "-Zfuture-incompat-test requires nightly (permanently)"
)]
fn bad_ids() {
    let p = simple_project();

    p.cargo("report future-incompatibilities --id 1")
        .with_status(101)
        .with_stderr("error: no reports are currently available")
        .run();

    p.cargo("check")
        .env("RUSTFLAGS", "-Zfuture-incompat-test")
        .masquerade_as_nightly_cargo(&["future-incompat-test"])
        .run();

    p.cargo("report future-incompatibilities --id foo")
        .with_status(1)
        .with_stderr("error: Invalid value: could not parse `foo` as a number")
        .run();

    p.cargo("report future-incompatibilities --id 7")
        .with_status(101)
        .with_stderr(
            "\
error: could not find report with ID 7
Available IDs are: 1
",
        )
        .run();
}

#[cargo_test(
    nightly,
    reason = "-Zfuture-incompat-test requires nightly (permanently)"
)]
fn suggestions_for_updates() {
    Package::new("with_updates", "1.0.0")
        .file("src/lib.rs", FUTURE_EXAMPLE)
        .publish();
    Package::new("big_update", "1.0.0")
        .file("src/lib.rs", FUTURE_EXAMPLE)
        .publish();
    Package::new("without_updates", "1.0.0")
        .file("src/lib.rs", FUTURE_EXAMPLE)
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                with_updates = "1"
                big_update = "1"
                without_updates = "1"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("generate-lockfile").run();

    Package::new("with_updates", "1.0.1")
        .file("src/lib.rs", "")
        .publish();
    Package::new("with_updates", "1.0.2")
        .file("src/lib.rs", "")
        .publish();
    Package::new("with_updates", "3.0.1")
        .file("src/lib.rs", "")
        .publish();
    Package::new("big_update", "2.0.0")
        .file("src/lib.rs", "")
        .publish();

    // This is a hack to force cargo to update the index. Cargo can't do this
    // automatically because doing a network update on every build would be a
    // bad idea. Under normal circumstances, we'll hope the user has done
    // something else along the way to trigger an update (building some other
    // project or something). This could use some more consideration of how to
    // handle this better (maybe only trigger an update if it hasn't updated
    // in a long while?).
    p.cargo("update without_updates").run();

    let update_message = "\
- Some affected dependencies have newer versions available.
You may want to consider updating them to a newer version to see if the issue has been fixed.

big_update v1.0.0 has the following newer versions available: 2.0.0
with_updates v1.0.0 has the following newer versions available: 1.0.1, 1.0.2, 3.0.1
";

    p.cargo("check --future-incompat-report")
        .masquerade_as_nightly_cargo(&["future-incompat-test"])
        .env("RUSTFLAGS", "-Zfuture-incompat-test")
        .with_stderr_contains(update_message)
        .run();

    p.cargo("report future-incompatibilities")
        .with_stdout_contains(update_message)
        .run()
}
