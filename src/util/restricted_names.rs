//! Helpers for validating and checking names like package and crate names.

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
    match name.as_bytes() {
        [_, _, _] => {
            name.eq_ignore_ascii_case("con")
                || name.eq_ignore_ascii_case("prn")
                || name.eq_ignore_ascii_case("aux")
                || name.eq_ignore_ascii_case("nul")
        }
        [prefix @ .., b'1'..=b'9'] if prefix.len() == 3 => {
            prefix.eq_ignore_ascii_case(b"com") || prefix.eq_ignore_ascii_case(b"lpt")
        }
        _ => false,
    }
}

/// An artifact with this name will conflict with one of Cargo's build directories.
pub fn is_conflicting_artifact_name(name: &str) -> bool {
    ["deps", "examples", "build", "incremental"].contains(&name)
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

#[cfg(test)]
mod tests {
    use super::{is_windows_reserved, is_windows_reserved_path};
    use std::path::Path;

    const RESERVED: &[&str] = &[
        "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
        "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
    ];

    #[test]
    fn windows_reserved_names_ignore_ascii_case() {
        for name in RESERVED {
            for mask in 0..1 << name.len() {
                let variant: String = name
                    .bytes()
                    .enumerate()
                    .map(|(i, byte)| {
                        if mask & (1 << i) == 0 {
                            byte as char
                        } else {
                            byte.to_ascii_uppercase() as char
                        }
                    })
                    .collect();
                assert!(is_windows_reserved(&variant), "{variant:?}");
                assert!(is_windows_reserved_path(Path::new(&format!(
                    "src/{variant}.rs"
                ))));
                assert!(is_windows_reserved_path(Path::new(&format!(
                    "src/{variant}/mod.rs"
                ))));
            }
        }
    }

    #[test]
    fn windows_reserved_names_require_an_exact_match() {
        for name in [
            "",
            "co",
            "com",
            "lpt",
            "com0",
            "com10",
            "lpt0",
            "lpt10",
            "com:",
            "com/",
            "lpt:",
            "lpt/",
            "xcon",
            "conx",
            "con.rs",
            "nul.tar.gz",
            " con",
            "con ",
            "COM\u{b9}",
            "LPT\u{b2}",
            "c\u{f3}n",
            "\u{e9}1",
            "\u{e9}a1",
            "\u{4e2d}1",
            "\u{1f980}",
            "aux\0",
            "nul\n",
        ] {
            assert!(!is_windows_reserved(name), "{name:?}");
        }
    }

    #[test]
    fn windows_reserved_names_match_the_previous_implementation() {
        // Exercise every ASCII byte in every position around the reserved names,
        // including digit boundaries and names that become another reserved name.
        for name in RESERVED {
            for position in 0..name.len() {
                for byte in 0..=127 {
                    let mut candidate = name.as_bytes().to_vec();
                    candidate[position] = byte;
                    let candidate = std::str::from_utf8(&candidate).unwrap();
                    let expected = RESERVED.contains(&candidate.to_ascii_lowercase().as_str());
                    assert_eq!(is_windows_reserved(candidate), expected, "{candidate:?}");
                }
            }
        }
    }

    #[test]
    fn windows_reserved_paths_check_stems_and_parent_components() {
        for path in [
            "CON",
            "src/NuL.rs",
            "src/COM1.tar.gz",
            "src/Lpt9/mod.rs",
            "aux.txt/normal.rs",
        ] {
            assert!(is_windows_reserved_path(Path::new(path)), "{path:?}");
        }
        for path in [
            "",
            ".",
            "src/lib.rs",
            "src/conifer.rs",
            "src/com0.rs",
            "src/com10.rs",
            "src/.con",
            "src/file.con",
            "src/\u{4e2d}1.rs",
        ] {
            assert!(!is_windows_reserved_path(Path::new(path)), "{path:?}");
        }
    }

    #[test]
    #[cfg(windows)]
    fn windows_reserved_paths_handle_windows_separators_and_non_unicode() {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;

        assert!(is_windows_reserved_path(Path::new(r"C:\src\CoM1.rs")));
        assert!(is_windows_reserved_path(Path::new(r"src\AuX\mod.rs")));
        let non_unicode = OsString::from_wide(&[0xd800]);
        assert!(!is_windows_reserved_path(Path::new(&non_unicode)));
        assert!(is_windows_reserved_path(
            &Path::new(&non_unicode).join("nul.rs")
        ));
    }
}
