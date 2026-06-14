//! This module implements support for preferring some versions of a package
//! over other versions.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

use cargo_util_schemas::core::PartialVersion;

use crate::core::Dependency;
use crate::core::PackageId;
use crate::core::SourceId;
use crate::core::Summary;
use crate::util::CargoResult;
use crate::util::GlobalContext;
use crate::util::auth::RegistryConfig;
use crate::util::auth::RegistryConfigExtended;
use crate::util::context::CargoResolverConfig;
use crate::util::context::IncompatiblePublishAge;
use crate::util::interning::InternedString;
use crate::util::time_span::parse_time_span;

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
    publish_time: Option<jiff::Timestamp>,
    publish_age: Option<PublishAgePolicy>,
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

    pub fn publish_time(&mut self, publish_time: jiff::Timestamp) {
        self.publish_time = Some(publish_time);
    }

    pub fn publish_age(&mut self, policy: PublishAgePolicy) {
        self.publish_age = Some(policy);
    }

    /// Returns the version's publish-age if it is too new for the configured
    /// `min-publish-age`, otherwise `None`.
    pub fn too_new(&self, summary: &Summary) -> Option<PublishAgeViolation> {
        self.publish_age.as_ref()?.too_new(summary)
    }

    /// Whether the given package is preferred.
    pub fn should_prefer(&self, pkg_id: &PackageId) -> bool {
        self.try_to_use.contains(pkg_id)
            || self
                .prefer_patch_deps
                .get(&pkg_id.name())
                .map(|deps| deps.iter().any(|d| d.matches_id(*pkg_id)))
                .unwrap_or(false)
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
    /// - `publish_time`
    /// - `first_version`
    pub fn sort_summaries(
        &self,
        summaries: &mut Vec<Summary>,
        first_version: Option<VersionOrdering>,
    ) {
        if let Some(max_publish_time) = self.publish_time {
            summaries.retain(|s| {
                if let Some(summary_publish_time) = s.pubtime() {
                    summary_publish_time <= max_publish_time
                } else {
                    true
                }
            });
        }
        summaries.sort_unstable_by(|a, b| {
            let prefer_a = self.should_prefer(&a.package_id());
            let prefer_b = self.should_prefer(&b.package_id());
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

/// Snapshot of the `min-publish-age` configuration before resolution started.
#[derive(Debug)]
pub struct PublishAgePolicy {
    /// Reference "now" from [`GlobalContext::invocation_time`].
    invocation_time: jiff::Timestamp,
    /// `registry.global-min-publish-age`
    global: MinPublishAge,
    /// `registry.min-publish-age`
    crates_io: MinPublishAge,
    /// `registries.<name>.min-publish-age`
    per_registry: HashMap<String, MinPublishAge>,
}

impl PublishAgePolicy {
    /// Builds the policy from `min-publish-age` configuration.
    ///
    /// Returns `None` when either meets
    ///
    /// * the `-Zmin-publish-age` gate is off
    /// * the resolver is configured to allow pubtime-incompatible versions
    /// * no threshold is configured at all
    pub fn new(gctx: &GlobalContext) -> CargoResult<Option<Self>> {
        if !gctx.cli_unstable().min_publish_age {
            return Ok(None);
        }

        let resolver_config = gctx.get::<Option<CargoResolverConfig>>("resolver")?;
        if resolver_config
            .and_then(|c| c.incompatible_publish_age)
            .is_some_and(|v| v == IncompatiblePublishAge::Allow)
        {
            return Ok(None);
        }

        let parse = |key: &str, config: Option<String>| -> CargoResult<MinPublishAge> {
            let Some(config) = config else {
                return Ok(MinPublishAge::Unset);
            };
            if config == "0" {
                return Ok(MinPublishAge::None);
            }
            let duration = parse_time_span(&config)
                .map_err(|e| anyhow::format_err!("invalid value for `{key}`: {e}"))?;
            Ok(MinPublishAge::Age(duration, config))
        };

        let registry = gctx.get::<Option<RegistryConfigExtended>>("registry")?;
        let global = parse(
            "registry.global-min-publish-age",
            registry
                .as_ref()
                .and_then(|r| r.global_min_publish_age.clone()),
        )?;
        let crates_io = parse(
            "registry.min-publish-age",
            registry.and_then(|r| r.min_publish_age),
        )?;
        let mut per_registry = HashMap::new();
        if let Some(registries) =
            gctx.get::<Option<HashMap<String, RegistryConfig>>>("registries")?
        {
            for (name, config) in registries {
                let limit = parse(
                    &format!("registries.{name}.min-publish-age"),
                    config.min_publish_age,
                )?;
                if limit.is_set() {
                    per_registry.insert(name, limit);
                }
            }
        }

        let nothing_configured = !global.is_set() && !crates_io.is_set() && per_registry.is_empty();
        if nothing_configured {
            return Ok(None);
        }

        Ok(Some(Self {
            invocation_time: gctx.invocation_time(),
            global,
            crates_io,
            per_registry,
        }))
    }

    /// Returns the version's publish-age if it is too new for its registry.
    ///
    /// `None` means the version is acceptable.
    pub fn too_new(&self, summary: &Summary) -> Option<PublishAgeViolation> {
        let pubtime = summary.pubtime()?;
        let MinPublishAge::Age(min_age, config) = self.min_age(summary.source_id()) else {
            return None;
        };

        let max_pubtime = jiff::SignedDuration::try_from(*min_age)
            .ok()
            .and_then(|min_age| self.invocation_time.checked_sub(min_age).ok());

        let age = self.invocation_time.duration_since(pubtime);
        let publish_age = || PublishAgeViolation {
            age,
            config: config.clone(),
        };

        match max_pubtime {
            Some(max_pubtime) => (pubtime > max_pubtime).then(publish_age),
            None => Some(publish_age()),
        }
    }

    /// Resolves the minimum publish age for a given registry source.
    ///
    /// Priority:
    ///
    /// 1. `registries.<name>.min-publish-age`
    /// 2. `registry.min-publish-age`
    /// 3. `registry.global-min-publish-age`
    fn min_age(&self, source_id: SourceId) -> &MinPublishAge {
        // `registries.<name>` also covers crates.io, whose name is `crates-io`.
        if let Some(min_age) = source_id
            .alt_registry_key()
            .and_then(|name| self.per_registry.get(name))
            .filter(|min_age| min_age.is_set())
        {
            return min_age;
        }

        if source_id.is_crates_io() && self.crates_io.is_set() {
            return &self.crates_io;
        }

        &self.global
    }
}

/// A configured `min-publish-age` value for one scope.
#[derive(Debug, Clone)]
enum MinPublishAge {
    /// Key unset.
    Unset,
    /// No min-publish-age limit at all.
    None,
    /// An age threshold, with the raw config string for display.
    Age(Duration, String),
}

impl MinPublishAge {
    /// Whether a value was configured for this scope.
    fn is_set(&self) -> bool {
        !matches!(self, MinPublishAge::Unset)
    }
}

/// A violation of `min-publish-age` config.
#[derive(Debug, Clone, PartialEq)]
pub struct PublishAgeViolation {
    /// How long ago the version was published.
    age: jiff::SignedDuration,
    /// The configured `min-publish-age` it violates
    config: String,
}

impl PublishAgeViolation {
    /// How long ago the version was published,
    /// as a single friendly-spelled unit for display.
    pub fn age_label(&self) -> String {
        format_age_as_single_unit(self.age)
    }

    /// The configured `min-publish-age` it violates
    pub fn config(&self) -> &str {
        &self.config
    }
}

/// Formats an age as a single, friendly-spelled unit, never is multi-unit noise.
fn format_age_as_single_unit(age: jiff::SignedDuration) -> String {
    use jiff::Unit;
    use jiff::fmt::friendly::Designator;
    use jiff::fmt::friendly::Spacing;
    use jiff::fmt::friendly::SpanPrinter;

    // An age at or ahead of "now" gives a non-positive age.
    if age <= jiff::SignedDuration::ZERO {
        return "moments ago".to_string();
    }

    let rounded = jiff::Span::try_from(age).and_then(|span| {
        let unit = if age >= jiff::SignedDuration::from_hours(48) {
            Unit::Day
        } else if age >= jiff::SignedDuration::from_hours(1) {
            Unit::Hour
        } else if age >= jiff::SignedDuration::from_mins(1) {
            Unit::Minute
        } else {
            Unit::Second
        };
        let opts = jiff::SpanRound::new()
            .largest(unit)
            .smallest(unit)
            .relative(jiff::SpanRelativeTo::days_are_24_hours());
        span.round(opts)
    });

    let printer = SpanPrinter::new()
        .designator(Designator::Verbose)
        .spacing(Spacing::BetweenUnitsAndDesignators);

    match rounded {
        Ok(span) => format!("{} ago", printer.span_to_string(&span)),
        Err(e) => {
            tracing::warn!("failed to round `{age}`: {e}");
            format!("{} seconds ago", age.as_secs())
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::core::SourceId;
    use crate::sources::CRATES_IO_INDEX;
    use crate::sources::CRATES_IO_REGISTRY;
    use crate::util::IntoUrl as _;

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

    const NOW: &str = "2006-08-08T00:00:00Z";

    fn age(raw: &str) -> MinPublishAge {
        MinPublishAge::Age(parse_time_span(raw).unwrap(), raw.to_string())
    }

    fn hours(n: i64) -> jiff::SignedDuration {
        jiff::SignedDuration::from_hours(n)
    }

    fn policy(
        global: MinPublishAge,
        crates_io: MinPublishAge,
        per_registry: &[(&str, MinPublishAge)],
    ) -> PublishAgePolicy {
        PublishAgePolicy {
            invocation_time: NOW.parse().unwrap(),
            global,
            crates_io,
            per_registry: per_registry
                .iter()
                .map(|(name, age)| (name.to_string(), age.clone()))
                .collect(),
        }
    }

    fn crates_io_source() -> SourceId {
        let url = CRATES_IO_INDEX.into_url().unwrap();
        SourceId::for_alt_registry(&url, CRATES_IO_REGISTRY).unwrap()
    }

    fn alt_source() -> SourceId {
        let url = "https://example.com/index".into_url().unwrap();
        SourceId::for_alt_registry(&url, "alt").unwrap()
    }

    /// Gets a summary on `source`, published `age` before `NOW`.
    /// If age is negative, it means it is published in the future.
    fn published(source: SourceId, age: jiff::SignedDuration) -> Summary {
        let pkg_id = PackageId::try_new("foo", "1.0.0", source).unwrap();
        let mut summary =
            Summary::new(pkg_id, Vec::new(), &BTreeMap::new(), None::<&String>, None).unwrap();
        let now: jiff::Timestamp = NOW.parse().unwrap();
        summary.set_pubtime(now - age);
        summary
    }

    #[test]
    fn publish_age_reports_exact_age() {
        let p = policy(age("7 days"), MinPublishAge::Unset, &[]);
        let violation = p.too_new(&published(crates_io_source(), hours(50)));
        assert_eq!(
            violation,
            Some(PublishAgeViolation {
                age: hours(50),
                config: "7 days".to_string(),
            })
        );
    }

    #[test]
    fn publish_age_older_than_threshold_is_acceptable() {
        let p = policy(age("7 days"), MinPublishAge::Unset, &[]);
        let violation = p.too_new(&published(crates_io_source(), hours(10 * 24)));
        assert_eq!(violation, None);
    }

    #[test]
    fn publish_age_at_threshold_boundary_is_acceptable() {
        let p = policy(age("7 days"), MinPublishAge::Unset, &[]);
        let violation = p.too_new(&published(crates_io_source(), hours(7 * 24)));
        assert_eq!(violation, None);
    }

    #[test]
    fn publish_age_just_inside_threshold_is_too_new() {
        let p = policy(age("7 days"), MinPublishAge::Unset, &[]);
        let violation = p.too_new(&published(crates_io_source(), hours(7 * 24 - 1)));
        assert_eq!(
            violation,
            Some(PublishAgeViolation {
                age: hours(7 * 24 - 1),
                config: "7 days".to_string(),
            })
        );
    }

    #[test]
    fn publish_age_per_registry_overrides_global() {
        let p = policy(
            age("30 days"),
            MinPublishAge::Unset,
            &[("alt", age("1 day"))],
        );
        let violation = p.too_new(&published(alt_source(), hours(2 * 24)));
        assert_eq!(violation, None);
    }

    #[test]
    fn publish_age_crates_io_scope_excludes_alt_registry() {
        let p = policy(age("1 day"), age("30 days"), &[]);
        let crates_io = p.too_new(&published(crates_io_source(), hours(2 * 24)));
        let alt = p.too_new(&published(alt_source(), hours(2 * 24)));
        assert_eq!(
            crates_io,
            Some(PublishAgeViolation {
                age: hours(2 * 24),
                config: "30 days".to_string(),
            })
        );
        assert_eq!(alt, None);
    }

    #[test]
    fn publish_age_alt_registry_falls_through_to_global() {
        let p = policy(age("7 days"), MinPublishAge::Unset, &[]);
        let violation = p.too_new(&published(alt_source(), hours(2 * 24)));
        assert_eq!(
            violation,
            Some(PublishAgeViolation {
                age: hours(2 * 24),
                config: "7 days".to_string(),
            })
        );
    }

    #[test]
    fn publish_age_per_registry_too_new() {
        let p = policy(
            MinPublishAge::Unset,
            MinPublishAge::Unset,
            &[("alt", age("7 days"))],
        );
        let violation = p.too_new(&published(alt_source(), hours(2 * 24)));
        assert_eq!(
            violation,
            Some(PublishAgeViolation {
                age: hours(2 * 24),
                config: "7 days".to_string(),
            })
        );
    }

    #[test]
    fn publish_age_per_registry_zero_overrides_global() {
        let p = policy(
            age("30 days"),
            MinPublishAge::Unset,
            &[("alt", MinPublishAge::None)],
        );
        let violation = p.too_new(&published(alt_source(), hours(0)));
        assert_eq!(violation, None);
    }

    #[test]
    fn publish_age_no_applicable_scope_is_acceptable() {
        let p = policy(MinPublishAge::Unset, age("7 days"), &[]);
        let violation = p.too_new(&published(alt_source(), hours(0)));
        assert_eq!(violation, None);
    }

    #[test]
    fn publish_age_zero_disables_threshold() {
        let p = policy(MinPublishAge::None, MinPublishAge::Unset, &[]);
        let violation = p.too_new(&published(crates_io_source(), hours(0)));
        assert_eq!(violation, None);
    }

    #[test]
    fn publish_age_zero_stops_scope_fallthrough() {
        let p = policy(age("30 days"), MinPublishAge::None, &[]);
        let violation = p.too_new(&published(crates_io_source(), hours(0)));
        assert_eq!(violation, None);
    }

    #[test]
    fn publish_age_missing_pubtime_is_acceptable() {
        let p = policy(age("7 days"), MinPublishAge::Unset, &[]);
        let pkg_id = PackageId::try_new("foo", "1.0.0", crates_io_source()).unwrap();
        let summary =
            Summary::new(pkg_id, Vec::new(), &BTreeMap::new(), None::<&String>, None).unwrap();
        let violation = p.too_new(&summary);
        assert_eq!(violation, None);
    }

    #[test]
    fn publish_age_future_pubtime_is_too_new() {
        let p = policy(age("7 days"), MinPublishAge::Unset, &[]);
        let violation = p.too_new(&published(crates_io_source(), hours(-24)));
        assert_eq!(
            violation,
            Some(PublishAgeViolation {
                age: hours(-24),
                config: "7 days".to_string(),
            })
        );
    }

    #[test]
    fn publish_age_out_of_range_threshold_is_too_new() {
        // u64::MAX
        let p = policy(
            age("18446744073709551615 seconds"),
            MinPublishAge::Unset,
            &[],
        );
        let violation = p.too_new(&published(crates_io_source(), hours(24)));
        assert_eq!(
            violation,
            Some(PublishAgeViolation {
                age: hours(24),
                config: "18446744073709551615 seconds".to_string(),
            })
        );
    }

    #[track_caller]
    fn assert_age(secs: i64, expected: &str) {
        assert_eq!(
            format_age_as_single_unit(jiff::SignedDuration::from_secs(secs)),
            expected
        );
    }

    const MIN: i64 = 60;
    const HOUR: i64 = 60 * MIN;
    const DAY: i64 = 24 * HOUR;

    #[test]
    fn rounds_to_a_single_unit() {
        // `>= 2 days` rounds to the nearest day.
        assert_age(2 * DAY, "2 days ago");
        assert_age(2 * DAY + 8 * HOUR + 23 * MIN, "2 days ago");
        assert_age(2 * DAY + 13 * HOUR, "3 days ago");
        assert_age(540 * DAY, "540 days ago");

        // `1 hour ..< 2 days` rounds to the nearest hour.
        assert_age(47 * HOUR, "47 hours ago");
        assert_age(24 * HOUR, "24 hours ago");
        assert_age(11 * HOUR + 40 * MIN, "12 hours ago");
        assert_age(11 * HOUR + 20 * MIN, "11 hours ago");
        assert_age(HOUR, "1 hour ago");

        // `1 minute ..< 1 hour` rounds to the nearest minute.
        assert_age(40 * MIN, "40 minutes ago");
        assert_age(MIN, "1 minute ago");

        // `< 1 minute` rounds to the nearest second.
        assert_age(40, "40 seconds ago");
        assert_age(1, "1 second ago");

        // ahead of "now" (clock drift)
        assert_age(0, "moments ago");
        assert_age(-20, "moments ago");
        assert_age(-2 * DAY, "moments ago");
    }
}
