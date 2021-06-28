//! Tests for future-incompat-report messages

use cargo_test_support::registry::Package;
use cargo_test_support::{basic_manifest, is_nightly, project, Project};

// An arbitrary lint (array_into_iter) that triggers a report.
const FUTURE_EXAMPLE: &'static str = "fn main() { [true].into_iter(); }";
// Some text that will be displayed when the lint fires.
const FUTURE_OUTPUT: &'static str = "[..]array_into_iter[..]";

fn simple_project() -> Project {
    project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/main.rs", FUTURE_EXAMPLE)
        .build()
}

#[cargo_test]
fn no_output_on_stable() {
    let p = simple_project();

    p.cargo("build")
        .with_stderr_contains(FUTURE_OUTPUT)
        .with_stderr_does_not_contain("[..]cargo report[..]")
        .run();
}

#[cargo_test]
fn gate_future_incompat_report() {
    let p = simple_project();

    p.cargo("build --future-incompat-report")
        .with_stderr_contains("error: the `--future-incompat-report` flag is unstable[..]")
        .with_status(101)
        .run();

    // Both `-Z future-incompat-report` and `-Z unstable-opts` are required
    p.cargo("build --future-incompat-report -Z future-incompat-report")
        .masquerade_as_nightly_cargo()
        .with_stderr_contains("error: the `--future-incompat-report` flag is unstable[..]")
        .with_status(101)
        .run();

    p.cargo("build --future-incompat-report -Z unstable-options")
        .masquerade_as_nightly_cargo()
        .with_stderr_contains(
            "error: Usage of `--future-incompat-report` requires `-Z future-incompat-report`",
        )
        .with_status(101)
        .run();

    p.cargo("report future-incompatibilities --id foo")
        .with_stderr_contains("error: `cargo report` can only be used on the nightly channel")
        .with_status(101)
        .run();
}

#[cargo_test]
fn test_zero_future_incompat() {
    if !is_nightly() {
        return;
    }

    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/main.rs", "fn main() {}")
        .build();

    // No note if --future-incompat-report is not specified.
    p.cargo("build -Z future-incompat-report")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[COMPILING] foo v0.0.0 [..]
[FINISHED] [..]
",
        )
        .run();

    p.cargo("build --future-incompat-report -Z unstable-options -Z future-incompat-report")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[FINISHED] [..]
note: 0 dependencies had future-incompatible warnings
",
        )
        .run();
}

#[ignore]
#[cargo_test]
#[ignore] // Waiting on https://github.com/rust-lang/rust/pull/86478
fn test_single_crate() {
    if !is_nightly() {
        return;
    }

    let p = simple_project();

    for command in &["build", "check", "rustc", "test"] {
        p.cargo(command).arg("-Zfuture-incompat-report")
            .masquerade_as_nightly_cargo()
            .with_stderr_contains(FUTURE_OUTPUT)
            .with_stderr_contains("warning: the following packages contain code that will be rejected by a future version of Rust: foo v0.0.0 [..]")
            .with_stderr_does_not_contain("[..]incompatibility[..]")
            .run();

        p.cargo(command).arg("-Zfuture-incompat-report").arg("-Zunstable-options").arg("--future-incompat-report")
            .masquerade_as_nightly_cargo()
            .with_stderr_contains(FUTURE_OUTPUT)
            .with_stderr_contains("warning: the following packages contain code that will be rejected by a future version of Rust: foo v0.0.0 [..]")
            .with_stderr_contains("The package `foo v0.0.0 ([..])` currently triggers the following future incompatibility lints:")
            .run();
    }
}

