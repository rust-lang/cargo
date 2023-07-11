//! ```text
//! NAME
//!         stale-label
//!
//! SYNOPSIS
//!         stale-label
//!
//! DESCRIPTION
//!         Detect stale paths in autolabel definitions in triagebot.toml.
//!         Probably autofix them in the future.
//! ```

use std::fmt::Write as _;
use std::path::PathBuf;
use std::process;
use toml_edit::Document;

fn main() {
    let pkg_root = std::env!("CARGO_MANIFEST_DIR");
    let ws_root = PathBuf::from(format!("{pkg_root}/../.."));
    let path = {
        let path = ws_root.join("triagebot.toml");
        path.canonicalize().unwrap_or(path)
    };

    eprintln!("Checking file {path:?}\n");

    let mut failed = 0;
    let mut passed = 0;

    let toml = std::fs::read_to_string(path).expect("read from file");
    let doc = toml.parse::<Document>().expect("a toml");
    let autolabel = doc["autolabel"].as_table().expect("a toml table");

    for (label, value) in autolabel.iter() {
        let Some(trigger_files) = value.get("trigger_files") else {
            continue;
        };
        let trigger_files = trigger_files.as_array().expect("an array");
        let missing_files: Vec<_> = trigger_files
            .iter()
            // Hey TOML content is strict UTF-8.
            .map(|v| v.as_str().unwrap())
            .filter(|f| {
                // triagebot checks with `starts_with` only.
                // See https://github.com/rust-lang/triagebot/blob/0e4b48ca86ffede9cc70fb1611e658e4d013bce2/src/handlers/autolabel.rs#L45
                let path = ws_root.join(f);
                if path.exists() {
                    return false;
                }
                let Some(mut read_dir) = path.parent().and_then(|p| p.read_dir().ok()) else {
                    return true;
                };
                !read_dir.any(|e| {
                    e.unwrap()
                        .path()
                        .strip_prefix(&ws_root)
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .starts_with(f)
                })
            })
            .collect();

        failed += missing_files.len();
        passed += trigger_files.len() - missing_files.len();

        if missing_files.is_empty() {
            continue;
        }

        let mut msg = String::new();
        writeln!(
            &mut msg,
            "missing files defined in `autolabel.{label}.trigger_files`:"
        )
        .unwrap();
        for f in missing_files.iter() {
            writeln!(&mut msg, "\t {f}").unwrap();
        }
        eprintln!("{msg}");
    }

    let result = if failed == 0 { "ok" } else { "FAILED" };
    eprintln!("test result: {result}. {passed} passed; {failed} failed;");

    if failed > 0 {
        process::exit(1);
    }
}
