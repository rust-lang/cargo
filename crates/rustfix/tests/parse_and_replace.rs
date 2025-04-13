//! Tests that verify rustfix applies the appropriate changes to a file.
//!
//! This test works by reading a series of `*.rs` files in the
//! `tests/everything` directory. For each `.rs` file, it runs `rustc` to
//! collect JSON diagnostics from the file. It feeds that JSON data into
//! rustfix and applies the recommended suggestions to the `.rs` file. It then
//! compares the result with the corresponding `.fixed.rs` file. If they don't
//! match, then the test fails.
//!
//! There are several debugging environment variables for this test that you can set:
//!
//! - `RUST_LOG=parse_and_replace=debug`: Print debug information.
//! - `RUSTFIX_TEST_BLESS=test-name.rs`: When given the name of a test, this
//!   will overwrite the `.json` and `.fixed.rs` files with the expected
//!   values. This can be used when adding a new test.
//! - `RUSTFIX_TEST_RECORD_JSON=1`:  Records the JSON output to
//!   `*.recorded.json` files. You can then move that to `.json` or whatever
//!   you need.
//! - `RUSTFIX_TEST_RECORD_FIXED_RUST=1`: Records the fixed result to
//!   `*.recorded.rs` files. You can then move that to `.rs` or whatever you
//!   need.

#![allow(clippy::disallowed_methods, clippy::print_stdout, clippy::print_stderr)]

use anyhow::{anyhow, Context, Error};
use rustfix::apply_suggestions;
use std::collections::HashSet;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::tempdir;
use tracing::info;

mod fixmode {
    pub const EVERYTHING: &str = "yolo";
}

mod settings {
    // can be set as env var to debug
    pub const CHECK_JSON: &str = "RUSTFIX_TEST_CHECK_JSON";
    pub const RECORD_JSON: &str = "RUSTFIX_TEST_RECORD_JSON";
    pub const RECORD_FIXED_RUST: &str = "RUSTFIX_TEST_RECORD_FIXED_RUST";
    pub const BLESS: &str = "RUSTFIX_TEST_BLESS";
}

static mut VERSION: (u32, bool) = (0, false);

// Temporarily copy from `cargo_test_macro::version`.
fn version() -> (u32, bool) {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        let output = Command::new("rustc")
            .arg("-V")
            .output()
            .expect("cargo should run");
        let stdout = std::str::from_utf8(&output.stdout).expect("utf8");
        let vers = stdout.split_whitespace().skip(1).next().unwrap();
        let is_nightly = option_env!("CARGO_TEST_DISABLE_NIGHTLY").is_none()
            && (vers.contains("-nightly") || vers.contains("-dev"));
        let minor = vers.split('.').skip(1).next().unwrap().parse().unwrap();
        unsafe { VERSION = (minor, is_nightly) }
    });
    unsafe { VERSION }
}

fn compile(file: &Path) -> Result<Output, Error> {
    let tmp = tempdir()?;

    let args: Vec<OsString> = vec![
        file.into(),
        "--error-format=json".into(),
        "--emit=metadata".into(),
        "--crate-name=rustfix_test".into(),
        "--out-dir".into(),
        tmp.path().into(),
    ];

    let res = Command::new(env::var_os("RUSTC").unwrap_or("rustc".into()))
        .args(&args)
        .env("CLIPPY_DISABLE_DOCS_LINKS", "true")
        .env_remove("RUST_LOG")
        .output()?;

    Ok(res)
}

fn compile_and_get_json_errors(file: &Path) -> Result<String, Error> {
    let res = compile(file)?;
    let stderr = String::from_utf8(res.stderr)?;
    if stderr.contains("is only accepted on the nightly compiler") {
        panic!("rustfix tests require a nightly compiler");
    }

    match res.status.code() {
        Some(0) | Some(1) | Some(101) => Ok(stderr),
        _ => Err(anyhow!(
            "failed with status {:?}: {}",
            res.status.code(),
            stderr
        )),
    }
}

fn compiles_without_errors(file: &Path) -> Result<(), Error> {
    let res = compile(file)?;

    match res.status.code() {
        Some(0) => Ok(()),
        _ => {
            info!(
                "file {:?} failed to compile:\n{}",
                file,
                String::from_utf8(res.stderr)?
            );
            Err(anyhow!(
                "failed with status {:?} (`env RUST_LOG=parse_and_replace=info` for more info)",
                res.status.code(),
            ))
        }
    }
}

