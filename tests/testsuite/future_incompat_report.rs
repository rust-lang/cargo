//! Tests for future-incompat-report messages
//!
//! Note that these tests use the -Zfuture-incompat-test for rustc.
//! This causes rustc to treat *every* lint as future-incompatible.
//! This is done because future-incompatible lints are inherently
//! ephemeral, but we don't want to continually update these tests.
//! So we pick some random lint that will likely always be the same
//! over time.

use crate::prelude::*;
use cargo_test_support::registry::Package;
use cargo_test_support::{Project, basic_manifest, project, str};

use super::config::write_config_toml;

// An arbitrary lint (unused_variables) that triggers a lint.
// We use a special flag to force it to generate a report.
const FUTURE_EXAMPLE: &'static str = "pub fn foo() { let x = 1; }";
// Some text that will be displayed when the lint fires.
const FUTURE_OUTPUT: &'static str = "[..]unused variable[..]";

/// A project with a future-incompat error in the local package.
fn local_project() -> Project {
    project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/lib.rs", FUTURE_EXAMPLE)
        .build()
}

/// A project with a future-incompat error in a dependency.
fn dependency_project() -> Project {
    Package::new("bar", "1.0.0")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "1.0.0"
                edition = "2015"
                repository = "https://example.com/"
            "#,
        )
        .file("src/lib.rs", FUTURE_EXAMPLE)
        .publish();
    project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "1.0.0"
                edition = "2015"

                [dependencies]
                bar = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build()
}

