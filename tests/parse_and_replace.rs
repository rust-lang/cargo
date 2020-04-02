#![cfg(not(windows))] // TODO: should fix these tests on Windows

use anyhow::{anyhow, ensure, Context, Error};
use log::{debug, info, warn};
use rustfix::apply_suggestions;
use std::collections::HashSet;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use tempdir::TempDir;

mod fixmode {
    pub const EVERYTHING: &str = "yolo";
    pub const EDITION: &str = "edition";
}

mod settings {
    // can be set as env var to debug
    pub const CHECK_JSON: &str = "RUSTFIX_TEST_CHECK_JSON";
    pub const RECORD_JSON: &str = "RUSTFIX_TEST_RECORD_JSON";
    pub const RECORD_FIXED_RUST: &str = "RUSTFIX_TEST_RECORD_FIXED_RUST";
}

fn compile(file: &Path, mode: &str) -> Result<Output, Error> {
    let tmp = TempDir::new("rustfix-tests")?;

    let mut args: Vec<OsString> = vec![
        file.into(),
        "--error-format=pretty-json".into(),
        "-Zunstable-options".into(),
        "--emit=metadata".into(),
        "--crate-name=rustfix_test".into(),
        "--out-dir".into(),
        tmp.path().into(),
    ];

    if mode == fixmode::EDITION {
        args.push("--edition=2018".into());
    }

    let res = duct::cmd(env::var_os("RUSTC").unwrap_or("rustc".into()), &args)
        .env("CLIPPY_DISABLE_DOCS_LINKS", "true")
        .env_remove("RUST_LOG")
        .stdout_capture()
        .stderr_capture()
        .unchecked()
        .run()?;

    Ok(res)
}

fn compile_and_get_json_errors(file: &Path, mode: &str) -> Result<String, Error> {
    let res = compile(file, mode)?;
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

fn compiles_without_errors(file: &Path, mode: &str) -> Result<(), Error> {
    let res = compile(file, mode)?;

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

fn read_file(path: &Path) -> Result<String, Error> {
    use std::io::Read;

    let mut buffer = String::new();
    let mut file = fs::File::open(path)?;
    file.read_to_string(&mut buffer)?;
    Ok(buffer)
}

fn diff(expected: &str, actual: &str) -> String {
    use difference::{Changeset, Difference};
    use std::fmt::Write;

    let mut res = String::new();
    let changeset = Changeset::new(expected.trim(), actual.trim(), "\n");

    let mut different = false;
    for diff in changeset.diffs {
        let (prefix, diff) = match diff {
            Difference::Same(_) => continue,
            Difference::Add(add) => ("+", add),
            Difference::Rem(rem) => ("-", rem),
        };
        if !different {
            write!(
                &mut res,
                "differences found (+ == actual, - == expected):\n"
            )
            .unwrap();
            different = true;
        }
        for diff in diff.lines() {
            writeln!(&mut res, "{} {}", prefix, diff).unwrap();
        }
    }
    if different {
        write!(&mut res, "").unwrap();
    }

    res
}

fn test_rustfix_with_file<P: AsRef<Path>>(file: P, mode: &str) -> Result<(), Error> {
    let file: &Path = file.as_ref();
    let json_file = file.with_extension("json");
    let fixed_file = file.with_extension("fixed.rs");

    let filter_suggestions = if mode == fixmode::EVERYTHING {
        rustfix::Filter::Everything
    } else {
        rustfix::Filter::MachineApplicableOnly
    };

    debug!("next up: {:?}", file);
    let code = read_file(file).context(format!("could not read {}", file.display()))?;
    let errors = compile_and_get_json_errors(file, mode)
        .context(format!("could compile {}", file.display()))?;
    let suggestions =
        rustfix::get_suggestions_from_json(&errors, &HashSet::new(), filter_suggestions)
            .context("could not load suggestions")?;

    if std::env::var(settings::RECORD_JSON).is_ok() {
        use std::io::Write;
        let mut recorded_json = fs::File::create(&file.with_extension("recorded.json")).context(
            format!("could not create recorded.json for {}", file.display()),
        )?;
        recorded_json.write_all(errors.as_bytes())?;
    }

    if std::env::var(settings::CHECK_JSON).is_ok() {
        let expected_json = read_file(&json_file).context(format!(
            "could not load json fixtures for {}",
            file.display()
        ))?;
        let expected_suggestions =
            rustfix::get_suggestions_from_json(&expected_json, &HashSet::new(), filter_suggestions)
                .context("could not load expected suggestions")?;

        ensure!(
            expected_suggestions == suggestions,
            "got unexpected suggestions from clippy:\n{}",
            diff(
                &format!("{:?}", expected_suggestions),
                &format!("{:?}", suggestions)
            )
        );
    }

    let fixed = apply_suggestions(&code, &suggestions)
        .context(format!("could not apply suggestions to {}", file.display()))?;

    if std::env::var(settings::RECORD_FIXED_RUST).is_ok() {
        use std::io::Write;
        let mut recorded_rust = fs::File::create(&file.with_extension("recorded.rs"))?;
        recorded_rust.write_all(fixed.as_bytes())?;
    }

    let expected_fixed =
        read_file(&fixed_file).context(format!("could read fixed file for {}", file.display()))?;
    ensure!(
        fixed.trim() == expected_fixed.trim(),
        "file {} doesn't look fixed:\n{}",
        file.display(),
        diff(fixed.trim(), expected_fixed.trim())
    );

    compiles_without_errors(&fixed_file, mode)?;

    Ok(())
}

fn get_fixture_files(p: &str) -> Result<Vec<PathBuf>, Error> {
    Ok(fs::read_dir(&p)?
        .into_iter()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_file())
        .filter(|p| {
            let x = p.to_string_lossy();
            x.ends_with(".rs") && !x.ends_with(".fixed.rs") && !x.ends_with(".recorded.rs")
        })
        .collect())
}

fn assert_fixtures(dir: &str, mode: &str) {
    let files = get_fixture_files(&dir)
        .context(format!("couldn't load dir `{}`", dir))
        .unwrap();
    let mut failures = 0;

    for file in &files {
        if let Err(err) = test_rustfix_with_file(file, mode) {
            println!("failed: {}", file.display());
            warn!("{:?}", err);
            failures += 1;
        }
        info!("passed: {:?}", file);
    }

    if failures > 0 {
        panic!(
            "{} out of {} fixture asserts failed\n\
             (run with `env RUST_LOG=parse_and_replace=info` to get more details)",
            failures,
            files.len(),
        );
    }
}

#[test]
fn everything() {
    let _ = env_logger::try_init();
    assert_fixtures("./tests/everything", fixmode::EVERYTHING);
}

#[test]
#[ignore = "Requires custom rustc build"]
fn edition() {
    let _ = env_logger::try_init();
    assert_fixtures("./tests/edition", fixmode::EDITION);
}
