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
    rust_versions: Vec<PartialVersion>,
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

    pub fn rust_versions(&mut self, vers: Vec<PartialVersion>) {
        self.rust_versions = vers;
    }

    /// Sort (and filter) the given vector of summaries in-place
    ///
    /// Note: all summaries presumed to be for the same package.
    ///
    /// Sort order:
    /// 1. Preferred packages
    /// 2. Most compatible [`VersionPreferences::rust_versions`]
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

            if !self.rust_versions.is_empty() {
                let a_compat_count = self.msrv_compat_count(a);
                let b_compat_count = self.msrv_compat_count(b);
                if b_compat_count != a_compat_count {
                    return b_compat_count.cmp(&a_compat_count);
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

    fn msrv_compat_count(&self, summary: &Summary) -> usize {
        let Some(rust_version) = summary.rust_version() else {
            return self.rust_versions.len();
        };

        self.rust_versions
            .iter()
            .filter(|max| rust_version.is_compatible_with(max))
            .count()
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
    fn test_single_rust_version() {
        let mut vp = VersionPreferences::default();
        vp.rust_versions(vec!["1.50".parse().unwrap()]);

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
    fn test_multiple_rust_versions() {
        let mut vp = VersionPreferences::default();
        vp.rust_versions(vec!["1.45".parse().unwrap(), "1.55".parse().unwrap()]);

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
            "foo/1.2.4, foo/1.2.2, foo/1.2.0, foo/1.1.0, foo/1.0.9, foo/1.2.1, foo/1.2.3"
                .to_string()
        );

        vp.version_ordering(VersionOrdering::MinimumVersionsFirst);
        vp.sort_summaries(&mut summaries, None);
        assert_eq!(
            describe(&summaries),
            "foo/1.0.9, foo/1.1.0, foo/1.2.0, foo/1.2.2, foo/1.2.4, foo/1.2.1, foo/1.2.3"
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
