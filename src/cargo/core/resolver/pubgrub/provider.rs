//! The pubgrub [`DependencyProvider`] backed by Cargo's registry.
//!
//! This bridges two impedance mismatches between Cargo and pubgrub:
//!
//! * **async vs. sync.** Cargo's [`RegistryQueryer`] is poll-based and driven by
//!   an outer `wait()` loop, while pubgrub drives resolution synchronously by
//!   calling back into the provider. We block on the poll loop inside
//!   [`Provider::query`], reusing the queryer's caching.
//! * **registry data vs. the package encoding.** Cargo describes crates with
//!   [`Summary`]/[`Dependency`]/[`FeatureValue`]; we translate those into the
//!   [`PubGrubPackage`] encoding on demand in [`Provider::get_dependencies`].
//!
//! The translation logic mirrors the encoding used by
//! `Eh2406/pubgrub-crates-benchmark`, adapted to Cargo's types and multiple
//! sources.

use std::cell::RefCell;
use std::cmp::Reverse;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::rc::Rc;
use std::task::Poll;

use pubgrub::{Dependencies, DependencyProvider, PackageResolutionStatistics};
use semver::Version;

use crate::core::dependency::DepKind;
use crate::core::resolver::VersionPreferences;
use crate::core::resolver::dep_cache::RegistryQueryer;
use crate::core::summary::FeatureValue;
use crate::core::{Dependency, Registry, SourceId, Summary};
use crate::util::interning::InternedString;

use super::package::{
    BucketName, FeatureNamespace, PubGrubPackage, WideName, opt_version_req_to_pubgrub,
    opt_version_req_to_version_req,
};
use super::semver_pubgrub::{SemverCompatibility, SemverPubgrub};

/// The version PubGrub assigns to the synthetic [`PubGrubPackage::Root`].
pub fn root_version() -> Version {
    Version::new(0, 0, 0)
}

/// Error surfaced from the [`DependencyProvider`] callbacks.
///
/// Real (e.g. network) errors from the registry are stashed on the provider and
/// re-surfaced by the caller; this type is just the pubgrub-facing sentinel.
#[derive(Debug)]
pub struct ProviderError(String);

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for ProviderError {}

/// A single workspace member to resolve, plus how its features were requested.
pub struct Root {
    pub summary: Summary,
    /// Whether dev-dependencies of this member should be included.
    pub dev_deps: bool,
    /// Whether every feature should be enabled (lock-file resolution).
    pub all_features: bool,
    /// Whether default features are enabled.
    pub default_features: bool,
    /// Specific features requested (when `all_features` is false).
    pub features: Vec<InternedString>,
}

pub struct Provider<'a, T: Registry> {
    registry: RefCell<RegistryQueryer<'a, T>>,
    version_prefs: &'a VersionPreferences,
    roots: Vec<Root>,
    /// Cache of candidate summaries per crate, in preference order.
    versions: RefCell<HashMap<(InternedString, SourceId), Rc<Vec<Summary>>>>,
    /// Stashed real error from the registry, re-surfaced by the caller.
    error: RefCell<Option<anyhow::Error>>,
}

impl<'a, T: Registry> Provider<'a, T> {
    pub fn new(
        registry: RegistryQueryer<'a, T>,
        version_prefs: &'a VersionPreferences,
        roots: Vec<Root>,
    ) -> Self {
        // Workspace members are provided directly rather than queried from the
        // registry (they are typically path/local sources). Seed the version
        // cache with their summaries so `candidates`/`summary_for` find them.
        let mut versions: HashMap<(InternedString, SourceId), Rc<Vec<Summary>>> = HashMap::new();
        {
            let mut grouped: HashMap<(InternedString, SourceId), Vec<Summary>> = HashMap::new();
            for root in &roots {
                grouped
                    .entry((root.summary.name(), root.summary.source_id()))
                    .or_default()
                    .push(root.summary.clone());
            }
            for (key, summaries) in grouped {
                versions.insert(key, Rc::new(summaries));
            }
        }
        Provider {
            registry: RefCell::new(registry),
            version_prefs,
            roots,
            versions: RefCell::new(versions),
            error: RefCell::new(None),
        }
    }

