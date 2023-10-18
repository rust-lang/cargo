use std::collections::HashMap;
use std::fmt;

use anyhow::{bail, Context as _};
use semver::Version;
use serde::{de, ser};
use url::Url;

use crate::core::PackageId;
use crate::util::edit_distance;
use crate::util::errors::CargoResult;
use crate::util::interning::InternedString;
use crate::util::PartialVersion;
use crate::util::{validate_package_name, IntoUrl};

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
    name: InternedString,
    version: Option<PartialVersion>,
    url: Option<Url>,
}

impl PackageIdSpec {
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
            name: InternedString::new(name),
            version,
            url: None,
        })
    }

    /// Roughly equivalent to `PackageIdSpec::parse(spec)?.query(i)`
    pub fn query_str<I>(spec: &str, i: I) -> CargoResult<PackageId>
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

    /// Convert a `PackageId` to a `PackageIdSpec`, which will have both the `PartialVersion` and `Url`
    /// fields filled in.
    pub fn from_package_id(package_id: PackageId) -> PackageIdSpec {
        PackageIdSpec {
            name: package_id.name(),
            version: Some(package_id.version().clone().into()),
            url: Some(package_id.source_id().url().clone()),
        }
    }

    /// Tries to convert a valid `Url` to a `PackageIdSpec`.
    fn from_url(mut url: Url) -> CargoResult<PackageIdSpec> {
        if url.query().is_some() {
            bail!("cannot have a query string in a pkgid: {}", url)
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
                        (InternedString::new(name), Some(version))
                    }
                    None => {
                        if fragment.chars().next().unwrap().is_alphabetic() {
                            (InternedString::new(&fragment), None)
                        } else {
                            let version = fragment.parse::<PartialVersion>()?;
                            (InternedString::new(path_name), Some(version))
                        }
                    }
                },
                None => (InternedString::new(path_name), None),
            }
        };
        Ok(PackageIdSpec {
            name,
            version,
            url: Some(url),
        })
    }

    pub fn name(&self) -> InternedString {
        self.name
    }

    /// Full `semver::Version`, if present
    pub fn version(&self) -> Option<Version> {
        self.version.as_ref().and_then(|v| v.version())
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

    /// Checks whether the given `PackageId` matches the `PackageIdSpec`.
    pub fn matches(&self, package_id: PackageId) -> bool {
        if self.name() != package_id.name() {
            return false;
        }

        if let Some(ref v) = self.version {
            if !v.matches(package_id.version()) {
                return false;
            }
        }

        match self.url {
            Some(ref u) => u == package_id.source_id().url(),
            None => true,
        }
    }

    /// Checks a list of `PackageId`s to find 1 that matches this `PackageIdSpec`. If 0, 2, or
    /// more are found, then this returns an error.
    pub fn query<I>(&self, i: I) -> CargoResult<PackageId>
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
                try_spec(
                    PackageIdSpec {
                        name: self.name,
                        version: self.version.clone(),
                        url: None,
                    },
                    &mut suggestion,
                );
            }
            if suggestion.is_empty() && self.version.is_some() {
                try_spec(
                    PackageIdSpec {
                        name: self.name,
                        version: None,
                        url: None,
                    },
                    &mut suggestion,
                );
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
                    msg.push_str(&format!("\n  {}", PackageIdSpec::from_package_id(*id)));
                }
            }
        }
    }
}

