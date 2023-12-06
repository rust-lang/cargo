use std::collections::HashMap;
use std::fmt;

use anyhow::{bail, Context as _};
use semver::Version;
use serde::{de, ser};
use url::Url;

use crate::core::GitReference;
use crate::core::PackageId;
use crate::core::SourceKind;
use crate::util::edit_distance;
use crate::util::errors::CargoResult;
use crate::util::{validate_package_name, IntoUrl};
use crate::util_semver::PartialVersion;

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
    /// use cargo::core::PackageIdSpec;
    ///
    /// let specs = vec![
    ///     "https://crates.io/foo",
    ///     "https://crates.io/foo#1.2.3",
    ///     "https://crates.io/foo#bar:1.2.3",
    ///     "https://crates.io/foo#bar@1.2.3",
    ///     "foo",
    ///     "foo:1.2.3",
    ///     "foo@1.2.3",
    /// ];
    /// for spec in specs {
    ///     assert!(PackageIdSpec::parse(spec).is_ok());
    /// }
    pub fn parse(spec: &str) -> CargoResult<PackageIdSpec> {
        if spec.contains("://") {
            if let Ok(url) = spec.into_url() {
                return PackageIdSpec::from_url(url);
            }
        } else if spec.contains('/') || spec.contains('\\') {
            let abs = std::env::current_dir().unwrap_or_default().join(spec);
            if abs.exists() {
                let maybe_url = Url::from_file_path(abs)
                    .map_or_else(|_| "a file:// URL".to_string(), |url| url.to_string());
                bail!(
                    "package ID specification `{}` looks like a file path, \
                    maybe try {}",
                    spec,
                    maybe_url
                );
            }
        }
        let mut parts = spec.splitn(2, [':', '@']);
        let name = parts.next().unwrap();
        let version = match parts.next() {
            Some(version) => Some(version.parse::<PartialVersion>()?),
            None => None,
        };
        validate_package_name(name, "pkgid", "")?;
        Ok(PackageIdSpec {
            name: String::from(name),
            version,
            url: None,
            kind: None,
        })
    }

    /// Tries to convert a valid `Url` to a `PackageIdSpec`.
    fn from_url(mut url: Url) -> CargoResult<PackageIdSpec> {
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
                        bail!("cannot have a query string in a pkgid: {url}")
                    }
                    kind = Some(SourceKind::Registry);
                    url = strip_url_protocol(&url);
                }
                "sparse" => {
                    if url.query().is_some() {
                        bail!("cannot have a query string in a pkgid: {url}")
                    }
                    kind = Some(SourceKind::SparseRegistry);
                    // Leave `sparse` as part of URL, see `SourceId::new`
                    // url = strip_url_protocol(&url);
                }
                "path" => {
                    if url.query().is_some() {
                        bail!("cannot have a query string in a pkgid: {url}")
                    }
                    if scheme != "file" {
                        anyhow::bail!("`path+{scheme}` is unsupported; `path+file` and `file` schemes are supported");
                    }
                    kind = Some(SourceKind::Path);
                    url = strip_url_protocol(&url);
                }
                kind => anyhow::bail!("unsupported source protocol: {kind}"),
            }
        } else {
            if url.query().is_some() {
                bail!("cannot have a query string in a pkgid: {url}")
            }
        }

        let frag = url.fragment().map(|s| s.to_owned());
        url.set_fragment(None);

        let (name, version) = {
            let mut path = url
                .path_segments()
                .ok_or_else(|| anyhow::format_err!("pkgid urls must have a path: {}", url))?;
            let path_name = path.next_back().ok_or_else(|| {
                anyhow::format_err!(
                    "pkgid urls must have at least one path \
                     component: {}",
                    url
                )
            })?;
            match frag {
                Some(fragment) => match fragment.split_once([':', '@']) {
                    Some((name, part)) => {
                        let version = part.parse::<PartialVersion>()?;
                        (String::from(name), Some(version))
                    }
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
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        self.to_string().serialize(s)
    }
}

impl<'de> de::Deserialize<'de> for PackageIdSpec {
    fn deserialize<D>(d: D) -> Result<PackageIdSpec, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let string = String::deserialize(d)?;
        PackageIdSpec::parse(&string).map_err(de::Error::custom)
    }
}

pub trait PackageIdSpecQuery {
    /// Roughly equivalent to `PackageIdSpec::parse(spec)?.query(i)`
    fn query_str<I>(spec: &str, i: I) -> CargoResult<PackageId>
    where
        I: IntoIterator<Item = PackageId>;

