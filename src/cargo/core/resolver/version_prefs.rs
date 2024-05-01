//! This module implements support for preferring some versions of a package
//! over other versions.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use cargo_util_schemas::core::PartialVersion;

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
    version_ordering: VersionOrdering,
    max_rust_version: Option<PartialVersion>,
}

#[derive(Copy, Clone, Default, PartialEq, Eq, Hash, Debug)]
pub enum VersionOrdering {
    #[default]
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

    pub fn version_ordering(&mut self, ordering: VersionOrdering) {
        self.version_ordering = ordering;
    }

    pub fn max_rust_version(&mut self, ver: Option<PartialVersion>) {
        self.max_rust_version = ver;
    }

    /// Sort (and filter) the given vector of summaries in-place
    ///
    /// Note: all summaries presumed to be for the same package.
    ///
    /// Sort order:
    /// 1. Preferred packages
    /// 2. [`VersionPreferences::max_rust_version`]
    /// 3. `first_version`, falling back to [`VersionPreferences::version_ordering`] when `None`
    ///
    /// Filtering:
    /// - `first_version`
    pub fn sort_summaries(
        &self,
        summaries: &mut Vec<Summary>,
        first_version: Option<VersionOrdering>,
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
            if previous_cmp != Ordering::Equal {
                return previous_cmp;
            }

            if let Some(max_rust_version) = &self.max_rust_version {
                let a_is_compat = a
                    .rust_version()
                    .map(|a| a.is_compatible_with(max_rust_version))
                    .unwrap_or(true);
                let b_is_compat = b
                    .rust_version()
                    .map(|b| b.is_compatible_with(max_rust_version))
                    .unwrap_or(true);
                match (a_is_compat, b_is_compat) {
                    (true, true) => {}   // fallback
                    (false, false) => {} // fallback
                    (true, false) => return Ordering::Less,
                    (false, true) => return Ordering::Greater,
                }
            }

            let cmp = a.version().cmp(b.version());
            match first_version.unwrap_or(self.version_ordering) {
                VersionOrdering::MaximumVersionsFirst => cmp.reverse(),
                VersionOrdering::MinimumVersionsFirst => cmp,
            }
        });
        if first_version.is_some() && !summaries.is_empty() {
            let _ = summaries.split_off(1);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::core::SourceId;
    use std::collections::BTreeMap;

    fn pkgid(name: &str, version: &str) -> PackageId {
        let src_id =
            SourceId::from_url("registry+https://github.com/rust-lang/crates.io-index").unwrap();
        PackageId::try_new(name, version, src_id).unwrap()
    }

    fn dep(name: &str, version: &str) -> Dependency {
        let src_id =
            SourceId::from_url("registry+https://github.com/rust-lang/crates.io-index").unwrap();
        Dependency::parse(name, Some(version), src_id).unwrap()
    }

    fn summ(name: &str, version: &str, msrv: Option<&str>) -> Summary {
        let pkg_id = pkgid(name, version);
        let features = BTreeMap::new();
        Summary::new(
            pkg_id,
            Vec::new(),
            &features,
            None::<&String>,
            msrv.map(|m| m.parse().unwrap()),
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
            summ("foo", "1.2.4", None),
            summ("foo", "1.2.3", None),
            summ("foo", "1.1.0", None),
            summ("foo", "1.0.9", None),
        ];

        vp.version_ordering(VersionOrdering::MaximumVersionsFirst);
        vp.sort_summaries(&mut summaries, None);
        assert_eq!(
            describe(&summaries),
            "foo/1.2.3, foo/1.2.4, foo/1.1.0, foo/1.0.9".to_string()
        );

        vp.version_ordering(VersionOrdering::MinimumVersionsFirst);
        vp.sort_summaries(&mut summaries, None);
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
            summ("foo", "1.2.4", None),
            summ("foo", "1.2.3", None),
            summ("foo", "1.1.0", None),
            summ("foo", "1.0.9", None),
        ];

        vp.version_ordering(VersionOrdering::MaximumVersionsFirst);
        vp.sort_summaries(&mut summaries, None);
        assert_eq!(
            describe(&summaries),
            "foo/1.2.3, foo/1.2.4, foo/1.1.0, foo/1.0.9".to_string()
        );

        vp.version_ordering(VersionOrdering::MinimumVersionsFirst);
        vp.sort_summaries(&mut summaries, None);
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
            summ("foo", "1.2.4", None),
            summ("foo", "1.2.3", None),
            summ("foo", "1.1.0", None),
            summ("foo", "1.0.9", None),
        ];

        vp.version_ordering(VersionOrdering::MaximumVersionsFirst);
        vp.sort_summaries(&mut summaries, None);
        assert_eq!(
            describe(&summaries),
            "foo/1.2.3, foo/1.1.0, foo/1.2.4, foo/1.0.9".to_string()
        );

        vp.version_ordering(VersionOrdering::MinimumVersionsFirst);
        vp.sort_summaries(&mut summaries, None);
        assert_eq!(
            describe(&summaries),
            "foo/1.1.0, foo/1.2.3, foo/1.0.9, foo/1.2.4".to_string()
        );
    }

    #[test]
    fn test_max_rust_version() {
        let mut vp = VersionPreferences::default();
        vp.max_rust_version(Some("1.50".parse().unwrap()));

        let mut summaries = vec![
            summ("foo", "1.2.4", None),
            summ("foo", "1.2.3", Some("1.60")),
            summ("foo", "1.2.2", None),
            summ("foo", "1.2.1", Some("1.50")),
            summ("foo", "1.2.0", None),
            summ("foo", "1.1.0", Some("1.40")),
            summ("foo", "1.0.9", None),
        ];

        vp.version_ordering(VersionOrdering::MaximumVersionsFirst);
        vp.sort_summaries(&mut summaries, None);
        assert_eq!(
            describe(&summaries),
            "foo/1.2.4, foo/1.2.2, foo/1.2.1, foo/1.2.0, foo/1.1.0, foo/1.0.9, foo/1.2.3"
                .to_string()
        );

        vp.version_ordering(VersionOrdering::MinimumVersionsFirst);
        vp.sort_summaries(&mut summaries, None);
        assert_eq!(
            describe(&summaries),
            "foo/1.0.9, foo/1.1.0, foo/1.2.0, foo/1.2.1, foo/1.2.2, foo/1.2.4, foo/1.2.3"
                .to_string()
        );
    }

    #[test]
    fn test_empty_summaries() {
        let vp = VersionPreferences::default();
        let mut summaries = vec![];

        vp.sort_summaries(&mut summaries, Some(VersionOrdering::MaximumVersionsFirst));
        assert_eq!(summaries, vec![]);
    }
}
