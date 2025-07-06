use std::collections::HashMap;

use anyhow::{Context as _, bail};

use crate::core::PackageId;
use crate::core::PackageIdSpec;
use crate::util::edit_distance;
use crate::util::errors::CargoResult;

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
            let suggestion =
                edit_distance::closest_msg(spec, i.iter(), |id| id.name().as_str(), "package");
            format!("invalid package ID specification: `{}`{}", spec, suggestion)
        })?;
        spec.query(i)
    }

    fn matches(&self, package_id: PackageId) -> bool {
        if self.name() != package_id.name().as_str() {
            return false;
        }

        if let Some(ref v) = self.partial_version() {
            if !v.matches(package_id.version()) {
                return false;
            }
        }

        if let Some(u) = &self.url() {
            if *u != package_id.source_id().url() {
                return false;
            }
        }

        if let Some(k) = &self.kind() {
            if *k != package_id.source_id().kind() {
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
                    suggestion.push_str("\nhelp: there are similar package ID specifications:\n");
                    minimize(suggestion, &try_matches, self);
                }
            };
            if self.url().is_some() {
                let spec = PackageIdSpec::new(self.name().to_owned());
                let spec = if let Some(version) = self.partial_version().cloned() {
                    spec.with_version(version)
                } else {
                    spec
                };
                try_spec(spec, &mut suggestion);
            }
            if suggestion.is_empty() && self.version().is_some() {
                try_spec(PackageIdSpec::new(self.name().to_owned()), &mut suggestion);
            }
            if suggestion.is_empty() {
                suggestion.push_str(&edit_distance::closest_msg(
                    self.name(),
                    all_ids.iter(),
                    |id| id.name().as_str(),
                    "package",
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
    use crate::core::{PackageId, SourceId};
    use url::Url;

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
        assert!(
            PackageIdSpec::parse("https://example.com#foo@1.2")
                .unwrap()
                .matches(foo)
        );
        assert!(
            !PackageIdSpec::parse("https://bob.com#foo@1.2")
                .unwrap()
                .matches(foo)
        );
        assert!(
            PackageIdSpec::parse("registry+https://example.com#foo@1.2")
                .unwrap()
                .matches(foo)
        );
        assert!(
            !PackageIdSpec::parse("git+https://example.com#foo@1.2")
                .unwrap()
                .matches(foo)
        );

        let meta = PackageId::try_new("meta", "1.2.3+hello", sid).unwrap();
        assert!(PackageIdSpec::parse("meta").unwrap().matches(meta));
        assert!(PackageIdSpec::parse("meta@1").unwrap().matches(meta));
        assert!(PackageIdSpec::parse("meta@1.2").unwrap().matches(meta));
        assert!(PackageIdSpec::parse("meta@1.2.3").unwrap().matches(meta));
        assert!(
            !PackageIdSpec::parse("meta@1.2.3-alpha.0")
                .unwrap()
                .matches(meta)
        );
        assert!(
            PackageIdSpec::parse("meta@1.2.3+hello")
                .unwrap()
                .matches(meta)
        );
        assert!(
            !PackageIdSpec::parse("meta@1.2.3+bye")
                .unwrap()
                .matches(meta)
        );

        let pre = PackageId::try_new("pre", "1.2.3-alpha.0", sid).unwrap();
        assert!(PackageIdSpec::parse("pre").unwrap().matches(pre));
        assert!(!PackageIdSpec::parse("pre@1").unwrap().matches(pre));
        assert!(!PackageIdSpec::parse("pre@1.2").unwrap().matches(pre));
        assert!(!PackageIdSpec::parse("pre@1.2.3").unwrap().matches(pre));
        assert!(
            PackageIdSpec::parse("pre@1.2.3-alpha.0")
                .unwrap()
                .matches(pre)
        );
        assert!(
            !PackageIdSpec::parse("pre@1.2.3-alpha.1")
                .unwrap()
                .matches(pre)
        );
        assert!(
            !PackageIdSpec::parse("pre@1.2.3-beta.0")
                .unwrap()
                .matches(pre)
        );
        assert!(
            !PackageIdSpec::parse("pre@1.2.3+hello")
                .unwrap()
                .matches(pre)
        );
        assert!(
            !PackageIdSpec::parse("pre@1.2.3-alpha.0+hello")
                .unwrap()
                .matches(pre)
        );
    }
}
