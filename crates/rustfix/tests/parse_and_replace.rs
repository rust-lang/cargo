//! Tests that verify rustfix applies the appropriate changes to a file.
//!
//! This test works by reading a series of `*.rs` files in the
//! `tests/everything` directory. For each `.rs` file, it runs `rustc` to
//! collect JSON diagnostics from the file. It feeds that JSON data into
//! rustfix and applies the recommended suggestions to the `.rs` file. It then
//! compares the result with the corresponding `.fixed.rs` file. If they don't
//! match, then the test fails.
//!
//! The files ending in `.nightly.rs` will run only on the nightly toolchain
//!
//! To override snapshots, run `SNAPSHOTS=overwrite cargo test`.
//! See [`snapbox::assert::Action`] for different actions.

#![allow(clippy::disallowed_methods, clippy::print_stdout, clippy::print_stderr)]

use anyhow::{Context, Error, anyhow};
use rustfix::apply_suggestions;
use serde_json::Value;
use snapbox::data::DataFormat;
use snapbox::{Assert, Data};
use std::collections::HashSet;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::tempdir;

mod fixmode {
    pub const EVERYTHING: &str = "yolo";
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
        _ => Err(anyhow!(
            "file {:?} failed compile with status {:?}:\n {}",
            file,
            res.status.code(),
            String::from_utf8(res.stderr)?
        )),
    }
}

fn test_rustfix_with_file<P: AsRef<Path>>(file: P, mode: &str) {
    let file: &Path = file.as_ref();
    let json_file = file.with_extension("json");
    let expected_fixed_file = file.with_extension("fixed.rs");

    let filter_suggestions = if mode == fixmode::EVERYTHING {
        rustfix::Filter::Everything
    } else {
        rustfix::Filter::MachineApplicableOnly
    };

    let code = fs::read_to_string(file).unwrap();

    let json = compile_and_get_json_errors(file)
        .with_context(|| format!("could not compile {}", file.display()))
        .unwrap();

    let suggestions =
        rustfix::get_suggestions_from_json(&json, &HashSet::new(), filter_suggestions)
            .context("could not load suggestions")
            .unwrap();

    let fixed = apply_suggestions(&code, &suggestions)
        .with_context(|| format!("could not apply suggestions to {}", file.display()))
        .unwrap()
        .replace('\r', "");

    let assert = Assert::new().action_env(snapbox::assert::DEFAULT_ACTION_ENV);
    let (actual_fix, expected_fix) = assert.normalize(
        Data::text(&fixed),
        Data::read_from(expected_fixed_file.as_path(), Some(DataFormat::Text)),
    );

    if actual_fix != expected_fix {
        let fixed_assert = assert.try_eq(Some(&"Current Fix"), actual_fix, expected_fix);
        assert!(fixed_assert.is_ok(), "{}", fixed_assert.err().unwrap());

        let expected_json = Data::read_from(json_file.as_path(), Some(DataFormat::Text));

        let pretty_json = json
            .split("\n")
            .filter(|j| !j.is_empty())
            .map(|j| {
                serde_json::to_string_pretty(&serde_json::from_str::<Value>(j).unwrap()).unwrap()
            })
            .collect::<Vec<String>>()
            .join("\n");

        let json_assert = assert.try_eq(
            Some(&"Compiler Error"),
            Data::text(pretty_json),
            expected_json,
        );
        assert!(json_assert.is_ok(), "{}", json_assert.err().unwrap());
    }

    compiles_without_errors(&expected_fixed_file).unwrap();
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
run_test! {multiple_solutions, "multiple-solutions.rs"}
run_test! {replace_only_one_char, "replace-only-one-char.rs"}
run_test! {str_lit_type_mismatch, "str-lit-type-mismatch.rs"}
run_test! {use_insert, "use-insert.rs"}
