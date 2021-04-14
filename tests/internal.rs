//! Tests for internal code checks.

#![allow(clippy::all)]

use std::fs;

#[test]
fn check_forbidden_code() {
    // Do not use certain macros, functions, etc.
    if !cargo_util::is_ci() {
        // Only check these on CI, otherwise it could be annoying.
        use std::io::Write;
        writeln!(
            std::io::stderr(),
            "\nSkipping check_forbidden_code test, set CI=1 to enable"
        )
        .unwrap();
        return;
    }
    let root_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for entry in walkdir::WalkDir::new(&root_path)
        .into_iter()
        .filter_entry(|e| e.path() != root_path.join("doc"))
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !entry
            .file_name()
            .to_str()
            .map(|s| s.ends_with(".rs"))
            .unwrap_or(false)
        {
            continue;
        }
        eprintln!("checking {}", path.display());
        let c = fs::read_to_string(path).unwrap();
        for (line_index, line) in c.lines().enumerate() {
            if line.trim().starts_with("//") {
                continue;
            }
            if line_has_print(line) {
                if entry.file_name().to_str().unwrap() == "cargo_new.rs" && line.contains("Hello") {
                    // An exception.
                    continue;
                }
                panic!(
                    "found print macro in {}:{}\n\n{}\n\n\
                    print! macros should not be used in Cargo because they can panic.\n\
                    Use one of the drop_print macros instead.\n\
                    ",
                    path.display(),
                    line_index,
                    line
                );
            }
            if line_has_macro(line, "dbg") {
                panic!(
                    "found dbg! macro in {}:{}\n\n{}\n\n\
                    dbg! should not be used outside of debugging.",
                    path.display(),
                    line_index,
                    line
                );
            }
        }
    }
}

fn line_has_print(line: &str) -> bool {
    line_has_macro(line, "print")
        || line_has_macro(line, "eprint")
        || line_has_macro(line, "println")
        || line_has_macro(line, "eprintln")
}

#[test]
fn line_has_print_works() {
    assert!(line_has_print("print!"));
    assert!(line_has_print("println!"));
    assert!(line_has_print("eprint!"));
    assert!(line_has_print("eprintln!"));
    assert!(line_has_print("(print!(\"hi!\"))"));
    assert!(!line_has_print("print"));
    assert!(!line_has_print("i like to print things"));
    assert!(!line_has_print("drop_print!"));
    assert!(!line_has_print("drop_println!"));
    assert!(!line_has_print("drop_eprint!"));
    assert!(!line_has_print("drop_eprintln!"));
}

fn line_has_macro(line: &str, mac: &str) -> bool {
    for (i, _) in line.match_indices(mac) {
        if line.get(i + mac.len()..i + mac.len() + 1) != Some("!") {
            continue;
        }
        if i == 0 {
            return true;
        }
        // Check for identifier boundary start.
        let prev1 = line.get(i - 1..i).unwrap().chars().next().unwrap();
        if prev1.is_alphanumeric() || prev1 == '_' {
            continue;
        }
        return true;
    }
    false
}