    /// Checks whether the given `PackageId` matches the `PackageIdSpec`.
    fn matches(&self, package_id: PackageId) -> bool;

    /// Checks a list of `PackageId`s to find 1 that matches this `PackageIdSpec`. If 0, 2, or
    /// more are found, then this returns an error.
    fn query<I>(&self, i: I) -> CargoResult<PackageId>
    where
        I: IntoIterator<Item = PackageId>;
}

impl PackageIdSpecQuery for PackageIdSpec {
    fn query_str<I>(spec: &str, i: I) -> CargoResult<PackageId>
    where
        I: IntoIterator<Item = PackageId>,
    {
        let i: Vec<_> = i.into_iter().collect();
        let spec = PackageIdSpec::parse(spec).with_context(|| {
            let suggestion = edit_distance::closest_msg(spec, i.iter(), |id| id.name().as_str());
            format!("invalid package ID specification: `{}`{}", spec, suggestion)
        })?;
        spec.query(i)
    }

    fn matches(&self, package_id: PackageId) -> bool {
        if self.name() != package_id.name().as_str() {
            return false;
        }

        if let Some(ref v) = self.version {
            if !v.matches(package_id.version()) {
                return false;
            }
        }

        if let Some(u) = &self.url {
            if u != package_id.source_id().url() {
                return false;
            }
        }

        if let Some(k) = &self.kind {
            if k != package_id.source_id().kind() {
                return false;
            }
        }

        true
    }

