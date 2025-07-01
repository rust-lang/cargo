use std::fmt;

use semver::Version;
use serde::{de, ser};
use url::Url;

use crate::core::GitReference;
use crate::core::PartialVersion;
use crate::core::PartialVersionError;
use crate::core::SourceKind;
use crate::manifest::PackageName;
use crate::restricted_names::NameValidationError;

type Result<T> = std::result::Result<T, PackageIdSpecError>;

/// Some or all of the data required to identify a package:
///
///  1. the package name (a `String`, required)
///  2. the package version (a `Version`, optional)
///  3. the package source (a `Url`, optional)
///
/// If any of the optional fields are omitted, then the package ID may be ambiguous, there may be
/// more than one package/version/url combo that will match. However, often just the name is
/// sufficient to uniquely define a package ID.
#[derive(Clone, PartialEq, Eq, Debug, Hash, Ord, PartialOrd)]
pub struct PackageIdSpec {
    name: String,
    version: Option<PartialVersion>,
    url: Option<Url>,
    kind: Option<SourceKind>,
}

impl PackageIdSpec {
    pub fn new(name: String) -> Self {
        Self {
            name,
            version: None,
            url: None,
            kind: None,
        }
    }

    pub fn with_version(mut self, version: PartialVersion) -> Self {
        self.version = Some(version);
        self
    }

    pub fn with_url(mut self, url: Url) -> Self {
        self.url = Some(url);
        self
    }

    pub fn with_kind(mut self, kind: SourceKind) -> Self {
        self.kind = Some(kind);
        self
    }

    /// Parses a spec string and returns a `PackageIdSpec` if the string was valid.
    ///
    /// # Examples
    /// Some examples of valid strings
    ///
    /// ```
    /// use cargo_util_schemas::core::PackageIdSpec;
    ///
    /// let specs = vec![
    ///     "foo",
    ///     "foo@1.4",
    ///     "foo@1.4.3",
    ///     "foo:1.2.3",
    ///     "https://github.com/rust-lang/crates.io-index#foo",
    ///     "https://github.com/rust-lang/crates.io-index#foo@1.4.3",
    ///     "ssh://git@github.com/rust-lang/foo.git#foo@1.4.3",
    ///     "file:///path/to/my/project/foo",
    ///     "file:///path/to/my/project/foo#1.1.8"
    /// ];
    /// for spec in specs {
    ///     assert!(PackageIdSpec::parse(spec).is_ok());
    /// }
    pub fn parse(spec: &str) -> Result<PackageIdSpec> {
        if spec.contains("://") {
            if let Ok(url) = Url::parse(spec) {
                return PackageIdSpec::from_url(url);
            }
        } else if spec.contains('/') || spec.contains('\\') {
            let abs = std::env::current_dir().unwrap_or_default().join(spec);
            if abs.exists() {
                let maybe_url = Url::from_file_path(abs)
                    .map_or_else(|_| "a file:// URL".to_string(), |url| url.to_string());
                return Err(ErrorKind::MaybeFilePath {
                    spec: spec.into(),
                    maybe_url,
                }
                .into());
            }
        }
        let (name, version) = parse_spec(spec)?.unwrap_or_else(|| (spec.to_owned(), None));
        PackageName::new(&name)?;
        Ok(PackageIdSpec {
            name: String::from(name),
            version,
            url: None,
            kind: None,
        })
    }

