#[macro_use] extern crate duct;
#[macro_use] extern crate pretty_assertions;
extern crate tempdir;
#[macro_use] extern crate log;
extern crate env_logger;
extern crate serde_json;
extern crate rustfix;

use std::fs;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::collections::HashSet;
use tempdir::TempDir;

use rustfix::Replacement;

fn compile_and_get_json_errors(file: &Path) -> Result<String, Box<Error>> {
    let tmp = TempDir::new("rustfix-tests")?;
    let better_call_clippy = cmd!(
        "clippy-driver", "rustc", file,
        "--error-format=json", "--emit=metadata",
        "--out-dir", tmp.path()
    );
    let res = better_call_clippy
        .stdout_capture()
        .stderr_capture()
        .unchecked()
        .run()?;
    let stderr = String::from_utf8(res.stderr)?;

    use std::io::{Error, ErrorKind};
    match res.status.code() {
        Some(0) | Some(1) => Ok(stderr),
        _ => Err(Box::new(Error::new(
            ErrorKind::Other,
            format!("failed with status {:?}: {}", res.status.code(), stderr),
        )))
    }
}

fn read_file(path: &Path) -> Result<String, Box<Error>> {
    use std::io::Read;

    let mut buffer = String::new();
    let mut file = fs::File::open(path)?;
    file.read_to_string(&mut buffer)?;
    Ok(buffer)
}

fn apply_suggestion(file_content: &mut String, suggestion: &Replacement) -> Result<String, Box<Error>> {
    use std::cmp::max;

    let mut new_content = String::new();

    // Add the lines before the section we want to replace
    new_content.push_str(&file_content.lines()
        .take(max(suggestion.snippet.line_range.start.line - 1, 0) as usize)
        .collect::<Vec<_>>()
        .join("\n"));
    new_content.push_str("\n");

    // Parts of line before replacement
    new_content.push_str(&file_content.lines()
        .nth(suggestion.snippet.line_range.start.line - 1)
        .unwrap_or("")
        .chars()
        .take(suggestion.snippet.line_range.start.column - 1)
        .collect::<String>());

    // Insert new content! Finally!
    new_content.push_str(&suggestion.replacement);

    // Parts of line after replacement
    new_content.push_str(&file_content.lines()
        .nth(suggestion.snippet.line_range.end.line - 1)
        .unwrap_or("")
        .chars()
        .skip(suggestion.snippet.line_range.end.column - 1)
        .collect::<String>());

    // Add the lines after the section we want to replace
    new_content.push_str("\n");
    new_content.push_str(&file_content.lines()
        .skip(suggestion.snippet.line_range.end.line as usize)
        .collect::<Vec<_>>()
        .join("\n"));
    new_content.push_str("\n");

    Ok(new_content)
}

fn test_rustfix_with_file<P: AsRef<Path>>(file: P) -> Result<(), Box<Error>> {
    let file: &Path = file.as_ref();
    debug!("{:?}", file);
    let code = read_file(file)?;
    let errors = compile_and_get_json_errors(file)?;

    if std::env::var("RUSTFIX_TEST_RECORD_JSON").is_ok() {
        use std::io::Write;
        let mut recorded_json = fs::File::create(&file.with_extension("recorded.json"))?;
        recorded_json.write_all(errors.as_bytes())?;
    }

    let expected_json = read_file(&file.with_extension("json"))?;

    assert_eq!(
        errors.trim(),
        expected_json.trim(),
        "got unexpected json from clippy"
    );

    let suggestions = rustfix::get_suggestions_from_json(&errors, &HashSet::new());
    let mut fixed = code.clone();

    for sug in suggestions {
        trace!("{:?}", sug);
        for sol in sug.solutions {
            trace!("{:?}", sol);
            for r in sol.replacements {
                info!("replaced.");
                trace!("{:?}", r);
                fixed = apply_suggestion(&mut fixed, &r)?;
            }
        }
    }

    let expected_fixed = read_file(&file.with_extension("fixed.rs"))?;
    assert_eq!(fixed.trim(), expected_fixed.trim(), "file doesn't look fixed");
    Ok(())
}

fn get_fixture_files() -> Result<Vec<PathBuf>, Box<Error>> {
    Ok(fs::read_dir("./tests/fixtures")?
        .into_iter()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_file())
        .filter(|p| {
            let x = p.to_string_lossy();
            x.ends_with(".rs") && !x.ends_with(".fixed.rs")
        })
        .collect())
}

#[test]
fn fixtures() {
    let _ = env_logger::try_init();

    for file in &get_fixture_files().unwrap() {
        test_rustfix_with_file(file).unwrap();
        info!("passed: {:?}", file);
    }
}
