//! Helpers for validating and checking names like package and crate names.

use crate::util::CargoResult;
use anyhow::bail;
use std::path::Path;

/// Returns `true` if the name contains non-ASCII characters.
pub fn is_non_ascii_name(name: &str) -> bool {
    name.chars().any(|ch| ch > '\x7f')
}

/// A Rust keyword.
pub fn is_keyword(name: &str) -> bool {
    // See https://doc.rust-lang.org/reference/keywords.html
    [
        "Self", "abstract", "as", "async", "await", "become", "box", "break", "const", "continue",
        "crate", "do", "dyn", "else", "enum", "extern", "false", "final", "fn", "for", "if",
        "impl", "in", "let", "loop", "macro", "match", "mod", "move", "mut", "override", "priv",
        "pub", "ref", "return", "self", "static", "struct", "super", "trait", "true", "try",
        "type", "typeof", "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
    ]
    .contains(&name)
}

/// These names cannot be used on Windows, even with an extension.
pub fn is_windows_reserved(name: &str) -> bool {
    [
        "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
        "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
    ]
    .contains(&name.to_ascii_lowercase().as_str())
}

/// An artifact with this name will conflict with one of Cargo's build directories.
pub fn is_conflicting_artifact_name(name: &str) -> bool {
    ["deps", "examples", "build", "incremental"].contains(&name)
}

/// Check the base requirements for a package name.
///
/// This can be used for other things than package names, to enforce some
/// level of sanity. Note that package names have other restrictions
/// elsewhere. `cargo new` has a few restrictions, such as checking for
/// reserved names. crates.io has even more restrictions.
pub fn validate_package_name(name: &str, what: &str, help: &str) -> CargoResult<()> {
    let mut chars = name.chars();
    if let Some(ch) = chars.next() {
        if ch.is_digit(10) {
            // A specific error for a potentially common case.
            bail!(
                "the name `{}` cannot be used as a {}, \
                the name cannot start with a digit{}",
                name,
                what,
                help
            );
        }
        if !(unicode_xid::UnicodeXID::is_xid_start(ch) || ch == '_') {
            bail!(
                "invalid character `{}` in {}: `{}`, \
                the first character must be a Unicode XID start character \
                (most letters or `_`){}",
                ch,
                what,
                name,
                help
            );
        }
    }
    for ch in chars {
        if !(unicode_xid::UnicodeXID::is_xid_continue(ch) || ch == '-') {
            bail!(
                "invalid character `{}` in {}: `{}`, \
                characters must be Unicode XID characters \
                (numbers, `-`, `_`, or most letters){}",
                ch,
                what,
                name,
                help
            );
        }
    }
    Ok(())
}

/// Check the entire path for names reserved in Windows.
pub fn is_windows_reserved_path(path: &Path) -> bool {
    path.iter()
        .filter_map(|component| component.to_str())
        .any(|component| {
            let stem = component.split('.').next().unwrap();
            is_windows_reserved(stem)
        })
}

/// Returns `true` if the name contains any glob pattern wildcards.
pub fn is_glob_pattern<T: AsRef<str>>(name: T) -> bool {
    name.as_ref().contains(&['*', '?', '[', ']'][..])
}

/// Returns true if names such as aux.* are allowed.
///
/// Traditionally, Windows did not allow a set of file names (see `is_windows_reserved_name`
/// for a list). More recent versions of Windows have relaxed this restriction. This test
/// determines whether we are running in a mode that allows Windows reserved names.
pub fn windows_reserved_names_are_allowed() -> bool {
    #[cfg(windows)]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use std::ptr;
        use winapi::um::fileapi::GetFullPathNameW;

        let test_file_name: Vec<_> = OsStr::new("aux.rs").encode_wide().collect();

        let buffer_length = unsafe {
            GetFullPathNameW(test_file_name.as_ptr(), 0, ptr::null_mut(), ptr::null_mut())
        };

        if buffer_length == 0 {
            // This means the call failed, so we'll conservatively assume reserved names are not allowed.
            return false;
        }

        let mut buffer = vec![0u16; buffer_length as usize];

        let result = unsafe {
            GetFullPathNameW(
                test_file_name.as_ptr(),
                buffer_length,
                buffer.as_mut_ptr(),
                ptr::null_mut(),
            )
        };

        if result == 0 {
            // Once again, conservatively assume reserved names are not allowed if the
            // GetFullPathNameW call failed.
            return false;
        }

        // Under the old rules, a file name like aux.rs would get converted into \\.\aux, so
        // we detect this case by checking if the string starts with \\.\
        //
        // Otherwise, the filename will be something like C:\Users\Foo\Documents\aux.rs
        let prefix: Vec<_> = OsStr::new("\\\\.\\").encode_wide().collect();
        if buffer.starts_with(&prefix) {
            false
        } else {
            true
        }
    }
    #[cfg(not(windows))]
    true
}