    pub fn take_error(&self) -> Option<anyhow::Error> {
        self.error.borrow_mut().take()
    }

    pub fn registry(&self) -> std::cell::Ref<'_, RegistryQueryer<'a, T>> {
        self.registry.borrow()
    }

    /// Stash a real error and return the pubgrub sentinel.
    fn fail(&self, context: impl Into<String>, err: anyhow::Error) -> ProviderError {
        let msg = context.into();
        if self.error.borrow().is_none() {
            *self.error.borrow_mut() = Some(err);
        }
        ProviderError(msg)
    }

    /// Blocking enumeration of all candidate versions of a crate, in preference
    /// order (preferred/locked first, then highest version, honoring
    /// minimal-versions and publish-time filters via [`VersionPreferences`]).
    fn candidates(
        &self,
        name: InternedString,
        source: SourceId,
    ) -> Result<Rc<Vec<Summary>>, ProviderError> {
        if let Some(c) = self.versions.borrow().get(&(name, source)) {
            return Ok(c.clone());
        }
        // A wildcard dependency to enumerate every version of the crate.
        let dep = Dependency::parse(name, None, source)
            .map_err(|e| self.fail(format!("failed to query `{name}`"), e))?;
        let summaries = {
            let mut registry = self.registry.borrow_mut();
            loop {
                match registry.query(&dep, None) {
                    Poll::Ready(Ok(s)) => break s,
                    Poll::Ready(Err(e)) => {
                        return Err(self.fail(format!("failed to query `{name}`"), e));
                    }
                    Poll::Pending => {
                        if let Err(e) = registry.wait() {
                            return Err(self.fail(format!("failed to query `{name}`"), e));
                        }
                    }
                }
            }
        };
        let mut summaries = (*summaries).clone();
        // Order by Cargo's version preferences so `choose_version` selects the
        // same candidate the default resolver would prefer.
        self.version_prefs.sort_summaries(&mut summaries, None);
        let summaries = Rc::new(summaries);
        self.versions
            .borrow_mut()
            .insert((name, source), summaries.clone());
        Ok(summaries)
    }

    /// The summary for an exact (name, source, version), if it exists.
    pub(super) fn summary_for(
        &self,
        name: InternedString,
        source: SourceId,
        version: &Version,
    ) -> Result<Option<Summary>, ProviderError> {
        let candidates = self.candidates(name, source)?;
        Ok(candidates
            .iter()
            .find(|s| s.version() == version)
            .cloned())
    }

    /// If every available version matching `dep` lies in a single compatibility
    /// bucket, return it. Used to decide between a plain bucket and a wide
    /// package.
    fn only_one_compat_in_data(&self, dep: &Dependency) -> Option<SemverCompatibility> {
        let pubgrub_req = opt_version_req_to_pubgrub(dep.version_req());
        let candidates = self.candidates(dep.package_name(), dep.source_id()).ok()?;
        let mut iter = candidates
            .iter()
            .map(|s| s.version())
            .filter(|v| pubgrub_req.contains(v))
            .map(SemverCompatibility::from);
        let first = iter.next()?;
        if iter.any(|c| c != first) {
            None
        } else {
            Some(first)
        }
    }