    /// Tries to convert a valid `Url` to a `PackageIdSpec`.
    fn from_url(mut url: Url) -> Result<PackageIdSpec> {
        let mut kind = None;
        if let Some((kind_str, scheme)) = url.scheme().split_once('+') {
            match kind_str {
                "git" => {
                    let git_ref = GitReference::from_query(url.query_pairs());
                    url.set_query(None);
                    kind = Some(SourceKind::Git(git_ref));
                    url = strip_url_protocol(&url);
                }
                "registry" => {
                    if url.query().is_some() {
                        return Err(ErrorKind::UnexpectedQueryString(url).into());
                    }
                    kind = Some(SourceKind::Registry);
                    url = strip_url_protocol(&url);
                }
                "sparse" => {
                    if url.query().is_some() {
                        return Err(ErrorKind::UnexpectedQueryString(url).into());
                    }
                    kind = Some(SourceKind::SparseRegistry);
                    // Leave `sparse` as part of URL, see `SourceId::new`
                    // url = strip_url_protocol(&url);
                }
                "path" => {
                    if url.query().is_some() {
                        return Err(ErrorKind::UnexpectedQueryString(url).into());
                    }
                    if scheme != "file" {
                        return Err(ErrorKind::UnsupportedPathPlusScheme(scheme.into()).into());
                    }
                    kind = Some(SourceKind::Path);
                    url = strip_url_protocol(&url);
                }
                kind => return Err(ErrorKind::UnsupportedProtocol(kind.into()).into()),
            }
        } else {
            if url.query().is_some() {
                return Err(ErrorKind::UnexpectedQueryString(url).into());
            }
        }

        let frag = url.fragment().map(|s| s.to_owned());
        url.set_fragment(None);

        let (name, version) = {
            let Some(path_name) = url.path_segments().and_then(|mut p| p.next_back()) else {
                return Err(ErrorKind::MissingUrlPath(url).into());
            };
            match frag {
                Some(fragment) => match parse_spec(&fragment)? {
                    Some((name, ver)) => (name, ver),
                    None => {
                        if fragment.chars().next().unwrap().is_alphabetic() {
                            (String::from(fragment.as_str()), None)
                        } else {
                            let version = fragment.parse::<PartialVersion>()?;
                            (String::from(path_name), Some(version))
                        }
                    }
                },
                None => (String::from(path_name), None),
            }
        };
        PackageName::new(&name)?;
        Ok(PackageIdSpec {
            name,
            version,
            url: Some(url),
            kind,
        })
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    /// Full `semver::Version`, if present
    pub fn version(&self) -> Option<Version> {
        self.version.as_ref().and_then(|v| v.to_version())
    }

    pub fn partial_version(&self) -> Option<&PartialVersion> {
        self.version.as_ref()
    }

    pub fn url(&self) -> Option<&Url> {
        self.url.as_ref()
    }

    pub fn set_url(&mut self, url: Url) {
        self.url = Some(url);
    }

    pub fn kind(&self) -> Option<&SourceKind> {
        self.kind.as_ref()
    }

    pub fn set_kind(&mut self, kind: SourceKind) {
        self.kind = Some(kind);
    }
}

fn parse_spec(spec: &str) -> Result<Option<(String, Option<PartialVersion>)>> {
    let Some((name, ver)) = spec
        .rsplit_once('@')
        .or_else(|| spec.rsplit_once(':').filter(|(n, _)| !n.ends_with(':')))
    else {
        return Ok(None);
    };
    let name = name.to_owned();
    let ver = ver.parse::<PartialVersion>()?;
    Ok(Some((name, Some(ver))))
}

fn strip_url_protocol(url: &Url) -> Url {
    // Ridiculous hoop because `Url::set_scheme` errors when changing to http/https
    let raw = url.to_string();
    raw.split_once('+').unwrap().1.parse().unwrap()
}

impl fmt::Display for PackageIdSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut printed_name = false;
        match self.url {
            Some(ref url) => {
                if let Some(protocol) = self.kind.as_ref().and_then(|k| k.protocol()) {
                    write!(f, "{protocol}+")?;
                }
                write!(f, "{}", url)?;
                if let Some(SourceKind::Git(git_ref)) = self.kind.as_ref() {
                    if let Some(pretty) = git_ref.pretty_ref(true) {
                        write!(f, "?{}", pretty)?;
                    }
                }
                if url.path_segments().unwrap().next_back().unwrap() != &*self.name {
                    printed_name = true;
                    write!(f, "#{}", self.name)?;
                }
            }
            None => {
                printed_name = true;
                write!(f, "{}", self.name)?;
            }
        }
        if let Some(ref v) = self.version {
            write!(f, "{}{}", if printed_name { "@" } else { "#" }, v)?;
        }
        Ok(())
    }
}

impl ser::Serialize for PackageIdSpec {
    fn serialize<S>(&self, s: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        self.to_string().serialize(s)
    }
}

