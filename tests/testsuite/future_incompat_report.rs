//! Tests for future-incompat-report messages

use cargo_test_support::registry::Package;
use cargo_test_support::{basic_manifest, is_nightly, project};

#[cargo_test]
fn no_output_on_stable() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/main.rs", "fn main() { [true].into_iter(); }")
        .build();

    p.cargo("build")
        .with_stderr_contains("  = note: `#[warn(array_into_iter)]` on by default")
        .with_stderr_does_not_contain("[..]crates[..]")
        .run();
}

#[cargo_test]
fn gate_future_incompat_report() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/main.rs", "fn main() { [true].into_iter(); }")
        .build();

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

    p.cargo("describe-future-incompatibilities --id foo")
        .with_stderr_contains(
            "error: `cargo describe-future-incompatibilities` can only be used on the nightly channel"
        )
        .with_status(101)
        .run();
}

#[cargo_test]
fn test_single_crate() {
    if !is_nightly() {
        return;
    }

    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/main.rs", "fn main() { [true].into_iter(); }")
        .build();

    for command in &["build", "check", "rustc", "test"] {
        p.cargo(command).arg("-Zfuture-incompat-report")
            .masquerade_as_nightly_cargo()
            .with_stderr_contains("  = note: `#[warn(array_into_iter)]` on by default")
            .with_stderr_contains("warning: the following crates contain code that will be rejected by a future version of Rust: foo v0.0.0 [..]")
            .with_stderr_does_not_contain("[..]incompatibility[..]")
            .run();

        p.cargo(command).arg("-Zfuture-incompat-report").arg("-Zunstable-options").arg("--future-incompat-report")
            .masquerade_as_nightly_cargo()
            .with_stderr_contains("  = note: `#[warn(array_into_iter)]` on by default")
            .with_stderr_contains("warning: the following crates contain code that will be rejected by a future version of Rust: foo v0.0.0 [..]")
            .with_stderr_contains("The crate `foo v0.0.0 ([..])` currently triggers the following future incompatibility lints:")
            .run();
    }
}

#[cargo_test]
fn test_multi_crate() {
    if !is_nightly() {
        return;
    }

    Package::new("first-dep", "0.0.1")
        .file("src/lib.rs", "fn foo() { [25].into_iter(); }")
        .publish();
    Package::new("second-dep", "0.0.2")
        .file("src/lib.rs", "fn foo() { ['a'].into_iter(); }")
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
            .with_stderr_does_not_contain("[..]array_into_iter[..]")
            .with_stderr_contains("warning: the following crates contain code that will be rejected by a future version of Rust: first-dep v0.0.1, second-dep v0.0.2")
            // Check that we don't have the 'triggers' message shown at the bottom of this loop
            .with_stderr_does_not_contain("[..]triggers[..]")
            .run();

        p.cargo("describe-future-incompatibilities -Z future-incompat-report --id bad-id")
            .masquerade_as_nightly_cargo()
            .with_stderr_contains("error: Expected an id of [..]")
            .with_stderr_does_not_contain("[..]triggers[..]")
            .with_status(101)
            .run();

        p.cargo(command).arg("-Zunstable-options").arg("-Zfuture-incompat-report").arg("--future-incompat-report")
            .masquerade_as_nightly_cargo()
            .with_stderr_contains("warning: the following crates contain code that will be rejected by a future version of Rust: first-dep v0.0.1, second-dep v0.0.2")
            .with_stderr_contains("The crate `first-dep v0.0.1` currently triggers the following future incompatibility lints:")
            .with_stderr_contains("The crate `second-dep v0.0.2` currently triggers the following future incompatibility lints:")
            .run();
    }

    // Test that passing the correct id via '--id' doesn't generate a warning message
    let output = p
        .cargo("build -Z future-incompat-report")
        .masquerade_as_nightly_cargo()
        .exec_with_output()
        .unwrap();

    // Extract the 'id' from the stdout. We are looking
    // for the id in a line of the form "run `cargo describe-future-incompatibilities --id yZ7S`"
    // which is generated by Cargo to tell the user what command to run
    // This is just to test that passing the id suppresses the warning mesasge. Any users needing
    // access to the report from a shell script should use the `--future-incompat-report` flag
    let stderr = std::str::from_utf8(&output.stderr).unwrap();

    // Find '--id <ID>' in the output
    let mut iter = stderr.split(" ");
    iter.find(|w| *w == "--id").unwrap();
    let id = iter
        .next()
        .unwrap_or_else(|| panic!("Unexpected output:\n{}", stderr));
    // Strip off the trailing '`' included in the output
    let id: String = id.chars().take_while(|c| *c != '`').collect();

    p.cargo(&format!("describe-future-incompatibilities -Z future-incompat-report --id {}", id))
        .masquerade_as_nightly_cargo()
        .with_stderr_contains("The crate `first-dep v0.0.1` currently triggers the following future incompatibility lints:")
        .with_stderr_contains("The crate `second-dep v0.0.2` currently triggers the following future incompatibility lints:")
        .run();
}