    /// Map a Cargo [`Dependency`] to the PubGrub package + version range that
    /// represents it.
    pub(super) fn from_dep(
        &self,
        dep: &Dependency,
        from: InternedString,
        from_version: &Version,
    ) -> (PubGrubPackage, SemverPubgrub) {
        let pubgrub_req = opt_version_req_to_pubgrub(dep.version_req());
        let compat = pubgrub_req
            .only_one_compatibility_range()
            .or_else(|| self.only_one_compat_in_data(dep));
        match compat {
            Some(compat) => (
                PubGrubPackage::Bucket {
                    name: BucketName {
                        name: dep.package_name(),
                        source: dep.source_id(),
                        compat,
                    },
                    member: false,
                    all_features: false,
                },
                pubgrub_req,
            ),
            None => (
                PubGrubPackage::Wide {
                    name: WideName {
                        name: dep.package_name(),
                        source: dep.source_id(),
                        req: opt_version_req_to_version_req(dep.version_req()),
                        from,
                        from_compat: SemverCompatibility::from(from_version),
                    },
                },
                SemverPubgrub::full(),
            ),
        }
    }

    /// Count candidate versions of a crate that fall in `range`.
    fn count_matches(&self, range: &SemverPubgrub, name: InternedString, source: SourceId) -> u32 {
        self.candidates(name, source)
            .map(|c| c.iter().filter(|s| range.contains(s.version())).count() as u32)
            .unwrap_or(0)
    }
}

/// Insert a dependency constraint, intersecting with any existing one for the
/// same package.
fn deps_insert(
    deps: &mut HashMap<PubGrubPackage, SemverPubgrub>,
    pkg: PubGrubPackage,
    range: SemverPubgrub,
) {
    deps.entry(pkg)
        .and_modify(|old| *old = old.intersection(&range))
        .or_insert(range);
}

/// A deterministic synthetic version for a `links` package, unique to a given
/// crate version, so that two crates declaring the same `links` value conflict.
fn links_version(pkg: &PubGrubPackage, version: &Version) -> Version {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    pkg.hash(&mut hasher);
    version.hash(&mut hasher);
    let h = hasher.finish();
    let b = h.to_be_bytes();
    Version::new(
        u16::from_be_bytes([b[0], b[1]]) as u64,
        u16::from_be_bytes([b[2], b[3]]) as u64,
        u16::from_be_bytes([b[4], b[5]]) as u64,
    )
}

impl<'a, T: Registry> DependencyProvider for Provider<'a, T> {
    type P = PubGrubPackage;
    type V = Version;
    type VS = SemverPubgrub;
    type M = String;
    type Err = ProviderError;
    type Priority = (u32, Reverse<u32>);

    fn choose_version(
        &self,
        package: &PubGrubPackage,
        range: &SemverPubgrub,
    ) -> Result<Option<Version>, ProviderError> {
        Ok(match package {
            PubGrubPackage::Root => Some(root_version()),
            PubGrubPackage::Links { .. } => {
                use std::ops::Bound;
                match range.bounding_range() {
                    Some((_, Bound::Included(v))) => Some(v.clone()),
                    _ => return Err(ProviderError("links package has no concrete version".into())),
                }
            }
            PubGrubPackage::Wide { name }
            | PubGrubPackage::WideFeatures { name, .. }
            | PubGrubPackage::WideDefaultFeatures { name } => {
                // Pick the canonical version of the first compatibility bucket
                // that matches the wide requirement and lies in `range`.
                let candidates = self.candidates(name.name, name.source)?;
                candidates
                    .iter()
                    .map(|s| s.version())
                    .filter(|v| name.req.matches(v))
                    .map(|v| SemverCompatibility::from(v).canonical())
                    .find(|v| range.contains(v))
            }
            PubGrubPackage::Bucket { name, .. }
            | PubGrubPackage::BucketFeatures { name, .. }
            | PubGrubPackage::BucketDefaultFeatures { name } => {
                let candidates = self.candidates(name.name, name.source)?;
                candidates
                    .iter()
                    .map(|s| s.version())
                    .find(|v| range.contains(v))
                    .cloned()
            }
        })
    }

