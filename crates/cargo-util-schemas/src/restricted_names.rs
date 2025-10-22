//! Helpers for validating and checking names like package and crate names.

type Result<T> = std::result::Result<T, NameValidationError>;

/// Error validating names in Cargo.
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct NameValidationError(#[from] ErrorKind);

/// Non-public error kind for [`NameValidationError`].
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
enum ErrorKind {
    #[error("{0} cannot be empty")]
    Empty(&'static str),

    #[error("invalid character `{ch}` in {what}: `{name}`, {reason}")]
    InvalidCharacter {
        ch: char,
        what: &'static str,
        name: String,
        reason: &'static str,
    },

    #[error(
        "profile name `{name}` is reserved\n{help}\n\
         See https://doc.rust-lang.org/cargo/reference/profiles.html \
         for more on configuring profiles."
    )]
    ProfileNameReservedKeyword { name: String, help: &'static str },

    #[error("feature named `{0}` is not allowed to start with `dep:`")]
    FeatureNameStartsWithDepColon(String),
}

pub(crate) fn validate_package_name(name: &str) -> Result<()> {
    for part in name.split("::") {
        validate_name(part, "package name")?;
    }
    Ok(())
}

pub(crate) fn validate_registry_name(name: &str) -> Result<()> {
    validate_name(name, "registry name")
}

pub(crate) fn validate_name(name: &str, what: &'static str) -> Result<()> {
    if name.is_empty() {
        return Err(ErrorKind::Empty(what).into());
    }

    let mut chars = name.chars();
    if let Some(ch) = chars.next() {
        if ch.is_digit(10) {
            // A specific error for a potentially common case.
            return Err(ErrorKind::InvalidCharacter {
                ch,
                what,
                name: name.into(),
                reason: "the name cannot start with a digit",
            }
            .into());
        }
        if !(unicode_ident::is_xid_start(ch) || ch == '_') {
            return Err(ErrorKind::InvalidCharacter {
                ch,
                what,
                name: name.into(),
                reason: "the first character must be a Unicode XID start character \
                 (most letters or `_`)",
            }
            .into());
        }
    }
    for ch in chars {
        if !(unicode_ident::is_xid_continue(ch) || ch == '-') {
            return Err(ErrorKind::InvalidCharacter {
                ch,
                what,
                name: name.into(),
                reason: "characters must be Unicode XID characters \
                 (numbers, `-`, `_`, or most letters)",
            }
            .into());
        }
    }
    Ok(())
}

/// Ensure a package name is [valid][validate_package_name]
pub(crate) fn sanitize_package_name(name: &str, placeholder: char) -> String {
    let mut slug = String::new();
    for part in name.split("::") {
        if !slug.is_empty() {
            slug.push_str("::");
        }
        slug.push_str(&sanitize_name(part, placeholder));
    }
    slug
}

pub(crate) fn sanitize_name(name: &str, placeholder: char) -> String {
    let mut slug = String::new();
    let mut chars = name.chars();
    while let Some(ch) = chars.next() {
        if (unicode_ident::is_xid_start(ch) || ch == '_') && !ch.is_digit(10) {
            slug.push(ch);
            break;
        }
    }
    while let Some(ch) = chars.next() {
        if unicode_ident::is_xid_continue(ch) || ch == '-' {
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
pub(crate) fn validate_profile_name(name: &str) -> Result<()> {
    if let Some(ch) = name
        .chars()
        .find(|ch| !ch.is_alphanumeric() && *ch != '_' && *ch != '-')
    {
        return Err(ErrorKind::InvalidCharacter {
            ch,
            what: "profile name",
            name: name.into(),
            reason: "allowed characters are letters, numbers, underscore, and hyphen",
        }
        .into());
    }

    let lower_name = name.to_lowercase();
    if lower_name == "debug" {
        return Err(ErrorKind::ProfileNameReservedKeyword {
            name: name.into(),
            help: "To configure the default development profile, \
                use the name `dev` as in [profile.dev]",
        }
        .into());
    }
    if lower_name == "build-override" {
        return Err(ErrorKind::ProfileNameReservedKeyword {
            name: name.into(),
            help: "To configure build dependency settings, use [profile.dev.build-override] \
                 and [profile.release.build-override]",
        }
        .into());
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
        return Err(ErrorKind::ProfileNameReservedKeyword {
            name: name.into(),
            help: "Please choose a different name.",
        }
        .into());
    }

    Ok(())
}

pub(crate) fn validate_feature_name(name: &str) -> Result<()> {
    let what = "feature name";
    if name.is_empty() {
        return Err(ErrorKind::Empty(what).into());
    }

    if name.starts_with("dep:") {
        return Err(ErrorKind::FeatureNameStartsWithDepColon(name.into()).into());
    }
    if name.contains('/') {
        return Err(ErrorKind::InvalidCharacter {
            ch: '/',
            what,
            name: name.into(),
            reason: "feature name is not allowed to contain slashes",
        }
        .into());
    }
    let mut chars = name.chars();
    if let Some(ch) = chars.next() {
        if !(unicode_ident::is_xid_start(ch) || ch == '_' || ch.is_digit(10)) {
            return Err(ErrorKind::InvalidCharacter {
                ch,
                what,
                name: name.into(),
                reason: "the first character must be a Unicode XID start character or digit \
                 (most letters or `_` or `0` to `9`)",
            }
            .into());
        }
    }
    for ch in chars {
        if !(unicode_ident::is_xid_continue(ch) || ch == '-' || ch == '+' || ch == '.') {
            return Err(ErrorKind::InvalidCharacter {
                ch,
                what,
                name: name.into(),
                reason: "characters must be Unicode XID characters, '-', `+`, or `.` \
                 (numbers, `+`, `-`, `_`, `.`, or most letters)",
            }
            .into());
        }
    }
    Ok(())
}

pub(crate) fn validate_path_base_name(name: &str) -> Result<()> {
    validate_name(name, "path base name")
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