#[cargo_test(
    nightly,
    reason = "-Zfuture-incompat-test requires nightly (permanently)"
)]
fn incompat_in_local_crate() {
    // A simple example where a local crate triggers a future-incompatibility warning.
    let p = local_project();

    p.cargo("check")
        .env("RUSTFLAGS", "-Zfuture-incompat-test")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[WARNING] unused variable: `x`
...

[WARNING] `foo` (lib) generated 1 warning[..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[WARNING] the following packages contain code that will be rejected by a future version of Rust: foo v0.0.0 ([ROOT]/foo)
[NOTE] to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`

"#]])
        .run();

    p.cargo("check --future-incompat-report")
        .env("RUSTFLAGS", "-Zfuture-incompat-test")
        .with_stderr_data(str![[r#"
[WARNING] unused variable: `x`
...

[WARNING] `foo` (lib) generated 1 warning[..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[WARNING] the following packages contain code that will be rejected by a future version of Rust: foo v0.0.0 ([ROOT]/foo)
[NOTE] this report can be shown with `cargo report future-incompatibilities --id 1`

"#]])
    .run();

    p.cargo("report future-incompatibilities --id 1")
        .with_stdout_data(str![[r#"
The following warnings were discovered during the build. These warnings are an
indication that the packages contain code that will become an error in a
future release of Rust. These warnings typically cover changes to close
soundness problems, unintended or undocumented behavior, or critical problems
that cannot be fixed in a backwards-compatible fashion, and are not expected
to be in wide use.

Each warning should contain a link for more information on what the warning
means and how to resolve it.


The package `foo v0.0.0 ([ROOT]/foo)` currently triggers the following future incompatibility lints:
> [WARNING] unused variable: `x`
...

"#]])
        .run();
}

#[cargo_test(
    nightly,
    reason = "-Zfuture-incompat-test requires nightly (permanently)"
)]
fn incompat_in_dependency() {
    // A simple example where a remote dependency triggers a future-incompatibility warning.
    let p = dependency_project();

    p.cargo("check")
        .env("RUSTFLAGS", "-Zfuture-incompat-test")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[CHECKING] bar v1.0.0
[CHECKING] foo v1.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[WARNING] the following packages contain code that will be rejected by a future version of Rust: bar v1.0.0
[NOTE] to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`

"#]])
        .run();

    p.cargo("check --future-incompat-report")
        .env("RUSTFLAGS", "-Zfuture-incompat-test")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[WARNING] the following packages contain code that will be rejected by a future version of Rust: bar v1.0.0
[HELP] ensure the maintainers know of this problem (e.g. creating a bug report if needed)
      or even helping with a fix (e.g. by creating a pull request)
        - bar@1.0.0
        - repository: https://example.com/
        - detailed warning command: `cargo report future-incompatibilities --id 1 --package bar@1.0.0`
[HELP] use your own version of the dependency with the `[patch]` section in `Cargo.toml`
      For more information, see:
      https://doc.rust-lang.org/cargo/reference/overriding-dependencies.html#the-patch-section
[NOTE] this report can be shown with `cargo report future-incompatibilities --id 1`

"#]])
        .run();

    p.cargo("report future-incompatibilities --id 1")
        .with_stdout_data(str![[r#"
The following warnings were discovered during the build. These warnings are an
indication that the packages contain code that will become an error in a
future release of Rust. These warnings typically cover changes to close
soundness problems, unintended or undocumented behavior, or critical problems
that cannot be fixed in a backwards-compatible fashion, and are not expected
to be in wide use.

Each warning should contain a link for more information on what the warning
means and how to resolve it.

to solve this problem, you can try the following approaches:

- ensure the maintainers know of this problem (e.g. creating a bug report if needed)
or even helping with a fix (e.g. by creating a pull request)
  - bar@1.0.0
  - repository: https://example.com/
  - detailed warning command: `cargo report future-incompatibilities --id 1 --package bar@1.0.0`

- use your own version of the dependency with the `[patch]` section in `Cargo.toml`
For more information, see:
https://doc.rust-lang.org/cargo/reference/overriding-dependencies.html#the-patch-section

The package `bar v1.0.0` currently triggers the following future incompatibility lints:
> [WARNING] unused variable: `x`
...

"#]])
        .run();
}

// This feature is stable, and should not be gated
#[cargo_test]
fn no_gate_future_incompat_report() {
    let p = local_project();

    p.cargo("check --future-incompat-report")
        .with_status(0)
        .run();

    p.cargo("report future-incompatibilities --id foo")
        .with_stderr_data(str![[r#"
[ERROR] no reports are currently available

"#]])
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
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check --future-incompat-report")
        .env("RUSTFLAGS", "-Zfuture-incompat-test")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[NOTE] 0 dependencies had future-incompatible warnings

"#]])
        .run();
}

#[cargo_test(
    nightly,
    reason = "-Zfuture-incompat-test requires nightly (permanently)"
)]
fn test_single_crate() {
    let p = local_project();

    for command in &["build", "check", "rustc", "test"] {
        let check_has_future_compat = || {
            p.cargo(command)
                .env("RUSTFLAGS", "-Zfuture-incompat-test")
                .with_stderr_data("\
...
[WARNING] unused variable: `x`
...
[WARNING] the following packages contain code that will be rejected by a future version of Rust: foo v0.0.0 ([ROOT]/foo)
...
")
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
            .with_stderr_data(
                "\
[WARNING] unused variable: `x`
...
",
            )
            .with_stderr_does_not_contain("[..]rejected[..]")
            .with_stderr_does_not_contain("[..]incompatibility[..]")
            .run();

        // Check that passing `--future-incompat-report` overrides `frequency = 'never'`
        p.cargo(command).arg("--future-incompat-report")
            .env("RUSTFLAGS", "-Zfuture-incompat-test")
            .with_stderr_data("\
[WARNING] unused variable: `x`
...
[WARNING] the following packages contain code that will be rejected by a future version of Rust: foo v0.0.0 ([ROOT]/foo)
...
[NOTE] this report can be shown with `cargo report future-incompatibilities --id [..]`
...
")
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
            .with_stderr_data("\
...
[WARNING] the following packages contain code that will be rejected by a future version of Rust: first-dep v0.0.1, second-dep v0.0.2
...
")
            // Check that we don't have the 'triggers' message shown at the bottom of this loop,
            // and that we don't explain how to show a per-package report
            .with_stderr_does_not_contain("[..]triggers[..]")
            .with_stderr_does_not_contain("[..]--package[..]")
            .with_stderr_does_not_contain("[..]-p[..]")
            .run();

        p.cargo(command).arg("--future-incompat-report")
            .env("RUSTFLAGS", "-Zfuture-incompat-test")
            .with_stderr_data("\
...
[WARNING] the following packages contain code that will be rejected by a future version of Rust: first-dep v0.0.1, second-dep v0.0.2
...
        - first-dep@0.0.1
...
        - second-dep@0.0.2
...
")
            .run();

        p.cargo("report future-incompatibilities")
            .arg("--package")
            .arg("first-dep@0.0.1")
            .with_stdout_data(
                "\
...
The package `first-dep v0.0.1` currently triggers the following future incompatibility lints:
> [WARNING] unused variable: `x`
...
",
            )
            .with_stdout_does_not_contain("[..]second-dep-0.0.2/src[..]")
            .run();

        p.cargo("report future-incompatibilities")
            .arg("--package")
            .arg("second-dep@0.0.2")
            .with_stdout_data(
                "\
...
The package `second-dep v0.0.2` currently triggers the following future incompatibility lints:
> [WARNING] unused variable: `x`
...
",
            )
            .with_stdout_does_not_contain("[..]first-dep-0.0.1/src[..]")
            .run();
    }

    // Test that passing the correct id via '--id' doesn't generate a warning message
    let output = p
        .cargo("check")
        .env("RUSTFLAGS", "-Zfuture-incompat-test")
        .run();

    // Extract the 'id' from the stdout. We are looking
    // for the id in a line of the form "run `cargo report future-incompatibilities --id yZ7S`"
    // which is generated by Cargo to tell the user what command to run
    // This is just to test that passing the id suppresses the warning message. Any users needing
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
        .with_stdout_data(str![[r#"
...
The package `first-dep v0.0.1` currently triggers the following future incompatibility lints:
...
The package `second-dep v0.0.2` currently triggers the following future incompatibility lints:
...
"#]])
        .run();

    // Test without --id, and also the full output of the report.
    let output = p.cargo("report future-incompat").run();
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
    let p = local_project();

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
    let p = local_project();

    p.cargo("report future-incompatibilities --id 1")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no reports are currently available

"#]])
        .run();

    p.cargo("check")
        .env("RUSTFLAGS", "-Zfuture-incompat-test")
        .masquerade_as_nightly_cargo(&["future-incompat-test"])
        .run();

    p.cargo("report future-incompatibilities --id foo")
        .with_status(1)
        .with_stderr_data(str![
            "[ERROR] Invalid value: could not parse `foo` as a number"
        ])
        .run();

    p.cargo("report future-incompatibilities --id 7")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] could not find report with ID 7
Available IDs are: 1

"#]])
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
        .dep("with_updates", "1.0.0")
        .publish();
    Package::new("without_updates", "1.0.0")
        .file("src/lib.rs", FUTURE_EXAMPLE)
        .dep("big_update", "1.0.0")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

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
        .dep("with_updates", "1.0.0")
        .publish();

    // This is a hack to force cargo to update the index. Cargo can't do this
    // automatically because doing a network update on every build would be a
    // bad idea. Under normal circumstances, we'll hope the user has done
    // something else along the way to trigger an update (building some other
    // project or something). This could use some more consideration of how to
    // handle this better (maybe only trigger an update if it hasn't updated
    // in a long while?).
    p.cargo("update without_updates").run();

    p.cargo("check --future-incompat-report")
        .masquerade_as_nightly_cargo(&["future-incompat-test"])
        .env("RUSTFLAGS", "-Zfuture-incompat-test")
        .with_stderr_data(str![[r#"
[DOWNLOADING] crates ...
[DOWNLOADED] without_updates v1.0.0 (registry `dummy-registry`)
[DOWNLOADED] with_updates v1.0.0 (registry `dummy-registry`)
[DOWNLOADED] big_update v1.0.0 (registry `dummy-registry`)
[CHECKING] with_updates v1.0.0
[CHECKING] big_update v1.0.0
[CHECKING] without_updates v1.0.0
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[WARNING] the following packages contain code that will be rejected by a future version of Rust: big_update v1.0.0, with_updates v1.0.0, without_updates v1.0.0
[HELP] update to a newer version to see if the issue has been fixed
        - big_update v1.0.0 has the following newer versions available: 2.0.0
        - with_updates v1.0.0 has the following newer versions available: 1.0.1, 1.0.2, 3.0.1
[HELP] ensure the maintainers know of this problem (e.g. creating a bug report if needed)
      or even helping with a fix (e.g. by creating a pull request)
        - big_update@1.0.0
        - repository: <not found>
        - detailed warning command: `cargo report future-incompatibilities --id 1 --package big_update@1.0.0`
      
        - with_updates@1.0.0
        - repository: <not found>
        - detailed warning command: `cargo report future-incompatibilities --id 1 --package with_updates@1.0.0`
      
        - without_updates@1.0.0
        - repository: <not found>
        - detailed warning command: `cargo report future-incompatibilities --id 1 --package without_updates@1.0.0`
[HELP] use your own version of the dependency with the `[patch]` section in `Cargo.toml`
      For more information, see:
      https://doc.rust-lang.org/cargo/reference/overriding-dependencies.html#the-patch-section
[NOTE] this report can be shown with `cargo report future-incompatibilities --id 1`

"#]])
        .run();

    p.cargo("report future-incompatibilities")
        .with_stdout_data(str![[r#"
The following warnings were discovered during the build. These warnings are an
indication that the packages contain code that will become an error in a
future release of Rust. These warnings typically cover changes to close
soundness problems, unintended or undocumented behavior, or critical problems
that cannot be fixed in a backwards-compatible fashion, and are not expected
to be in wide use.

Each warning should contain a link for more information on what the warning
means and how to resolve it.

to solve this problem, you can try the following approaches:

- update to a newer version to see if the issue has been fixed
  - big_update v1.0.0 has the following newer versions available: 2.0.0
  - with_updates v1.0.0 has the following newer versions available: 1.0.1, 1.0.2, 3.0.1

- ensure the maintainers know of this problem (e.g. creating a bug report if needed)
or even helping with a fix (e.g. by creating a pull request)
  - big_update@1.0.0
  - repository: <not found>
  - detailed warning command: `cargo report future-incompatibilities --id 1 --package big_update@1.0.0`

  - with_updates@1.0.0
  - repository: <not found>
  - detailed warning command: `cargo report future-incompatibilities --id 1 --package with_updates@1.0.0`

  - without_updates@1.0.0
  - repository: <not found>
  - detailed warning command: `cargo report future-incompatibilities --id 1 --package without_updates@1.0.0`

- use your own version of the dependency with the `[patch]` section in `Cargo.toml`
For more information, see:
https://doc.rust-lang.org/cargo/reference/overriding-dependencies.html#the-patch-section

The package `big_update v1.0.0` currently triggers the following future incompatibility lints:
> [WARNING] unused variable: `x`
...

The package `with_updates v1.0.0` currently triggers the following future incompatibility lints:
> [WARNING] unused variable: `x`
...

The package `without_updates v1.0.0` currently triggers the following future incompatibility lints:
> [WARNING] unused variable: `x`
...

"#]])
        .run();
}

#[cargo_test(
    nightly,
    reason = "-Zfuture-incompat-test requires nightly (permanently)"
)]
fn correct_report_id_when_cached() {
    // Checks for a bug where the `--id` value was off-by-one when the report
    // is already cached.
    let p = dependency_project();

    p.cargo("check --future-incompat-report")
        .env("RUSTFLAGS", "-Zfuture-incompat-test")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[CHECKING] bar v1.0.0
[CHECKING] foo v1.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[WARNING] the following packages contain code that will be rejected by a future version of Rust: bar v1.0.0
[HELP] ensure the maintainers know of this problem (e.g. creating a bug report if needed)
      or even helping with a fix (e.g. by creating a pull request)
        - bar@1.0.0
        - repository: https://example.com/
        - detailed warning command: `cargo report future-incompatibilities --id 1 --package bar@1.0.0`
[HELP] use your own version of the dependency with the `[patch]` section in `Cargo.toml`
      For more information, see:
      https://doc.rust-lang.org/cargo/reference/overriding-dependencies.html#the-patch-section
[NOTE] this report can be shown with `cargo report future-incompatibilities --id 1`

"#]])
        .run();

    p.cargo("check --future-incompat-report")
        .env("RUSTFLAGS", "-Zfuture-incompat-test")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[WARNING] the following packages contain code that will be rejected by a future version of Rust: bar v1.0.0
[HELP] ensure the maintainers know of this problem (e.g. creating a bug report if needed)
      or even helping with a fix (e.g. by creating a pull request)
        - bar@1.0.0
        - repository: https://example.com/
        - detailed warning command: `cargo report future-incompatibilities --id 1 --package bar@1.0.0`
[HELP] use your own version of the dependency with the `[patch]` section in `Cargo.toml`
      For more information, see:
      https://doc.rust-lang.org/cargo/reference/overriding-dependencies.html#the-patch-section
[NOTE] this report can be shown with `cargo report future-incompatibilities --id 1`

"#]])
        .run();
}