    fn query<I>(&self, i: I) -> CargoResult<PackageId>
    where
        I: IntoIterator<Item = PackageId>,
    {
        let all_ids: Vec<_> = i.into_iter().collect();
        let mut ids = all_ids.iter().copied().filter(|&id| self.matches(id));
        let Some(ret) = ids.next() else {
            let mut suggestion = String::new();
            let try_spec = |spec: PackageIdSpec, suggestion: &mut String| {
                let try_matches: Vec<_> = all_ids
                    .iter()
                    .copied()
                    .filter(|&id| spec.matches(id))
                    .collect();
                if !try_matches.is_empty() {
                    suggestion.push_str("\nDid you mean one of these?\n");
                    minimize(suggestion, &try_matches, self);
                }
            };
            if self.url.is_some() {
                let spec = PackageIdSpec::new(self.name.clone());
                let spec = if let Some(version) = self.version.clone() {
                    spec.with_version(version)
                } else {
                    spec
                };
                try_spec(spec, &mut suggestion);
            }
            if suggestion.is_empty() && self.version.is_some() {
                try_spec(PackageIdSpec::new(self.name.clone()), &mut suggestion);
            }
            if suggestion.is_empty() {
                suggestion.push_str(&edit_distance::closest_msg(
                    &self.name,
                    all_ids.iter(),
                    |id| id.name().as_str(),
                ));
            }

            bail!(
                "package ID specification `{}` did not match any packages{}",
                self,
                suggestion
            );
        };
        return match ids.next() {
            Some(other) => {
                let mut msg = format!(
                    "There are multiple `{}` packages in \
                     your project, and the specification \
                     `{}` is ambiguous.\n\
                     Please re-run this command \
                     with one of the following \
                     specifications:",
                    self.name(),
                    self
                );
                let mut vec = vec![ret, other];
                vec.extend(ids);
                minimize(&mut msg, &vec, self);
                Err(anyhow::format_err!("{}", msg))
            }
            None => Ok(ret),
        };

        fn minimize(msg: &mut String, ids: &[PackageId], spec: &PackageIdSpec) {
            let mut version_cnt = HashMap::new();
            for id in ids {
                *version_cnt.entry(id.version()).or_insert(0) += 1;
            }
            for id in ids {
                if version_cnt[id.version()] == 1 {
                    msg.push_str(&format!("\n  {}@{}", spec.name(), id.version()));
                } else {
                    msg.push_str(&format!("\n  {}", id.to_spec()));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PackageIdSpec;
    use super::PackageIdSpecQuery;
    use crate::core::{GitReference, PackageId, SourceId, SourceKind};
    use url::Url;

    #[test]
    fn good_parsing() {
        #[track_caller]
        fn ok(spec: &str, expected: PackageIdSpec, expected_rendered: &str) {
            let parsed = PackageIdSpec::parse(spec).unwrap();
            assert_eq!(parsed, expected);
            let rendered = parsed.to_string();
            assert_eq!(rendered, expected_rendered);
            let reparsed = PackageIdSpec::parse(&rendered).unwrap();
            assert_eq!(reparsed, expected);
        }

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
    }

    #[test]
    fn bad_parsing() {
        assert!(PackageIdSpec::parse("baz:").is_err());
        assert!(PackageIdSpec::parse("baz:*").is_err());
        assert!(PackageIdSpec::parse("baz@").is_err());
        assert!(PackageIdSpec::parse("baz@*").is_err());
        assert!(PackageIdSpec::parse("baz@^1.0").is_err());
        assert!(PackageIdSpec::parse("https://baz:1.0").is_err());
        assert!(PackageIdSpec::parse("https://#baz:1.0").is_err());
        assert!(
            PackageIdSpec::parse("foobar+https://github.com/rust-lang/crates.io-index").is_err()
        );
        assert!(PackageIdSpec::parse("path+https://github.com/rust-lang/crates.io-index").is_err());

        // Only `git+` can use `?`
        assert!(PackageIdSpec::parse("file:///path/to/my/project/foo?branch=dev").is_err());
        assert!(PackageIdSpec::parse("path+file:///path/to/my/project/foo?branch=dev").is_err());
        assert!(PackageIdSpec::parse(
            "registry+https://github.com/rust-lang/cargo#0.52.0?branch=dev"
        )
        .is_err());
        assert!(PackageIdSpec::parse(
            "sparse+https://github.com/rust-lang/cargo#0.52.0?branch=dev"
        )
        .is_err());
    }

    #[test]
    fn matching() {
        let url = Url::parse("https://example.com").unwrap();
        let sid = SourceId::for_registry(&url).unwrap();

        let foo = PackageId::try_new("foo", "1.2.3", sid).unwrap();
        assert!(PackageIdSpec::parse("foo").unwrap().matches(foo));
        assert!(!PackageIdSpec::parse("bar").unwrap().matches(foo));
        assert!(PackageIdSpec::parse("foo:1.2.3").unwrap().matches(foo));
        assert!(!PackageIdSpec::parse("foo:1.2.2").unwrap().matches(foo));
        assert!(PackageIdSpec::parse("foo@1.2.3").unwrap().matches(foo));
        assert!(!PackageIdSpec::parse("foo@1.2.2").unwrap().matches(foo));
        assert!(PackageIdSpec::parse("foo@1.2").unwrap().matches(foo));
        assert!(PackageIdSpec::parse("https://example.com#foo@1.2")
            .unwrap()
            .matches(foo));
        assert!(!PackageIdSpec::parse("https://bob.com#foo@1.2")
            .unwrap()
            .matches(foo));
        assert!(PackageIdSpec::parse("registry+https://example.com#foo@1.2")
            .unwrap()
            .matches(foo));
        assert!(!PackageIdSpec::parse("git+https://example.com#foo@1.2")
            .unwrap()
            .matches(foo));

        let meta = PackageId::try_new("meta", "1.2.3+hello", sid).unwrap();
        assert!(PackageIdSpec::parse("meta").unwrap().matches(meta));
        assert!(PackageIdSpec::parse("meta@1").unwrap().matches(meta));
        assert!(PackageIdSpec::parse("meta@1.2").unwrap().matches(meta));
        assert!(PackageIdSpec::parse("meta@1.2.3").unwrap().matches(meta));
        assert!(!PackageIdSpec::parse("meta@1.2.3-alpha.0")
            .unwrap()
            .matches(meta));
        assert!(PackageIdSpec::parse("meta@1.2.3+hello")
            .unwrap()
            .matches(meta));
        assert!(!PackageIdSpec::parse("meta@1.2.3+bye")
            .unwrap()
            .matches(meta));

        let pre = PackageId::try_new("pre", "1.2.3-alpha.0", sid).unwrap();
        assert!(PackageIdSpec::parse("pre").unwrap().matches(pre));
        assert!(!PackageIdSpec::parse("pre@1").unwrap().matches(pre));
        assert!(!PackageIdSpec::parse("pre@1.2").unwrap().matches(pre));
        assert!(!PackageIdSpec::parse("pre@1.2.3").unwrap().matches(pre));
        assert!(PackageIdSpec::parse("pre@1.2.3-alpha.0")
            .unwrap()
            .matches(pre));
        assert!(!PackageIdSpec::parse("pre@1.2.3-alpha.1")
            .unwrap()
            .matches(pre));
        assert!(!PackageIdSpec::parse("pre@1.2.3-beta.0")
            .unwrap()
            .matches(pre));
        assert!(!PackageIdSpec::parse("pre@1.2.3+hello")
            .unwrap()
            .matches(pre));
        assert!(!PackageIdSpec::parse("pre@1.2.3-alpha.0+hello")
            .unwrap()
            .matches(pre));
    }
}