impl fmt::Display for PackageIdSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut printed_name = false;
        match self.url {
            Some(ref url) => {
                write!(f, "{}", url)?;
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

#[cfg(test)]
mod tests {
    use super::PackageIdSpec;
    use crate::core::{PackageId, SourceId};
    use crate::util::interning::InternedString;
    use url::Url;

    #[test]
    fn good_parsing() {
        #[track_caller]
        fn ok(spec: &str, expected: PackageIdSpec, expected_rendered: &str) {
            let parsed = PackageIdSpec::parse(spec).unwrap();
            assert_eq!(parsed, expected);
            assert_eq!(parsed.to_string(), expected_rendered);
        }

        ok(
            "https://crates.io/foo",
            PackageIdSpec {
                name: InternedString::new("foo"),
                version: None,
                url: Some(Url::parse("https://crates.io/foo").unwrap()),
            },
            "https://crates.io/foo",
        );
        ok(
            "https://crates.io/foo#1.2.3",
            PackageIdSpec {
                name: InternedString::new("foo"),
                version: Some("1.2.3".parse().unwrap()),
                url: Some(Url::parse("https://crates.io/foo").unwrap()),
            },
            "https://crates.io/foo#1.2.3",
        );
        ok(
            "https://crates.io/foo#1.2",
            PackageIdSpec {
                name: InternedString::new("foo"),
                version: Some("1.2".parse().unwrap()),
                url: Some(Url::parse("https://crates.io/foo").unwrap()),
            },
            "https://crates.io/foo#1.2",
        );
        ok(
            "https://crates.io/foo#bar:1.2.3",
            PackageIdSpec {
                name: InternedString::new("bar"),
                version: Some("1.2.3".parse().unwrap()),
                url: Some(Url::parse("https://crates.io/foo").unwrap()),
            },
            "https://crates.io/foo#bar@1.2.3",
        );
        ok(
            "https://crates.io/foo#bar@1.2.3",
            PackageIdSpec {
                name: InternedString::new("bar"),
                version: Some("1.2.3".parse().unwrap()),
                url: Some(Url::parse("https://crates.io/foo").unwrap()),
            },
            "https://crates.io/foo#bar@1.2.3",
        );
        ok(
            "https://crates.io/foo#bar@1.2",
            PackageIdSpec {
                name: InternedString::new("bar"),
                version: Some("1.2".parse().unwrap()),
                url: Some(Url::parse("https://crates.io/foo").unwrap()),
            },
            "https://crates.io/foo#bar@1.2",
        );
        ok(
            "foo",
            PackageIdSpec {
                name: InternedString::new("foo"),
                version: None,
                url: None,
            },
            "foo",
        );
        ok(
            "foo:1.2.3",
            PackageIdSpec {
                name: InternedString::new("foo"),
                version: Some("1.2.3".parse().unwrap()),
                url: None,
            },
            "foo@1.2.3",
        );
        ok(
            "foo@1.2.3",
            PackageIdSpec {
                name: InternedString::new("foo"),
                version: Some("1.2.3".parse().unwrap()),
                url: None,
            },
            "foo@1.2.3",
        );
        ok(
            "foo@1.2",
            PackageIdSpec {
                name: InternedString::new("foo"),
                version: Some("1.2".parse().unwrap()),
                url: None,
            },
            "foo@1.2",
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
    }

    #[test]
    fn matching() {
        let url = Url::parse("https://example.com").unwrap();
        let sid = SourceId::for_registry(&url).unwrap();

        let foo = PackageId::new("foo", "1.2.3", sid).unwrap();
        assert!(PackageIdSpec::parse("foo").unwrap().matches(foo));
        assert!(!PackageIdSpec::parse("bar").unwrap().matches(foo));
        assert!(PackageIdSpec::parse("foo:1.2.3").unwrap().matches(foo));
        assert!(!PackageIdSpec::parse("foo:1.2.2").unwrap().matches(foo));
        assert!(PackageIdSpec::parse("foo@1.2.3").unwrap().matches(foo));
        assert!(!PackageIdSpec::parse("foo@1.2.2").unwrap().matches(foo));
        assert!(PackageIdSpec::parse("foo@1.2").unwrap().matches(foo));

        let meta = PackageId::new("meta", "1.2.3+hello", sid).unwrap();
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

        let pre = PackageId::new("pre", "1.2.3-alpha.0", sid).unwrap();
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
