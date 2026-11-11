//! Tests for flags given without a value.
//!
//! This is a CLI parsing error that
//! in `clap::Arg::num_args(0..=1)` on flag occurrence may take zero values,
//! but we forgot to check whether the value is empty at some cases.
//!
//! The reason of those taking zero values is mostly that we want to list available options.
//!
//! Note that any flag that take zero values, e.g, `clap::Arg::num_args(0..)`,
//! may also have the same issue.

use crate::prelude::*;

use cargo_test_support::project;
use cargo_test_support::registry;
use cargo_test_support::str;

#[cargo_test]
fn repeated_occurrence_without_a_value() {
    let registry = registry::init();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                license = "MIT"
                description = "foo"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("examples/a.rs", "fn main() {}")
        .file("tests/test1.rs", "")
        .file("benches/bench1.rs", "")
        .build();

    p.cargo("check -p foo -p")
        .with_stderr_data(str![[r#"
[WARNING] `--package` without a value is ignored
 |
 = [NOTE] this was previously accepted but will become a hard error in the future
 = [HELP] pass a value to `--package` or remove it
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check --bin foo --bin")
        .with_stderr_data(str![[r#"
[WARNING] `--bin` without a value is ignored
 |
 = [NOTE] this was previously accepted but will become a hard error in the future
 = [HELP] pass a value to `--bin` or remove it
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check --example a --example")
        .with_stderr_data(str![[r#"
[WARNING] `--example` without a value is ignored
 |
 = [NOTE] this was previously accepted but will become a hard error in the future
 = [HELP] pass a value to `--example` or remove it
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check --test test1 --test")
        .with_stderr_data(str![[r#"
[WARNING] `--test` without a value is ignored
 |
 = [NOTE] this was previously accepted but will become a hard error in the future
 = [HELP] pass a value to `--test` or remove it
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check --bench bench1 --bench")
        .with_stderr_data(str![[r#"
[WARNING] `--bench` without a value is ignored
 |
 = [NOTE] this was previously accepted but will become a hard error in the future
 = [HELP] pass a value to `--bench` or remove it
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check --target host-tuple --target")
        .with_stderr_data(str![[r#"
[WARNING] `--target` without a value is ignored
 |
 = [NOTE] this was previously accepted but will become a hard error in the future
 = [HELP] pass a value to `--target` or remove it
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("clean -p foo -p")
        .with_stderr_data(str![[r#"
[WARNING] `--package` without a value is ignored
 |
 = [NOTE] this was previously accepted but will become a hard error in the future
 = [HELP] pass a value to `--package` or remove it
[REMOVED] [FILE_NUM] files, [FILE_SIZE]B total

"#]])
        .run();

    p.cargo("update -p foo -p")
        .with_stderr_data(str![[r#"
[WARNING] `--package` without a value is ignored
 |
 = [NOTE] this was previously accepted but will become a hard error in the future
 = [HELP] pass a value to `--package` or remove it
[LOCKING] 0 packages to highest compatible versions

"#]])
        .run();

    p.cargo("uninstall -p foo -p")
        .with_status(101)
        .with_stderr_data(str![[r#"
[WARNING] `--package` without a value is ignored
 |
 = [NOTE] this was previously accepted but will become a hard error in the future
 = [HELP] pass a value to `--package` or remove it
[ERROR] package ID specification `foo` did not match any packages

"#]])
        .run();

    p.cargo("tree -p foo -p")
        .with_stdout_data(str![[r#"
foo v0.0.1 ([ROOT]/foo)

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `--package` without a value is ignored
 |
 = [NOTE] this was previously accepted but will become a hard error in the future
 = [HELP] pass a value to `--package` or remove it

"#]])
        .run();

    p.cargo("package --no-verify -p foo -p")
        .with_stderr_data(str![[r#"
[WARNING] `--package` without a value is ignored
 |
 = [NOTE] this was previously accepted but will become a hard error in the future
 = [HELP] pass a value to `--package` or remove it
[WARNING] manifest has no documentation, homepage or repository
 |
 = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 7 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)

"#]])
        .run();

    p.cargo("publish --no-verify --dry-run -p foo -p")
        .arg("--index")
        .arg(registry.index_url().as_str())
        .with_stderr_data(str![[r#"
[WARNING] `--package` without a value is ignored
 |
 = [NOTE] this was previously accepted but will become a hard error in the future
 = [HELP] pass a value to `--package` or remove it
[UPDATING] `[ROOT]/registry` index
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 7 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[UPLOADING] foo v0.0.1 ([ROOT]/foo)
[WARNING] aborting upload due to dry run

"#]])
        .run();

    p.cargo("tree -i foo -i")
        .with_stdout_data(str![[r#"
foo v0.0.1 ([ROOT]/foo)

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `--invert` without a value is ignored
 |
 = [NOTE] this was previously accepted but will become a hard error in the future
 = [HELP] pass a value to `--invert` or remove it

"#]])
        .run();
}

#[cargo_test]
fn repeated_occurrence_without_a_value_and_one_next_arg() {
    let p = project().file("src/main.rs", "fn main() {}").build();
    let config = "--config=build.rustc-wrapper='does-not-exist'";

    p.cargo("check -p foo -p")
        .arg(config)
        .with_status(101)
        .with_stderr_data(str![[r#"
[WARNING] `--package` without a value is ignored
 |
 = [NOTE] this was previously accepted but will become a hard error in the future
 = [HELP] pass a value to `--package` or remove it
[ERROR] could not execute process `does-not-exist rustc -vV` (never executed)

Caused by:
  [NOT_FOUND]

"#]])
        .run();

    p.cargo("check -p")
        .arg(config)
        .arg("-p")
        .arg("foo")
        .with_status(101)
        .with_stderr_data(str![[r#"
[WARNING] `--package` without a value is ignored
 |
 = [NOTE] this was previously accepted but will become a hard error in the future
 = [HELP] pass a value to `--package` or remove it
[ERROR] could not execute process `does-not-exist rustc -vV` (never executed)

Caused by:
  [NOT_FOUND]

"#]])
        .run();
}
