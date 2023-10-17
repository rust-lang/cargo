//! Test runner for the semver compatibility doc chapter.
//!
//! This extracts all the "rust" annotated code blocks and tests that they
//! either fail or succeed as expected. This also checks that the examples are
//! formatted correctly.
//!
//! An example with the word "MINOR" at the top is expected to successfully
//! build against the before and after. Otherwise it should fail. A comment of
//! "// Error:" will check that the given message appears in the error output.
//!
//! The code block can also include the annotations:
//! - `run-fail`: The test should fail at runtime, not compiletime.
//! - `dont-deny`: By default tests have a `#![deny(warnings)]`. This option
//!   avoids this attribute. Note that `#![allow(unused)]` is always added.

use std::error::Error;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn main() {
    if let Err(e) = doit() {
        eprintln!("error: {}", e);
        std::process::exit(1);
    }
}

const SEPARATOR: &str = "///////////////////////////////////////////////////////////";

fn doit() -> Result<(), Box<dyn Error>> {
    let filename = std::env::args().nth(1).unwrap_or_else(|| {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../src/doc/src/reference/semver.md")
            .to_str()
            .unwrap()
            .to_string()
    });
    let contents = fs::read_to_string(filename)?;
    let mut lines = contents.lines().enumerate();

    loop {
        // Find a rust block.
        let (block_start, run_program, deny_warnings) = loop {
            match lines.next() {
                Some((lineno, line)) => {
                    if line.trim().starts_with("```rust") && !line.contains("skip") {
                        break (
                            lineno + 1,
                            line.contains("run-fail"),
                            !line.contains("dont-deny"),
                        );
                    }
                }
                None => return Ok(()),
            }
        };
        // Read in the code block.
        let mut block = Vec::new();
        loop {
            match lines.next() {
                Some((_, line)) => {
                    if line.trim() == "```" {
                        break;
                    }
                    // Support rustdoc/mdbook hidden lines.
                    let line = line.strip_prefix("# ").unwrap_or(line);
                    if line == "#" {
                        block.push("");
                    } else {
                        block.push(line);
                    }
                }
                None => {
                    return Err(format!(
                        "rust block did not end for example starting on line {}",
                        block_start
                    )
                    .into());
                }
            }
        }
        // Split it into the separate source files.
        let parts: Vec<_> = block.split(|line| line.trim() == SEPARATOR).collect();
        if parts.len() != 4 {
            return Err(format!(
                "expected 4 sections in example starting on line {}, got {}:\n{:?}",
                block_start,
                parts.len(),
                parts
            )
            .into());
        }
        let join = |part: &[&str]| {
            let mut result = String::new();
            result.push_str("#![allow(unused)]\n");
            if deny_warnings {
                result.push_str("#![deny(warnings)]\n");
            }
            result.push_str(&part.join("\n"));
            if !result.ends_with('\n') {
                result.push('\n');
            }
            result
        };
        let expect_success = parts[0][0].contains("MINOR");
        eprintln!("Running test from line {}", block_start);

        let result = run_test(
            join(parts[1]),
            join(parts[2]),
            join(parts[3]),
            expect_success,
            run_program,
        );

        if let Err(e) = result {
            return Err(format!(
                "test failed for example starting on line {}: {}",
                block_start, e
            )
            .into());
        }
    }
}

const CRATE_NAME: &str = "updated_crate";

fn run_test(
    before: String,
    after: String,
    example: String,
    expect_success: bool,
    run_program: bool,
) -> Result<(), Box<dyn Error>> {
    let tempdir = tempfile::TempDir::new()?;
    let before_p = tempdir.path().join("before.rs");
    let after_p = tempdir.path().join("after.rs");
    let example_p = tempdir.path().join("example.rs");

    let check_fn = if run_program {
        run_check
    } else {
        compile_check
    };

    compile_check(before, &before_p, CRATE_NAME, false, true)?;
    check_fn(example.clone(), &example_p, "example", true, true)?;
    compile_check(after, &after_p, CRATE_NAME, false, true)?;
    check_fn(example, &example_p, "example", true, expect_success)?;
    Ok(())
}

