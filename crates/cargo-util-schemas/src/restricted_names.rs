//! Helpers for validating and checking names like package and crate names.

use anyhow::bail;
use anyhow::Result;

/// Check the base requirements for a package name.
///
/// This can be used for other things than package names, to enforce some
/// level of sanity. Note that package names have other restrictions
/// elsewhere. `cargo new` has a few restrictions, such as checking for
/// reserved names. crates.io has even more restrictions.
pub fn validate_package_name(name: &str, what: &str, help: &str) -> Result<()> {
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

/// Validate dir-names and profile names according to RFC 2678.
pub fn validate_profile_name(name: &str) -> Result<()> {
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

pub fn validate_feature_name(name: &str) -> Result<()> {
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
                "invalid character `{ch}` in feature `{name}`, \
                the first character must be a Unicode XID start character or digit \
                (most letters or `_` or `0` to `9`)",
            );
        }
    }
    for ch in chars {
        if !(unicode_xid::UnicodeXID::is_xid_continue(ch) || ch == '-' || ch == '+' || ch == '.') {
            bail!(
                "invalid character `{ch}` in feature `{name}`, \
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

    #[test]
    fn valid_feature_names() {
        assert!(validate_feature_name("c++17").is_ok());
        assert!(validate_feature_name("128bit").is_ok());
        assert!(validate_feature_name("_foo").is_ok());
        assert!(validate_feature_name("feat-name").is_ok());
        assert!(validate_feature_name("feat_name").is_ok());
        assert!(validate_feature_name("foo.bar").is_ok());

        assert!(validate_feature_name("").is_err());
        assert!(validate_feature_name("+foo").is_err());
        assert!(validate_feature_name("-foo").is_err());
        assert!(validate_feature_name(".foo").is_err());
        assert!(validate_feature_name("dep:bar").is_err());
        assert!(validate_feature_name("foo/bar").is_err());
        assert!(validate_feature_name("foo:bar").is_err());
        assert!(validate_feature_name("foo?").is_err());
        assert!(validate_feature_name("?foo").is_err());
        assert!(validate_feature_name("ⒶⒷⒸ").is_err());
        assert!(validate_feature_name("a¼").is_err());
        assert!(validate_feature_name("").is_err());
    }
}