fn diff(expected: &str, actual: &str) -> String {
    use similar::{ChangeTag, TextDiff};
    use std::fmt::Write;

    let mut res = String::new();
    let diff = TextDiff::from_lines(expected.trim(), actual.trim());

    let mut different = false;
    for op in diff.ops() {
        for change in diff.iter_changes(op) {
            let prefix = match change.tag() {
                ChangeTag::Equal => continue,
                ChangeTag::Insert => "+",
                ChangeTag::Delete => "-",
            };
            if !different {
                writeln!(&mut res, "differences found (+ == actual, - == expected):").unwrap();
                different = true;
            }
            write!(&mut res, "{} {}", prefix, change.value()).unwrap();
        }
    }
    if different {
        write!(&mut res, "").unwrap();
    }

    res
}

fn test_rustfix_with_file<P: AsRef<Path>>(file: P, mode: &str) {
    let file: &Path = file.as_ref();
    let json_file = file.with_extension("json");
    let fixed_file = file.with_extension("fixed.rs");

    let filter_suggestions = if mode == fixmode::EVERYTHING {
        rustfix::Filter::Everything
    } else {
        rustfix::Filter::MachineApplicableOnly
    };

    let code = fs::read_to_string(file).unwrap();
    let errors = compile_and_get_json_errors(file)
        .with_context(|| format!("could not compile {}", file.display())).unwrap();
    let suggestions =
        rustfix::get_suggestions_from_json(&errors, &HashSet::new(), filter_suggestions)
            .context("could not load suggestions").unwrap();

    if std::env::var(settings::RECORD_JSON).is_ok() {
        fs::write(file.with_extension("recorded.json"), &errors).unwrap();
    }

    if std::env::var(settings::CHECK_JSON).is_ok() {
        let expected_json = fs::read_to_string(&json_file)
            .with_context(|| format!("could not load json fixtures for {}", file.display())).unwrap();
        let expected_suggestions =
            rustfix::get_suggestions_from_json(&expected_json, &HashSet::new(), filter_suggestions)
                .context("could not load expected suggestions").unwrap();

        assert!(
            expected_suggestions == suggestions,
            "got unexpected suggestions from clippy:\n{}",
            diff(
                &format!("{:?}", expected_suggestions),
                &format!("{:?}", suggestions)
            )
        );
    }

    let fixed = apply_suggestions(&code, &suggestions)
        .with_context(|| format!("could not apply suggestions to {}", file.display())).unwrap()
        .replace('\r', "");

    if std::env::var(settings::RECORD_FIXED_RUST).is_ok() {
        fs::write(file.with_extension("recorded.rs"), &fixed).unwrap();
    }

    if let Some(bless_name) = std::env::var_os(settings::BLESS) {
        if bless_name == file.file_name().unwrap() {
            std::fs::write(&json_file, &errors).unwrap();
            std::fs::write(&fixed_file, &fixed).unwrap();
        }
    }

    let expected_fixed = fs::read_to_string(&fixed_file)
        .with_context(|| format!("could read fixed file for {}", file.display())).unwrap()
        .replace('\r', "");
    assert!(
        fixed.trim() == expected_fixed.trim(),
        "file {} doesn't look fixed:\n{}",
        file.display(),
        diff(fixed.trim(), expected_fixed.trim())
    );

    compiles_without_errors(&fixed_file).unwrap();

}

macro_rules! run_test {
    ($name:ident, $file:expr) => {
        #[test]
        #[allow(non_snake_case)]
        fn $name() {
            let (_, nightly) = version();
            if !$file.ends_with(".nightly.rs") || nightly {
                let file = Path::new(concat!("./tests/everything/", $file));
                assert!(file.is_file(), "could not load {}", $file);
                test_rustfix_with_file(file, fixmode::EVERYTHING);
            }
        }
    };
}

run_test! {
    closure_immutable_outer_variable,
    "closure-immutable-outer-variable.rs"
}
run_test! {dedup_suggestions, "dedup-suggestions.rs"}
run_test! {E0178, "E0178.rs"}
run_test! {handle_insert_only, "handle-insert-only.rs"}
run_test! {lt_generic_comp, "lt-generic-comp.rs"}
run_test! {multiple_solutions, "multiple-solutions.nightly.rs"}
run_test! {replace_only_one_char, "replace-only-one-char.rs"}
run_test! {str_lit_type_mismatch, "str-lit-type-mismatch.rs"}
run_test! {use_insert, "use-insert.rs"}