    fn prioritize(
        &self,
        package: &PubGrubPackage,
        range: &SemverPubgrub,
        stats: &PackageResolutionStatistics,
    ) -> Self::Priority {
        let conflicts = stats.conflict_count();
        match package {
            PubGrubPackage::Root => (conflicts, Reverse(0)),
            // Decide links last: it only rubber-stamps uniqueness.
            PubGrubPackage::Links { .. } => (conflicts, Reverse(u32::MAX)),
            PubGrubPackage::Bucket { name, .. } => {
                if range.as_singleton().is_some() {
                    (conflicts, Reverse(1))
                } else {
                    (conflicts, Reverse(self.count_matches(range, name.name, name.source)))
                }
            }
            PubGrubPackage::BucketFeatures { name, .. }
            | PubGrubPackage::BucketDefaultFeatures { name } => {
                if range.as_singleton().is_some() {
                    (conflicts, Reverse(0))
                } else {
                    (
                        conflicts,
                        Reverse(
                            self.count_matches(range, name.name, name.source)
                                .saturating_add(1),
                        ),
                    )
                }
            }
            PubGrubPackage::Wide { name }
            | PubGrubPackage::WideFeatures { name, .. }
            | PubGrubPackage::WideDefaultFeatures { name } => (
                conflicts,
                Reverse(
                    self.count_matches(range, name.name, name.source)
                        .saturating_add(1),
                ),
            ),
        }
    }