fn check_formatting(path: &Path) -> Result<(), Box<dyn Error>> {
    match Command::new("rustfmt")
        .args(&["--edition=2018", "--check"])
        .arg(path)
        .status()
    {
        Ok(status) => {
            if !status.success() {
                return Err(format!("failed to run rustfmt: {}", status).into());
            }
            Ok(())
        }
        Err(e) => Err(format!("failed to run rustfmt: {}", e).into()),
    }
}

fn compile(
    contents: &str,
    path: &Path,
    crate_name: &str,
    extern_path: bool,
) -> Result<Output, Box<dyn Error>> {
    let crate_type = if contents.contains("fn main()") {
        "bin"
    } else {
        "rlib"
    };

    fs::write(path, &contents)?;
    check_formatting(path)?;
    let out_dir = path.parent().unwrap();
    let mut cmd = Command::new("rustc");
    cmd.args(&[
        "--edition=2021",
        "--crate-type",
        crate_type,
        "--crate-name",
        crate_name,
        "--out-dir",
    ]);
    cmd.arg(&out_dir);
    if extern_path {
        let epath = out_dir.join(format!("lib{}.rlib", CRATE_NAME));
        cmd.arg("--extern")
            .arg(format!("{}={}", CRATE_NAME, epath.display()));
    }
    cmd.arg(path);
    cmd.output().map_err(Into::into)
}

fn compile_check(
    mut contents: String,
    path: &Path,
    crate_name: &str,
    extern_path: bool,
    expect_success: bool,
) -> Result<(), Box<dyn Error>> {
    // If the example has an error message, remove it so that it can be
    // compared with the actual output, and also to avoid issues with rustfmt
    // moving it around.
    let expected_error = match contents.find("// Error:") {
        Some(index) => {
            let start = contents[..index].rfind(|ch| ch != ' ').unwrap();
            let end = contents[index..].find('\n').unwrap();
            let error = contents[index + 9..index + end].trim().to_string();
            contents.replace_range(start + 1..index + end, "");
            Some(error)
        }
        None => None,
    };

    let output = compile(&contents, path, crate_name, extern_path)?;

    let stderr = std::str::from_utf8(&output.stderr).unwrap();
    match (output.status.success(), expect_success) {
        (true, true) => Ok(()),
        (true, false) => Err(format!(
            "expected failure, got success {}\n===== Contents:\n{}\n===== Output:\n{}\n",
            path.display(),
            contents,
            stderr
        )
        .into()),
        (false, true) => Err(format!(
            "expected success, got error {}\n===== Contents:\n{}\n===== Output:\n{}\n",
            path.display(),
            contents,
            stderr
        )
        .into()),
        (false, false) => {
            if expected_error.is_none() {
                return Err("failing test should have an \"// Error:\" annotation ".into());
            }
            let expected_error = expected_error.unwrap();
            if !stderr.contains(&expected_error) {
                Err(format!(
                    "expected error message not found in compiler output\nExpected: {}\nGot:\n{}\n",
                    expected_error, stderr
                )
                .into())
            } else {
                Ok(())
            }
        }
    }
}

fn run_check(
    contents: String,
    path: &Path,
    crate_name: &str,
    extern_path: bool,
    expect_success: bool,
) -> Result<(), Box<dyn Error>> {
    let compile_output = compile(&contents, path, crate_name, extern_path)?;

    if !compile_output.status.success() {
        let stderr = std::str::from_utf8(&compile_output.stderr).unwrap();
        return Err(format!(
            "expected success, got error {}\n===== Contents:\n{}\n===== Output:\n{}\n",
            path.display(),
            contents,
            stderr
        )
        .into());
    }

    let binary_path = path.parent().unwrap().join(crate_name);

    let output = Command::new(binary_path).output()?;

    let stderr = std::str::from_utf8(&output.stderr).unwrap();

    match (output.status.success(), expect_success) {
        (true, false) => Err(format!(
            "expected panic, got success {}\n===== Contents:\n{}\n===== Output:\n{}\n",
            path.display(),
            contents,
            stderr
        )
        .into()),
        (false, true) => Err(format!(
            "expected success, got panic {}\n===== Contents:\n{}\n===== Output:\n{}\n",
            path.display(),
            contents,
            stderr,
        )
        .into()),
        (_, _) => Ok(()),
    }
}
