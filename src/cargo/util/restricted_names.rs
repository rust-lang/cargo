//! Helpers for validating and checking names like package and crate names.

use crate::core::PackageId;
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
    if name.is_empty() {
        bail!("{what} cannot be empty");
    }

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

/// Ensure a package name is [valid][validate_package_name]
pub fn sanitize_package_name(name: &str, placeholder: char) -> String {
    let mut slug = String::new();
    let mut chars = name.chars();
    while let Some(ch) = chars.next() {
        if (unicode_xid::UnicodeXID::is_xid_start(ch) || ch == '_') && !ch.is_digit(10) {
            slug.push(ch);
            break;
        }
    }
    while let Some(ch) = chars.next() {
        if unicode_xid::UnicodeXID::is_xid_continue(ch) || ch == '-' {
            slug.push(ch);
        } else {
            slug.push(placeholder);
        }
    }
    if slug.is_empty() {
        slug.push_str("package");
    }
    slug
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

/// Validate dir-names and profile names according to RFC 2678.
pub fn validate_profile_name(name: &str) -> CargoResult<()> {
    if let Some(ch) = name
        .chars()
        .find(|ch| !ch.is_alphanumeric() && *ch != '_' && *ch != '-')
    {
        bail!(
            "invalid character `{}` in profile name `{}`\n\
                Allowed characters are letters, numbers, underscore, and hyphen.",
            ch,
            name
        );
    }

    const SEE_DOCS: &str = "See https://doc.rust-lang.org/cargo/reference/profiles.html \
            for more on configuring profiles.";

    let lower_name = name.to_lowercase();
    if lower_name == "debug" {
        bail!(
            "profile name `{}` is reserved\n\
                 To configure the default development profile, use the name `dev` \
                 as in [profile.dev]\n\
                {}",
            name,
            SEE_DOCS
        );
    }
    if lower_name == "build-override" {
        bail!(
            "profile name `{}` is reserved\n\
                 To configure build dependency settings, use [profile.dev.build-override] \
                 and [profile.release.build-override]\n\
                 {}",
            name,
            SEE_DOCS
        );
    }

    // These are some arbitrary reservations. We have no plans to use
    // these, but it seems safer to reserve a few just in case we want to
    // add more built-in profiles in the future. We can also uses special
    // syntax like cargo:foo if needed. But it is unlikely these will ever
    // be used.
    if matches!(
        lower_name.as_str(),
        "build"
            | "check"
            | "clean"
            | "config"
            | "fetch"
            | "fix"
            | "install"
            | "metadata"
            | "package"
            | "publish"
            | "report"
            | "root"
            | "run"
            | "rust"
            | "rustc"
            | "rustdoc"
            | "target"
            | "tmp"
            | "uninstall"
    ) || lower_name.starts_with("cargo")
    {
        bail!(
            "profile name `{}` is reserved\n\
                 Please choose a different name.\n\
                 {}",
            name,
            SEE_DOCS
        );
    }

    Ok(())
}

pub fn validate_feature_name(pkg_id: PackageId, name: &str) -> CargoResult<()> {
    if name.is_empty() {
        bail!("feature name cannot be empty");
    }

    if name.starts_with("dep:") {
        bail!("feature named `{name}` is not allowed to start with `dep:`",);
    }
    if name.contains('/') {
        bail!("feature named `{name}` is not allowed to contain slashes",);
    }
    let mut chars = name.chars();
    if let Some(ch) = chars.next() {
        if !(unicode_xid::UnicodeXID::is_xid_start(ch) || ch == '_' || ch.is_digit(10)) {
            bail!(
                "invalid character `{ch}` in feature `{name}` in package {pkg_id}, \
                the first character must be a Unicode XID start character or digit \
                (most letters or `_` or `0` to `9`)",
            );
        }
    }
    for ch in chars {
        if !(unicode_xid::UnicodeXID::is_xid_continue(ch) || ch == '-' || ch == '+' || ch == '.') {
            bail!(
                "invalid character `{ch}` in feature `{name}` in package {pkg_id}, \
                characters must be Unicode XID characters, '-', `+`, or `.` \
                (numbers, `+`, `-`, `_`, `.`, or most letters)",
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::CRATES_IO_INDEX;
    use crate::util::into_url::IntoUrl;

    use crate::core::SourceId;

    #[test]
    fn valid_feature_names() {
        let loc = CRATES_IO_INDEX.into_url().unwrap();
        let source_id = SourceId::for_registry(&loc).unwrap();
        let pkg_id = PackageId::try_new("foo", "1.0.0", source_id).unwrap();

        assert!(validate_feature_name(pkg_id, "c++17").is_ok());
        assert!(validate_feature_name(pkg_id, "128bit").is_ok());
        assert!(validate_feature_name(pkg_id, "_foo").is_ok());
        assert!(validate_feature_name(pkg_id, "feat-name").is_ok());
        assert!(validate_feature_name(pkg_id, "feat_name").is_ok());
        assert!(validate_feature_name(pkg_id, "foo.bar").is_ok());

        assert!(validate_feature_name(pkg_id, "+foo").is_err());
        assert!(validate_feature_name(pkg_id, "-foo").is_err());
        assert!(validate_feature_name(pkg_id, ".foo").is_err());
        assert!(validate_feature_name(pkg_id, "foo:bar").is_err());
        assert!(validate_feature_name(pkg_id, "foo?").is_err());
        assert!(validate_feature_name(pkg_id, "?foo").is_err());
        assert!(validate_feature_name(pkg_id, "ⒶⒷⒸ").is_err());
        assert!(validate_feature_name(pkg_id, "a¼").is_err());
        assert!(validate_feature_name(pkg_id, "").is_err());
    }
}