#[ignore]
#[cargo_test]
#[ignore] // Waiting on https://github.com/rust-lang/rust/pull/86478
fn test_multi_crate() {
    if !is_nightly() {
        return;
    }

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
        .file("src/main.rs", "fn main() {}")
        .build();

    for command in &["build", "check", "rustc", "test"] {
        p.cargo(command).arg("-Zfuture-incompat-report")
            .masquerade_as_nightly_cargo()
            .with_stderr_does_not_contain(FUTURE_OUTPUT)
            .with_stderr_contains("warning: the following packages contain code that will be rejected by a future version of Rust: first-dep v0.0.1, second-dep v0.0.2")
            // Check that we don't have the 'triggers' message shown at the bottom of this loop
            .with_stderr_does_not_contain("[..]triggers[..]")
            .run();

        p.cargo(command).arg("-Zunstable-options").arg("-Zfuture-incompat-report").arg("--future-incompat-report")
            .masquerade_as_nightly_cargo()
            .with_stderr_contains("warning: the following packages contain code that will be rejected by a future version of Rust: first-dep v0.0.1, second-dep v0.0.2")
            .with_stderr_contains("The package `first-dep v0.0.1` currently triggers the following future incompatibility lints:")
            .with_stderr_contains("The package `second-dep v0.0.2` currently triggers the following future incompatibility lints:")
            .run();
    }

    // Test that passing the correct id via '--id' doesn't generate a warning message
    let output = p
        .cargo("build -Z future-incompat-report")
        .masquerade_as_nightly_cargo()
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

    p.cargo(&format!("report future-incompatibilities -Z future-incompat-report --id {}", id))
        .masquerade_as_nightly_cargo()
        .with_stdout_contains("The package `first-dep v0.0.1` currently triggers the following future incompatibility lints:")
        .with_stdout_contains("The package `second-dep v0.0.2` currently triggers the following future incompatibility lints:")
        .run();

    // Test without --id, and also the full output of the report.
    let output = p
        .cargo("report future-incompat -Z future-incompat-report")
        .masquerade_as_nightly_cargo()
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
            lines.next().unwrap()
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

#[ignore]
#[cargo_test]
#[ignore] // Waiting on https://github.com/rust-lang/rust/pull/86478
fn color() {
    if !is_nightly() {
        return;
    }

    let p = simple_project();

    p.cargo("check -Zfuture-incompat-report")
        .masquerade_as_nightly_cargo()
        .run();

    p.cargo("report future-incompatibilities -Z future-incompat-report")
        .masquerade_as_nightly_cargo()
        .with_stdout_does_not_contain("[..]\x1b[[..]")
        .run();

    p.cargo("report future-incompatibilities -Z future-incompat-report")
        .masquerade_as_nightly_cargo()
        .env("CARGO_TERM_COLOR", "always")
        .with_stdout_contains("[..]\x1b[[..]")
        .run();
}

#[ignore]
#[cargo_test]
#[ignore] // Waiting on https://github.com/rust-lang/rust/pull/86478
fn bad_ids() {
    if !is_nightly() {
        return;
    }

    let p = simple_project();

    p.cargo("report future-incompatibilities -Z future-incompat-report --id 1")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr("error: no reports are currently available")
        .run();

    p.cargo("check -Zfuture-incompat-report")
        .masquerade_as_nightly_cargo()
        .run();

    p.cargo("report future-incompatibilities -Z future-incompat-report --id foo")
        .masquerade_as_nightly_cargo()
        .with_status(1)
        .with_stderr("error: Invalid value: could not parse `foo` as a number")
        .run();

    p.cargo("report future-incompatibilities -Z future-incompat-report --id 7")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
error: could not find report with ID 7
Available IDs are: 1
",
        )
        .run();
}

#[ignore]
#[cargo_test]
#[ignore] // Waiting on https://github.com/rust-lang/rust/pull/86478
fn suggestions_for_updates() {
    if !is_nightly() {
        return;
    }

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
    p.cargo("update -p without_updates").run();

    p.cargo("check -Zfuture-incompat-report")
        .masquerade_as_nightly_cargo()
        .with_stderr_contains("[..]cargo report future-incompatibilities --id 1[..]")
        .run();

    p.cargo("report future-incompatibilities")
        .masquerade_as_nightly_cargo()
        .with_stdout_contains(
            "\
The following packages appear to have newer versions available.
You may want to consider updating them to a newer version to see if the issue has been fixed.

big_update v1.0.0 has the following newer versions available: 2.0.0
with_updates v1.0.0 has the following newer versions available: 1.0.1, 1.0.2
",
        )
        .run();
}
