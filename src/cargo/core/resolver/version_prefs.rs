//! This module implements support for preferring some versions of a package
//! over other versions.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use crate::core::{Dependency, PackageId, Summary};
use crate::util::interning::InternedString;

/// A collection of preferences for particular package versions.
///
/// This is built up with [`Self::prefer_package_id`] and [`Self::prefer_dependency`], then used to sort the set of
/// summaries for a package during resolution via [`Self::sort_summaries`].
///
/// As written, a version is either "preferred" or "not preferred".  Later extensions may
/// introduce more granular preferences.
#[derive(Default)]
pub struct VersionPreferences {
    try_to_use: HashSet<PackageId>,
    prefer_patch_deps: HashMap<InternedString, HashSet<Dependency>>,
}

pub enum VersionOrdering {
    MaximumVersionsFirst,
    MinimumVersionsFirst,
}

impl VersionPreferences {
    /// Indicate that the given package (specified as a [`PackageId`]) should be preferred.
    pub fn prefer_package_id(&mut self, pkg_id: PackageId) {
        self.try_to_use.insert(pkg_id);
    }

    /// Indicate that the given package (specified as a [`Dependency`])  should be preferred.
    pub fn prefer_dependency(&mut self, dep: Dependency) {
        self.prefer_patch_deps
            .entry(dep.package_name())
            .or_insert_with(HashSet::new)
            .insert(dep);
    }

    /// Sort the given vector of summaries in-place, with all summaries presumed to be for
    /// the same package.  Preferred versions appear first in the result, sorted by
    /// `version_ordering`, followed by non-preferred versions sorted the same way.
    pub fn sort_summaries(
        &self,
        summaries: &mut Vec<Summary>,
        version_ordering: VersionOrdering,
        first_version: bool,
    ) {
        let should_prefer = |pkg_id: &PackageId| {
            self.try_to_use.contains(pkg_id)
                || self
                    .prefer_patch_deps
                    .get(&pkg_id.name())
                    .map(|deps| deps.iter().any(|d| d.matches_id(*pkg_id)))
                    .unwrap_or(false)
        };
        summaries.sort_unstable_by(|a, b| {
            let prefer_a = should_prefer(&a.package_id());
            let prefer_b = should_prefer(&b.package_id());
            let previous_cmp = prefer_a.cmp(&prefer_b).reverse();
            match previous_cmp {
                Ordering::Equal => {
                    let cmp = a.version().cmp(b.version());
                    match version_ordering {
                        VersionOrdering::MaximumVersionsFirst => cmp.reverse(),
                        VersionOrdering::MinimumVersionsFirst => cmp,
                    }
                }
                _ => previous_cmp,
            }
        });
        if first_version {
            let _ = summaries.split_off(1);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::core::SourceId;
    use crate::util::RustVersion;
    use std::collections::BTreeMap;

    fn pkgid(name: &str, version: &str) -> PackageId {
        let src_id =
            SourceId::from_url("registry+https://github.com/rust-lang/crates.io-index").unwrap();
        PackageId::new(name, version, src_id).unwrap()
    }

    fn dep(name: &str, version: &str) -> Dependency {
        let src_id =
            SourceId::from_url("registry+https://github.com/rust-lang/crates.io-index").unwrap();
        Dependency::parse(name, Some(version), src_id).unwrap()
    }

    fn summ(name: &str, version: &str) -> Summary {
        let pkg_id = pkgid(name, version);
        let features = BTreeMap::new();
        Summary::new(
            pkg_id,
            Vec::new(),
            &features,
            None::<&String>,
            None::<RustVersion>,
        )
        .unwrap()
    }

    fn describe(summaries: &Vec<Summary>) -> String {
        let strs: Vec<String> = summaries
            .iter()
            .map(|summary| format!("{}/{}", summary.name(), summary.version()))
            .collect();
        strs.join(", ")
    }

    #[test]
    fn test_prefer_package_id() {
        let mut vp = VersionPreferences::default();
        vp.prefer_package_id(pkgid("foo", "1.2.3"));

        let mut summaries = vec![
            summ("foo", "1.2.4"),
            summ("foo", "1.2.3"),
            summ("foo", "1.1.0"),
            summ("foo", "1.0.9"),
        ];

        vp.sort_summaries(&mut summaries, VersionOrdering::MaximumVersionsFirst, false);
        assert_eq!(
            describe(&summaries),
            "foo/1.2.3, foo/1.2.4, foo/1.1.0, foo/1.0.9".to_string()
        );

        vp.sort_summaries(&mut summaries, VersionOrdering::MinimumVersionsFirst, false);
        assert_eq!(
            describe(&summaries),
            "foo/1.2.3, foo/1.0.9, foo/1.1.0, foo/1.2.4".to_string()
        );
    }

    #[test]
    fn test_prefer_dependency() {
        let mut vp = VersionPreferences::default();
        vp.prefer_dependency(dep("foo", "=1.2.3"));

        let mut summaries = vec![
            summ("foo", "1.2.4"),
            summ("foo", "1.2.3"),
            summ("foo", "1.1.0"),
            summ("foo", "1.0.9"),
        ];

        vp.sort_summaries(&mut summaries, VersionOrdering::MaximumVersionsFirst, false);
        assert_eq!(
            describe(&summaries),
            "foo/1.2.3, foo/1.2.4, foo/1.1.0, foo/1.0.9".to_string()
        );

        vp.sort_summaries(&mut summaries, VersionOrdering::MinimumVersionsFirst, false);
        assert_eq!(
            describe(&summaries),
            "foo/1.2.3, foo/1.0.9, foo/1.1.0, foo/1.2.4".to_string()
        );
    }

    #[test]
    fn test_prefer_both() {
        let mut vp = VersionPreferences::default();
        vp.prefer_package_id(pkgid("foo", "1.2.3"));
        vp.prefer_dependency(dep("foo", "=1.1.0"));

        let mut summaries = vec![
            summ("foo", "1.2.4"),
            summ("foo", "1.2.3"),
            summ("foo", "1.1.0"),
            summ("foo", "1.0.9"),
        ];

        vp.sort_summaries(&mut summaries, VersionOrdering::MaximumVersionsFirst, false);
        assert_eq!(
            describe(&summaries),
            "foo/1.2.3, foo/1.1.0, foo/1.2.4, foo/1.0.9".to_string()
        );

        vp.sort_summaries(&mut summaries, VersionOrdering::MinimumVersionsFirst, false);
        assert_eq!(
            describe(&summaries),
            "foo/1.1.0, foo/1.2.3, foo/1.0.9, foo/1.2.4".to_string()
        );
    }
}