    fn get_dependencies(
        &self,
        package: &PubGrubPackage,
        version: &Version,
    ) -> Result<Dependencies<PubGrubPackage, SemverPubgrub, String>, ProviderError> {
        let mut deps: HashMap<PubGrubPackage, SemverPubgrub> = HashMap::new();
        match package {
            PubGrubPackage::Root => {
                for root in &self.roots {
                    let summary = &root.summary;
                    let name = BucketName {
                        name: summary.name(),
                        source: summary.source_id(),
                        compat: SemverCompatibility::from(summary.version()),
                    };
                    let singleton = SemverPubgrub::singleton(summary.version().clone());
                    // The member itself, pinned to its exact version.
                    deps_insert(
                        &mut deps,
                        PubGrubPackage::Bucket {
                            name: name.clone(),
                            member: root.dev_deps,
                            all_features: root.all_features,
                        },
                        singleton.clone(),
                    );
                    if root.all_features {
                        // The all-features bucket pulls in every feature itself.
                    } else {
                        if root.default_features {
                            deps_insert(
                                &mut deps,
                                PubGrubPackage::BucketDefaultFeatures { name: name.clone() },
                                singleton.clone(),
                            );
                        }
                        for feat in &root.features {
                            deps_insert(
                                &mut deps,
                                PubGrubPackage::BucketFeatures {
                                    name: name.clone(),
                                    feature: FeatureNamespace::Feat(*feat),
                                },
                                singleton.clone(),
                            );
                        }
                    }
                }
                return Ok(Dependencies::Available(deps.into_iter().collect()));
            }

            PubGrubPackage::Links { .. } => {
                return Ok(Dependencies::Available(deps.into_iter().collect()));
            }

            PubGrubPackage::Bucket { name, member, all_features } => {
                let Some(summary) = self.summary_for(name.name, name.source, version)? else {
                    return Ok(Dependencies::Unavailable("no such version".into()));
                };
                // `links` uniqueness.
                if let Some(link) = summary.links() {
                    deps.insert(
                        PubGrubPackage::Links { links: link },
                        SemverPubgrub::singleton(links_version(package, version)),
                    );
                }
                for dep in summary.dependencies() {
                    let is_dev = dep.kind() == DepKind::Development;
                    if is_dev && !*member {
                        continue;
                    }
                    if dep.is_optional() && !*all_features {
                        // Optional deps are activated via feature packages.
                        continue;
                    }
                    let (cray, range) = self.from_dep(dep, name.name, version);
                    deps_insert(&mut deps, cray.clone(), range.clone());
                    if dep.uses_default_features() {
                        deps_insert(&mut deps, cray.with_default_features(), range.clone());
                    }
                    for f in dep.features() {
                        deps_insert(
                            &mut deps,
                            cray.with_feature(FeatureNamespace::Feat(*f)),
                            range.clone(),
                        );
                    }
                }
                if *all_features {
                    // Enable every feature (the implicit features of optional
                    // dependencies are included in the feature map, so this
                    // also activates all optional dependencies).
                    for feat in summary.features().keys() {
                        deps_insert(
                            &mut deps,
                            PubGrubPackage::BucketFeatures {
                                name: name.clone(),
                                feature: FeatureNamespace::Feat(*feat),
                            },
                            SemverPubgrub::singleton(version.clone()),
                        );
                    }
                }
                return Ok(Dependencies::Available(deps.into_iter().collect()));
            }

            PubGrubPackage::BucketFeatures { name, feature: FeatureNamespace::Feat(feat) } => {
                let Some(summary) = self.summary_for(name.name, name.source, version)? else {
                    return Ok(Dependencies::Unavailable("no such version".into()));
                };
                // A feature implies the crate at this exact version.
                deps.insert(
                    PubGrubPackage::Bucket {
                        name: name.clone(),
                        member: false,
                        all_features: false,
                    },
                    SemverPubgrub::singleton(version.clone()),
                );
                let Some(values) = summary.features().get(feat) else {
                    return Ok(Dependencies::Unavailable(format!(
                        "no feature `{feat}`"
                    )));
                };
                let singleton = SemverPubgrub::singleton(version.clone());
                for fv in values {
                    match fv {
                        FeatureValue::Feature(f) => deps_insert(
                            &mut deps,
                            package.with_feature(FeatureNamespace::Feat(*f)),
                            singleton.clone(),
                        ),
                        FeatureValue::Dep { dep_name } => deps_insert(
                            &mut deps,
                            package.with_feature(FeatureNamespace::Dep(*dep_name)),
                            singleton.clone(),
                        ),
                        FeatureValue::DepFeature {
                            dep_name,
                            dep_feature,
                            weak,
                        } => {
                            for dep in summary
                                .dependencies()
                                .iter()
                                .filter(|d| d.name_in_toml() == *dep_name)
                            {
                                if dep.kind() == DepKind::Development {
                                    continue;
                                }
                                let (cray, range) = self.from_dep(dep, name.name, version);
                                if dep.is_optional() {
                                    // Cargo's v1 lock resolver records the
                                    // optional dependency as part of the graph
                                    // for ANY `dep/feat` reference, including
                                    // weak `dep?/feat` ones. The `weak` flag
                                    // only controls whether the dependency's
                                    // own implicit feature is enabled.
                                    deps_insert(
                                        &mut deps,
                                        package.with_feature(FeatureNamespace::Dep(*dep_name)),
                                        singleton.clone(),
                                    );
                                    if !*weak
                                        && *dep_name != *feat
                                        && summary.features().contains_key(dep_name)
                                    {
                                        deps_insert(
                                            &mut deps,
                                            package.with_feature(FeatureNamespace::Feat(*dep_name)),
                                            singleton.clone(),
                                        );
                                    }
                                }
                                deps_insert(
                                    &mut deps,
                                    cray.with_feature(FeatureNamespace::Feat(*dep_feature)),
                                    range,
                                );
                            }
                        }
                    }
                }
                return Ok(Dependencies::Available(deps.into_iter().collect()));
            }

            PubGrubPackage::BucketFeatures { name, feature: FeatureNamespace::Dep(dep_name) } => {
                let Some(summary) = self.summary_for(name.name, name.source, version)? else {
                    return Ok(Dependencies::Unavailable("no such version".into()));
                };
                deps.insert(
                    PubGrubPackage::Bucket {
                        name: name.clone(),
                        member: false,
                        all_features: false,
                    },
                    SemverPubgrub::singleton(version.clone()),
                );
                let mut found = false;
                for dep in summary
                    .dependencies()
                    .iter()
                    .filter(|d| d.name_in_toml() == *dep_name)
                {
                    if !dep.is_optional() || dep.kind() == DepKind::Development {
                        continue;
                    }
                    found = true;
                    let (cray, range) = self.from_dep(dep, name.name, version);
                    deps_insert(&mut deps, cray.clone(), range.clone());
                    if dep.uses_default_features() {
                        deps_insert(&mut deps, cray.with_default_features(), range.clone());
                    }
                    for f in dep.features() {
                        deps_insert(
                            &mut deps,
                            cray.with_feature(FeatureNamespace::Feat(*f)),
                            range.clone(),
                        );
                    }
                }
                if found {
                    return Ok(Dependencies::Available(deps.into_iter().collect()));
                } else {
                    return Ok(Dependencies::Unavailable(format!(
                        "no optional dependency `{dep_name}`"
                    )));
                }
            }

            PubGrubPackage::BucketDefaultFeatures { name } => {
                let Some(summary) = self.summary_for(name.name, name.source, version)? else {
                    return Ok(Dependencies::Unavailable("no such version".into()));
                };
                deps.insert(
                    PubGrubPackage::Bucket {
                        name: name.clone(),
                        member: false,
                        all_features: false,
                    },
                    SemverPubgrub::singleton(version.clone()),
                );
                if summary.features().contains_key("default") {
                    deps_insert(
                        &mut deps,
                        package.with_feature(FeatureNamespace::Feat(InternedString::new("default"))),
                        SemverPubgrub::singleton(version.clone()),
                    );
                }
                return Ok(Dependencies::Available(deps.into_iter().collect()));
            }

            PubGrubPackage::Wide { name } => {
                let compat = SemverCompatibility::from(version);
                let range = opt_req_range(&name.req).intersection(&SemverPubgrub::compatibility(&compat));
                deps_insert(
                    &mut deps,
                    PubGrubPackage::Bucket {
                        name: BucketName {
                            name: name.name,
                            source: name.source,
                            compat,
                        },
                        member: false,
                        all_features: false,
                    },
                    range,
                );
                return Ok(Dependencies::Available(deps.into_iter().collect()));
            }

            PubGrubPackage::WideFeatures { name, feature } => {
                let compat = SemverCompatibility::from(version);
                let range = opt_req_range(&name.req).intersection(&SemverPubgrub::compatibility(&compat));
                // Tie this wide-feature decision to the underlying wide package.
                deps_insert(
                    &mut deps,
                    PubGrubPackage::Wide { name: name.clone() },
                    SemverPubgrub::singleton(version.clone()),
                );
                deps_insert(
                    &mut deps,
                    PubGrubPackage::BucketFeatures {
                        name: BucketName {
                            name: name.name,
                            source: name.source,
                            compat,
                        },
                        feature: *feature,
                    },
                    range,
                );
                return Ok(Dependencies::Available(deps.into_iter().collect()));
            }

            PubGrubPackage::WideDefaultFeatures { name } => {
                let compat = SemverCompatibility::from(version);
                let range = opt_req_range(&name.req).intersection(&SemverPubgrub::compatibility(&compat));
                deps_insert(
                    &mut deps,
                    PubGrubPackage::Wide { name: name.clone() },
                    SemverPubgrub::singleton(version.clone()),
                );
                deps_insert(
                    &mut deps,
                    PubGrubPackage::BucketDefaultFeatures {
                        name: BucketName {
                            name: name.name,
                            source: name.source,
                            compat,
                        },
                    },
                    range,
                );
                return Ok(Dependencies::Available(deps.into_iter().collect()));
            }
        }
    }
}

/// The PubGrub range for a bare [`semver::VersionReq`].
fn opt_req_range(req: &semver::VersionReq) -> SemverPubgrub {
    SemverPubgrub::from(req)
}