impl<'de> de::Deserialize<'de> for PackageIdSpec {
    fn deserialize<D>(d: D) -> std::result::Result<PackageIdSpec, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let string = String::deserialize(d)?;
        PackageIdSpec::parse(&string).map_err(de::Error::custom)
    }
}

#[cfg(feature = "unstable-schema")]
impl schemars::JsonSchema for PackageIdSpec {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "PackageIdSpec".into()
    }
    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        <String as schemars::JsonSchema>::json_schema(generator)
    }
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct PackageIdSpecError(#[from] ErrorKind);

impl From<PartialVersionError> for PackageIdSpecError {
    fn from(value: PartialVersionError) -> Self {
        ErrorKind::PartialVersion(value).into()
    }
}

impl From<NameValidationError> for PackageIdSpecError {
    fn from(value: NameValidationError) -> Self {
        ErrorKind::NameValidation(value).into()
    }
}

/// Non-public error kind for [`PackageIdSpecError`].
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
enum ErrorKind {
    #[error("unsupported source protocol: {0}")]
    UnsupportedProtocol(String),

    #[error("`path+{0}` is unsupported; `path+file` and `file` schemes are supported")]
    UnsupportedPathPlusScheme(String),

    #[error("cannot have a query string in a pkgid: {0}")]
    UnexpectedQueryString(Url),

    #[error("pkgid urls must have at least one path component: {0}")]
    MissingUrlPath(Url),

    #[error("package ID specification `{spec}` looks like a file path, maybe try {maybe_url}")]
    MaybeFilePath { spec: String, maybe_url: String },

    #[error(transparent)]
    NameValidation(#[from] crate::restricted_names::NameValidationError),

    #[error(transparent)]
    PartialVersion(#[from] crate::core::PartialVersionError),
}

#[cfg(test)]
mod tests {
    use super::ErrorKind;
    use super::PackageIdSpec;
    use crate::core::{GitReference, SourceKind};
    use url::Url;

    #[track_caller]
    fn ok(spec: &str, expected: PackageIdSpec, expected_rendered: &str) {
        let parsed = PackageIdSpec::parse(spec).unwrap();
        assert_eq!(parsed, expected);
        let rendered = parsed.to_string();
        assert_eq!(rendered, expected_rendered);
        let reparsed = PackageIdSpec::parse(&rendered).unwrap();
        assert_eq!(reparsed, expected);
    }

    macro_rules! err {
        ($spec:expr, $expected:pat) => {
            let err = PackageIdSpec::parse($spec).unwrap_err();
            let kind = err.0;
            assert!(
                matches!(kind, $expected),
                "`{}` parse error mismatch, got {kind:?}",
                $spec
            );
        };
    }

    #[test]
    fn good_parsing() {
        ok(
            "https://crates.io/foo",
            PackageIdSpec {
                name: String::from("foo"),
                version: None,
                url: Some(Url::parse("https://crates.io/foo").unwrap()),
                kind: None,
            },
            "https://crates.io/foo",
        );
        ok(
            "https://crates.io/foo#1.2.3",
            PackageIdSpec {
                name: String::from("foo"),
                version: Some("1.2.3".parse().unwrap()),
                url: Some(Url::parse("https://crates.io/foo").unwrap()),
                kind: None,
            },
            "https://crates.io/foo#1.2.3",
        );
        ok(
            "https://crates.io/foo#1.2",
            PackageIdSpec {
                name: String::from("foo"),
                version: Some("1.2".parse().unwrap()),
                url: Some(Url::parse("https://crates.io/foo").unwrap()),
                kind: None,
            },
            "https://crates.io/foo#1.2",
        );
        ok(
            "https://crates.io/foo#bar:1.2.3",
            PackageIdSpec {
                name: String::from("bar"),
                version: Some("1.2.3".parse().unwrap()),
                url: Some(Url::parse("https://crates.io/foo").unwrap()),
                kind: None,
            },
            "https://crates.io/foo#bar@1.2.3",
        );
        ok(
            "https://crates.io/foo#bar@1.2.3",
            PackageIdSpec {
                name: String::from("bar"),
                version: Some("1.2.3".parse().unwrap()),
                url: Some(Url::parse("https://crates.io/foo").unwrap()),
                kind: None,
            },
            "https://crates.io/foo#bar@1.2.3",
        );
        ok(
            "https://crates.io/foo#bar@1.2",
            PackageIdSpec {
                name: String::from("bar"),
                version: Some("1.2".parse().unwrap()),
                url: Some(Url::parse("https://crates.io/foo").unwrap()),
                kind: None,
            },
            "https://crates.io/foo#bar@1.2",
        );
        ok(
            "registry+https://crates.io/foo#bar@1.2",
            PackageIdSpec {
                name: String::from("bar"),
                version: Some("1.2".parse().unwrap()),
                url: Some(Url::parse("https://crates.io/foo").unwrap()),
                kind: Some(SourceKind::Registry),
            },
            "registry+https://crates.io/foo#bar@1.2",
        );
        ok(
            "sparse+https://crates.io/foo#bar@1.2",
            PackageIdSpec {
                name: String::from("bar"),
                version: Some("1.2".parse().unwrap()),
                url: Some(Url::parse("sparse+https://crates.io/foo").unwrap()),
                kind: Some(SourceKind::SparseRegistry),
            },
            "sparse+https://crates.io/foo#bar@1.2",
        );
        ok(
            "foo",
            PackageIdSpec {
                name: String::from("foo"),
                version: None,
                url: None,
                kind: None,
            },
            "foo",
        );
        ok(
            "foo::bar",
            PackageIdSpec {
                name: String::from("foo::bar"),
                version: None,
                url: None,
                kind: None,
            },
            "foo::bar",
        );
        ok(
            "foo:1.2.3",
            PackageIdSpec {
                name: String::from("foo"),
                version: Some("1.2.3".parse().unwrap()),
                url: None,
                kind: None,
            },
            "foo@1.2.3",
        );
        ok(
            "foo::bar:1.2.3",
            PackageIdSpec {
                name: String::from("foo::bar"),
                version: Some("1.2.3".parse().unwrap()),
                url: None,
                kind: None,
            },
            "foo::bar@1.2.3",
        );
        ok(
            "foo@1.2.3",
            PackageIdSpec {
                name: String::from("foo"),
                version: Some("1.2.3".parse().unwrap()),
                url: None,
                kind: None,
            },
            "foo@1.2.3",
        );
        ok(
            "foo::bar@1.2.3",
            PackageIdSpec {
                name: String::from("foo::bar"),
                version: Some("1.2.3".parse().unwrap()),
                url: None,
                kind: None,
            },
            "foo::bar@1.2.3",
        );
        ok(
            "foo@1.2",
            PackageIdSpec {
                name: String::from("foo"),
                version: Some("1.2".parse().unwrap()),
                url: None,
                kind: None,
            },
            "foo@1.2",
        );

        // pkgid-spec.md
        ok(
            "regex",
            PackageIdSpec {
                name: String::from("regex"),
                version: None,
                url: None,
                kind: None,
            },
            "regex",
        );
        ok(
            "regex@1.4",
            PackageIdSpec {
                name: String::from("regex"),
                version: Some("1.4".parse().unwrap()),
                url: None,
                kind: None,
            },
            "regex@1.4",
        );
        ok(
            "regex@1.4.3",
            PackageIdSpec {
                name: String::from("regex"),
                version: Some("1.4.3".parse().unwrap()),
                url: None,
                kind: None,
            },
            "regex@1.4.3",
        );
        ok(
            "https://github.com/rust-lang/crates.io-index#regex",
            PackageIdSpec {
                name: String::from("regex"),
                version: None,
                url: Some(Url::parse("https://github.com/rust-lang/crates.io-index").unwrap()),
                kind: None,
            },
            "https://github.com/rust-lang/crates.io-index#regex",
        );
        ok(
            "https://github.com/rust-lang/crates.io-index#regex@1.4.3",
            PackageIdSpec {
                name: String::from("regex"),
                version: Some("1.4.3".parse().unwrap()),
                url: Some(Url::parse("https://github.com/rust-lang/crates.io-index").unwrap()),
                kind: None,
            },
            "https://github.com/rust-lang/crates.io-index#regex@1.4.3",
        );
        ok(
            "sparse+https://github.com/rust-lang/crates.io-index#regex@1.4.3",
            PackageIdSpec {
                name: String::from("regex"),
                version: Some("1.4.3".parse().unwrap()),
                url: Some(
                    Url::parse("sparse+https://github.com/rust-lang/crates.io-index").unwrap(),
                ),
                kind: Some(SourceKind::SparseRegistry),
            },
            "sparse+https://github.com/rust-lang/crates.io-index#regex@1.4.3",
        );
        ok(
            "https://github.com/rust-lang/cargo#0.52.0",
            PackageIdSpec {
                name: String::from("cargo"),
                version: Some("0.52.0".parse().unwrap()),
                url: Some(Url::parse("https://github.com/rust-lang/cargo").unwrap()),
                kind: None,
            },
            "https://github.com/rust-lang/cargo#0.52.0",
        );
        ok(
            "https://github.com/rust-lang/cargo#cargo-platform@0.1.2",
            PackageIdSpec {
                name: String::from("cargo-platform"),
                version: Some("0.1.2".parse().unwrap()),
                url: Some(Url::parse("https://github.com/rust-lang/cargo").unwrap()),
                kind: None,
            },
            "https://github.com/rust-lang/cargo#cargo-platform@0.1.2",
        );
        ok(
            "ssh://git@github.com/rust-lang/regex.git#regex@1.4.3",
            PackageIdSpec {
                name: String::from("regex"),
                version: Some("1.4.3".parse().unwrap()),
                url: Some(Url::parse("ssh://git@github.com/rust-lang/regex.git").unwrap()),
                kind: None,
            },
            "ssh://git@github.com/rust-lang/regex.git#regex@1.4.3",
        );
        ok(
            "git+ssh://git@github.com/rust-lang/regex.git#regex@1.4.3",
            PackageIdSpec {
                name: String::from("regex"),
                version: Some("1.4.3".parse().unwrap()),
                url: Some(Url::parse("ssh://git@github.com/rust-lang/regex.git").unwrap()),
                kind: Some(SourceKind::Git(GitReference::DefaultBranch)),
            },
            "git+ssh://git@github.com/rust-lang/regex.git#regex@1.4.3",
        );
        ok(
            "git+ssh://git@github.com/rust-lang/regex.git?branch=dev#regex@1.4.3",
            PackageIdSpec {
                name: String::from("regex"),
                version: Some("1.4.3".parse().unwrap()),
                url: Some(Url::parse("ssh://git@github.com/rust-lang/regex.git").unwrap()),
                kind: Some(SourceKind::Git(GitReference::Branch("dev".to_owned()))),
            },
            "git+ssh://git@github.com/rust-lang/regex.git?branch=dev#regex@1.4.3",
        );
        ok(
            "file:///path/to/my/project/foo",
            PackageIdSpec {
                name: String::from("foo"),
                version: None,
                url: Some(Url::parse("file:///path/to/my/project/foo").unwrap()),
                kind: None,
            },
            "file:///path/to/my/project/foo",
        );
        ok(
            "file:///path/to/my/project/foo::bar",
            PackageIdSpec {
                name: String::from("foo::bar"),
                version: None,
                url: Some(Url::parse("file:///path/to/my/project/foo::bar").unwrap()),
                kind: None,
            },
            "file:///path/to/my/project/foo::bar",
        );
        ok(
            "file:///path/to/my/project/foo#1.1.8",
            PackageIdSpec {
                name: String::from("foo"),
                version: Some("1.1.8".parse().unwrap()),
                url: Some(Url::parse("file:///path/to/my/project/foo").unwrap()),
                kind: None,
            },
            "file:///path/to/my/project/foo#1.1.8",
        );
        ok(
            "path+file:///path/to/my/project/foo#1.1.8",
            PackageIdSpec {
                name: String::from("foo"),
                version: Some("1.1.8".parse().unwrap()),
                url: Some(Url::parse("file:///path/to/my/project/foo").unwrap()),
                kind: Some(SourceKind::Path),
            },
            "path+file:///path/to/my/project/foo#1.1.8",
        );
        ok(
            "path+file:///path/to/my/project/foo#bar",
            PackageIdSpec {
                name: String::from("bar"),
                version: None,
                url: Some(Url::parse("file:///path/to/my/project/foo").unwrap()),
                kind: Some(SourceKind::Path),
            },
            "path+file:///path/to/my/project/foo#bar",
        );
        ok(
            "path+file:///path/to/my/project/foo#foo::bar",
            PackageIdSpec {
                name: String::from("foo::bar"),
                version: None,
                url: Some(Url::parse("file:///path/to/my/project/foo").unwrap()),
                kind: Some(SourceKind::Path),
            },
            "path+file:///path/to/my/project/foo#foo::bar",
        );
        ok(
            "path+file:///path/to/my/project/foo#bar:1.1.8",
            PackageIdSpec {
                name: String::from("bar"),
                version: Some("1.1.8".parse().unwrap()),
                url: Some(Url::parse("file:///path/to/my/project/foo").unwrap()),
                kind: Some(SourceKind::Path),
            },
            "path+file:///path/to/my/project/foo#bar@1.1.8",
        );
        ok(
            "path+file:///path/to/my/project/foo#foo::bar:1.1.8",
            PackageIdSpec {
                name: String::from("foo::bar"),
                version: Some("1.1.8".parse().unwrap()),
                url: Some(Url::parse("file:///path/to/my/project/foo").unwrap()),
                kind: Some(SourceKind::Path),
            },
            "path+file:///path/to/my/project/foo#foo::bar@1.1.8",
        );
        ok(
            "path+file:///path/to/my/project/foo#bar@1.1.8",
            PackageIdSpec {
                name: String::from("bar"),
                version: Some("1.1.8".parse().unwrap()),
                url: Some(Url::parse("file:///path/to/my/project/foo").unwrap()),
                kind: Some(SourceKind::Path),
            },
            "path+file:///path/to/my/project/foo#bar@1.1.8",
        );
        ok(
            "path+file:///path/to/my/project/foo#foo::bar@1.1.8",
            PackageIdSpec {
                name: String::from("foo::bar"),
                version: Some("1.1.8".parse().unwrap()),
                url: Some(Url::parse("file:///path/to/my/project/foo").unwrap()),
                kind: Some(SourceKind::Path),
            },
            "path+file:///path/to/my/project/foo#foo::bar@1.1.8",
        );
    }

    #[test]
    fn bad_parsing() {
        err!("baz:", ErrorKind::PartialVersion(_));
        err!("baz:*", ErrorKind::PartialVersion(_));
        err!("baz@", ErrorKind::PartialVersion(_));
        err!("baz@*", ErrorKind::PartialVersion(_));
        err!("baz@^1.0", ErrorKind::PartialVersion(_));
        err!("https://baz:1.0", ErrorKind::NameValidation(_));
        err!("https://#baz:1.0", ErrorKind::NameValidation(_));
        err!(
            "foobar+https://github.com/rust-lang/crates.io-index",
            ErrorKind::UnsupportedProtocol(_)
        );
        err!(
            "path+https://github.com/rust-lang/crates.io-index",
            ErrorKind::UnsupportedPathPlusScheme(_)
        );

        // Only `git+` can use `?`
        err!(
            "file:///path/to/my/project/foo?branch=dev",
            ErrorKind::UnexpectedQueryString(_)
        );
        err!(
            "path+file:///path/to/my/project/foo?branch=dev",
            ErrorKind::UnexpectedQueryString(_)
        );
        err!(
            "registry+https://github.com/rust-lang/cargo?branch=dev#0.52.0",
            ErrorKind::UnexpectedQueryString(_)
        );
        err!(
            "sparse+https://github.com/rust-lang/cargo?branch=dev#0.52.0",
            ErrorKind::UnexpectedQueryString(_)
        );
        err!("@1.2.3", ErrorKind::NameValidation(_));
        err!("registry+https://github.com", ErrorKind::NameValidation(_));
        err!("https://crates.io/1foo#1.2.3", ErrorKind::NameValidation(_));
    }
}
