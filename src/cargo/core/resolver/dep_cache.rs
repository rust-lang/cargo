//! There are 2 sources of facts for the resolver:
//!
//! - The `Registry` tells us for a `Dependency` what versions are available to fulfil it.
//! - The `Summary` tells us for a version (and features) what dependencies need to be fulfilled for it to be activated.
//!
//! These constitute immutable facts, the soled ground truth that all other inference depends on.
//! Theoretically this could all be enumerated ahead of time, but we want to be lazy and only
//! look up things we need to. The compromise is to cache the results as they are computed.
//!
//! This module impl that cache in all the gory details

use crate::core::resolver::context::ResolverContext;
use crate::core::resolver::errors::describe_path_in_context;
use crate::core::resolver::types::{ConflictReason, DepInfo, FeaturesSet};
use crate::core::resolver::{
    ActivateError, ActivateResult, CliFeatures, RequestedFeatures, ResolveOpts, VersionOrdering,
    VersionPreferences,
};
use crate::core::{
    Dependency, FeatureValue, PackageId, PackageIdSpec, PackageIdSpecQuery, Registry, Summary,
};
use crate::sources::source::QueryKind;
use crate::util::closest_msg;
use crate::util::errors::CargoResult;
use crate::util::interning::{InternedString, INTERNED_DEFAULT};

use anyhow::Context as _;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt::Write;
use std::rc::Rc;
use std::task::Poll;
use tracing::debug;

pub struct RegistryQueryer<'a> {
    pub registry: &'a mut (dyn Registry + 'a),
    replacements: &'a [(PackageIdSpec, Dependency)],
    version_prefs: &'a VersionPreferences,
    /// a cache of `Candidate`s that fulfil a `Dependency` (and whether `first_version`)
    registry_cache: HashMap<(Dependency, Option<VersionOrdering>), Poll<Rc<Vec<Summary>>>>,
    /// a cache of `Dependency`s that are required for a `Summary`
    ///
    /// HACK: `first_version` is not kept in the cache key is it is 1:1 with
    /// `parent.is_none()` (the first element of the cache key) as it doesn't change through
    /// execution.
    summary_cache: HashMap<
        (Option<PackageId>, Summary, ResolveOpts),
        (Rc<(HashSet<InternedString>, Rc<Vec<DepInfo>>)>, bool),
    >,
    /// all the cases we ended up using a supplied replacement
    used_replacements: HashMap<PackageId, Summary>,
}

impl<'a> RegistryQueryer<'a> {
    pub fn new(
        registry: &'a mut dyn Registry,
        replacements: &'a [(PackageIdSpec, Dependency)],
        version_prefs: &'a VersionPreferences,
    ) -> Self {
        RegistryQueryer {
            registry,
            replacements,
            version_prefs,
            registry_cache: HashMap::new(),
            summary_cache: HashMap::new(),
            used_replacements: HashMap::new(),
        }
    }

    pub fn reset_pending(&mut self) -> bool {
        let mut all_ready = true;
        self.registry_cache.retain(|_, r| {
            if !r.is_ready() {
                all_ready = false;
            }
            r.is_ready()
        });
        self.summary_cache.retain(|_, (_, r)| {
            if !*r {
                all_ready = false;
            }
            *r
        });
        all_ready
    }

    pub fn used_replacement_for(&self, p: PackageId) -> Option<(PackageId, PackageId)> {
        self.used_replacements.get(&p).map(|r| (p, r.package_id()))
    }

    pub fn replacement_summary(&self, p: PackageId) -> Option<&Summary> {
        self.used_replacements.get(&p)
    }

    /// Queries the `registry` to return a list of candidates for `dep`.
    ///
    /// This method is the location where overrides are taken into account. If
    /// any candidates are returned which match an override then the override is
    /// applied by performing a second query for what the override should
    /// return.
    pub fn query(
        &mut self,
        dep: &Dependency,
        first_version: Option<VersionOrdering>,
    ) -> Poll<CargoResult<Rc<Vec<Summary>>>> {
        let registry_cache_key = (dep.clone(), first_version);
        if let Some(out) = self.registry_cache.get(&registry_cache_key).cloned() {
            return out.map(Result::Ok);
        }

        let mut ret = Vec::new();
        let ready = self.registry.query(dep, QueryKind::Exact, &mut |s| {
            ret.push(s.into_summary());
        })?;
        if ready.is_pending() {
            self.registry_cache
                .insert((dep.clone(), first_version), Poll::Pending);
            return Poll::Pending;
        }
        for summary in ret.iter() {
            let mut potential_matches = self
                .replacements
                .iter()
                .filter(|(spec, _)| spec.matches(summary.package_id()));

            let Some((spec, dep)) = potential_matches.next() else {
                continue;
            };
            debug!(
                "found an override for {} {}",
                dep.package_name(),
                dep.version_req()
            );

            let mut summaries = match self.registry.query_vec(dep, QueryKind::Exact)? {
                Poll::Ready(s) => s.into_iter(),
                Poll::Pending => {
                    self.registry_cache
                        .insert((dep.clone(), first_version), Poll::Pending);
                    return Poll::Pending;
                }
            };
            let s = summaries
                .next()
                .ok_or_else(|| {
                    anyhow::format_err!(
                        "no matching package for override `{}` found\n\
                     location searched: {}\n\
                     version required: {}",
                        spec,
                        dep.source_id(),
                        dep.version_req()
                    )
                })?
                .into_summary();
            let summaries = summaries.collect::<Vec<_>>();
            if !summaries.is_empty() {
                let bullets = summaries
                    .iter()
                    .map(|s| format!("  * {}", s.package_id()))
                    .collect::<Vec<_>>();
                return Poll::Ready(Err(anyhow::anyhow!(
                    "the replacement specification `{}` matched \
                     multiple packages:\n  * {}\n{}",
                    spec,
                    s.package_id(),
                    bullets.join("\n")
                )));
            }

            assert_eq!(
                s.name(),
                summary.name(),
                "dependency should be hard coded to have the same name"
            );
            if s.version() != summary.version() {
                return Poll::Ready(Err(anyhow::anyhow!(
                    "replacement specification `{}` matched {} and tried to override it with {}\n\
                     avoid matching unrelated packages by being more specific",
                    spec,
                    summary.version(),
                    s.version(),
                )));
            }

            let replace = if s.source_id() == summary.source_id() {
                debug!("Preventing\n{:?}\nfrom replacing\n{:?}", summary, s);
                None
            } else {
                Some(s)
            };
            let matched_spec = spec.clone();

            // Make sure no duplicates
            if let Some((spec, _)) = potential_matches.next() {
                return Poll::Ready(Err(anyhow::anyhow!(
                    "overlapping replacement specifications found:\n\n  \
                     * {}\n  * {}\n\nboth specifications match: {}",
                    matched_spec,
                    spec,
                    summary.package_id()
                )));
            }

            for dep in summary.dependencies() {
                debug!("\t{} => {}", dep.package_name(), dep.version_req());
            }
            if let Some(r) = replace {
                self.used_replacements.insert(summary.package_id(), r);
            }
        }

        self.version_prefs.sort_summaries(&mut ret, first_version);

        let out = Poll::Ready(Rc::new(ret));

        self.registry_cache.insert(registry_cache_key, out.clone());

        out.map(Result::Ok)
    }

    /// Find out what dependencies will be added by activating `candidate`,
    /// with features described in `opts`. Then look up in the `registry`
    /// the candidates that will fulfil each of these dependencies, as it is the
    /// next obvious question.
    pub fn build_deps(
        &mut self,
        cx: &ResolverContext,
        parent: Option<PackageId>,
        candidate: &Summary,
        opts: &ResolveOpts,
        first_version: Option<VersionOrdering>,
    ) -> ActivateResult<Rc<(HashSet<InternedString>, Rc<Vec<DepInfo>>)>> {
        // if we have calculated a result before, then we can just return it,
        // as it is a "pure" query of its arguments.
        if let Some(out) = self
            .summary_cache
            .get(&(parent, candidate.clone(), opts.clone()))
        {
            return Ok(out.0.clone());
        }
        // First, figure out our set of dependencies based on the requested set
        // of features. This also calculates what features we're going to enable
        // for our own dependencies.
        let (used_features, deps) = resolve_features(parent, candidate, opts)?;

        // Next, transform all dependencies into a list of possible candidates
        // which can satisfy that dependency.
        let mut all_ready = true;
        let mut deps = deps
            .into_iter()
            .filter_map(|(dep, features)| match self.query(&dep, first_version) {
                Poll::Ready(Ok(candidates)) => Some(Ok((dep, candidates, features))),
                Poll::Pending => {
                    all_ready = false;
                    // we can ignore Pending deps, resolve will be repeatedly called
                    // until there are none to ignore
                    None
                }
                Poll::Ready(Err(e)) => Some(Err(e).with_context(|| {
                    format!(
                        "failed to get `{}` as a dependency of {}",
                        dep.package_name(),
                        describe_path_in_context(cx, &candidate.package_id()),
                    )
                })),
            })
            .collect::<CargoResult<Vec<DepInfo>>>()?;

        // Attempt to resolve dependencies with fewer candidates before trying
        // dependencies with more candidates. This way if the dependency with
        // only one candidate can't be resolved we don't have to do a bunch of
        // work before we figure that out.
        deps.sort_by_key(|(_, a, _)| a.len());

        let out = Rc::new((used_features, Rc::new(deps)));

        // If we succeed we add the result to the cache so we can use it again next time.
        // We don't cache the failure cases as they don't impl Clone.
        self.summary_cache.insert(
            (parent, candidate.clone(), opts.clone()),
            (out.clone(), all_ready),
        );

        Ok(out)
    }
}

/// Returns the features we ended up using and
/// all dependencies and the features we want from each of them.
pub fn resolve_features<'b>(
    parent: Option<PackageId>,
    s: &'b Summary,
    opts: &'b ResolveOpts,
) -> ActivateResult<(HashSet<InternedString>, Vec<(Dependency, FeaturesSet)>)> {
    // First, filter by dev-dependencies.
    let deps = s.dependencies();
    let deps = deps.iter().filter(|d| d.is_transitive() || opts.dev_deps);

    let reqs = build_requirements(parent, s, opts)?;
    let mut ret = Vec::new();
    let default_dep = BTreeSet::new();
    let mut valid_dep_names = HashSet::new();

    // Next, collect all actually enabled dependencies and their features.
    for dep in deps {
        // Skip optional dependencies, but not those enabled through a
        // feature
        if dep.is_optional() && !reqs.deps.contains_key(&dep.name_in_toml()) {
            continue;
        }
        valid_dep_names.insert(dep.name_in_toml());
        // So we want this dependency. Move the features we want from
        // `feature_deps` to `ret` and register ourselves as using this
        // name.
        let mut base = reqs
            .deps
            .get(&dep.name_in_toml())
            .unwrap_or(&default_dep)
            .clone();
        base.extend(dep.features().iter());
        ret.push((dep.clone(), Rc::new(base)));
    }

    // This is a special case for command-line `--features
    // dep_name/feat_name` where `dep_name` does not exist. All other
    // validation is done either in `build_requirements` or
    // `build_feature_map`.
    if parent.is_none() {
        for dep_name in reqs.deps.keys() {
            if !valid_dep_names.contains(dep_name) {
                let e = RequirementError::MissingDependency(*dep_name);
                return Err(e.into_activate_error(parent, s));
            }
        }
    }

    Ok((reqs.into_features(), ret))
}

/// Takes requested features for a single package from the input `ResolveOpts` and
/// recurses to find all requested features, dependencies and requested
/// dependency features in a `Requirements` object, returning it to the resolver.
fn build_requirements<'a, 'b: 'a>(
    parent: Option<PackageId>,
    s: &'a Summary,
    opts: &'b ResolveOpts,
) -> ActivateResult<Requirements<'a>> {
    let mut reqs = Requirements::new(s);

    let handle_default = |uses_default_features, reqs: &mut Requirements<'_>| {
        if uses_default_features && s.features().contains_key("default") {
            if let Err(e) = reqs.require_feature(INTERNED_DEFAULT) {
                return Err(e.into_activate_error(parent, s));
            }
        }
        Ok(())
    };

    match &opts.features {
        RequestedFeatures::CliFeatures(CliFeatures {
            features,
            all_features,
            uses_default_features,
        }) => {
            if *all_features {
                for key in s.features().keys() {
                    if let Err(e) = reqs.require_feature(*key) {
                        return Err(e.into_activate_error(parent, s));
                    }
                }
            }

            for fv in features.iter() {
                if let Err(e) = reqs.require_value(fv) {
                    return Err(e.into_activate_error(parent, s));
                }
            }
            handle_default(*uses_default_features, &mut reqs)?;
        }
        RequestedFeatures::DepFeatures {
            features,
            uses_default_features,
        } => {
            for feature in features.iter() {
                if let Err(e) = reqs.require_feature(*feature) {
                    return Err(e.into_activate_error(parent, s));
                }
            }
            handle_default(*uses_default_features, &mut reqs)?;
        }
    }

    Ok(reqs)
}

/// Set of feature and dependency requirements for a package.
#[derive(Debug)]
struct Requirements<'a> {
    summary: &'a Summary,
    /// The deps map is a mapping of dependency name to list of features enabled.
    ///
    /// The resolver will activate all of these dependencies, with the given
    /// features enabled.
    deps: HashMap<InternedString, BTreeSet<InternedString>>,
    /// The set of features enabled on this package which is later used when
    /// compiling to instruct the code what features were enabled.
    features: HashSet<InternedString>,
}

/// An error for a requirement.
///
/// This will later be converted to an `ActivateError` depending on whether or
/// not this is a dependency or a root package.
enum RequirementError {
    /// The package does not have the requested feature.
    MissingFeature(InternedString),
    /// The package does not have the requested dependency.
    MissingDependency(InternedString),
    /// A feature has a direct cycle to itself.
    ///
    /// Note that cycles through multiple features are allowed (but perhaps
    /// they shouldn't be?).
    Cycle(InternedString),
}

impl Requirements<'_> {
    fn new(summary: &Summary) -> Requirements<'_> {
        Requirements {
            summary,
            deps: HashMap::new(),
            features: HashSet::new(),
        }
    }

    fn into_features(self) -> HashSet<InternedString> {
        self.features
    }

    fn require_dep_feature(
        &mut self,
        package: InternedString,
        feat: InternedString,
        weak: bool,
    ) -> Result<(), RequirementError> {
        // If `package` is indeed an optional dependency then we activate the
        // feature named `package`, but otherwise if `package` is a required
        // dependency then there's no feature associated with it.
        if !weak
            && self
                .summary
                .dependencies()
                .iter()
                .any(|dep| dep.name_in_toml() == package && dep.is_optional())
        {
            // This optional dependency may not have an implicit feature of
            // the same name if the `dep:` syntax is used to avoid creating
            // that implicit feature.
            if self.summary.features().contains_key(&package) {
                self.require_feature(package)?;
            }
        }
        self.deps.entry(package).or_default().insert(feat);
        Ok(())
    }

    fn require_dependency(&mut self, pkg: InternedString) {
        self.deps.entry(pkg).or_default();
    }

    fn require_feature(&mut self, feat: InternedString) -> Result<(), RequirementError> {
        if !self.features.insert(feat) {
            // Already seen this feature.
            return Ok(());
        }

        let Some(fvs) = self.summary.features().get(&feat) else {
            return Err(RequirementError::MissingFeature(feat));
        };

        for fv in fvs {
            if let FeatureValue::Feature(dep_feat) = fv {
                if *dep_feat == feat {
                    return Err(RequirementError::Cycle(feat));
                }
            }
            self.require_value(fv)?;
        }
        Ok(())
    }

    fn require_value(&mut self, fv: &FeatureValue) -> Result<(), RequirementError> {
        match fv {
            FeatureValue::Feature(feat) => self.require_feature(*feat)?,
            FeatureValue::Dep { dep_name } => self.require_dependency(*dep_name),
            FeatureValue::DepFeature {
                dep_name,
                dep_feature,
                // Weak features are always activated in the dependency
                // resolver. They will be narrowed inside the new feature
                // resolver.
                weak,
            } => self.require_dep_feature(*dep_name, *dep_feature, *weak)?,
        };
        Ok(())
    }
}

impl RequirementError {
    fn into_activate_error(self, parent: Option<PackageId>, summary: &Summary) -> ActivateError {
        match self {
            RequirementError::MissingFeature(feat) => {
                let deps: Vec<_> = summary
                    .dependencies()
                    .iter()
                    .filter(|dep| dep.name_in_toml() == feat)
                    .collect();
                if deps.is_empty() {
                    return match parent {
                        None => {
                            let closest = closest_msg(
                                &feat.as_str(),
                                summary.features().keys(),
                                |key| &key,
                                "feature",
                            );
                            ActivateError::Fatal(anyhow::format_err!(
                                "package `{}` does not have the feature `{}`{}",
                                summary.package_id(),
                                feat,
                                closest
                            ))
                        }
                        Some(p) => ActivateError::Conflict(p, ConflictReason::MissingFeature(feat)),
                    };
                }
                if deps.iter().any(|dep| dep.is_optional()) {
                    match parent {
                        None => {
                            let mut features =
                                features_enabling_dependency_sorted(summary, feat).peekable();
                            let mut suggestion = String::new();
                            if features.peek().is_some() {
                                suggestion = format!(
                                    "\nDependency `{}` would be enabled by these features:",
                                    feat
                                );
                                for feature in (&mut features).take(3) {
                                    let _ = write!(&mut suggestion, "\n\t- `{}`", feature);
                                }
                                if features.peek().is_some() {
                                    suggestion.push_str("\n\t  ...");
                                }
                            }
                            ActivateError::Fatal(anyhow::format_err!(
                                "\
package `{}` does not have feature `{}`

help: an optional dependency \
with that name exists, but the `features` table includes it with the \"dep:\" \
syntax so it does not have an implicit feature with that name{}",
                                summary.package_id(),
                                feat,
                                suggestion
                            ))
                        }
                        Some(p) => ActivateError::Conflict(
                            p,
                            ConflictReason::NonImplicitDependencyAsFeature(feat),
                        ),
                    }
                } else {
                    match parent {
                        None => ActivateError::Fatal(anyhow::format_err!(
                            "package `{}` does not have feature `{}`

help: a depednency with that name exists but it is required dependency and only optional dependencies can be used as features.",
                            summary.package_id(),
                            feat,
                        )),
                        Some(p) => ActivateError::Conflict(
                            p,
                            ConflictReason::RequiredDependencyAsFeature(feat),
                        ),
                    }
                }
            }
            RequirementError::MissingDependency(dep_name) => {
                match parent {
                    None => ActivateError::Fatal(anyhow::format_err!(
                        "package `{}` does not have a dependency named `{}`",
                        summary.package_id(),
                        dep_name
                    )),
                    // This code path currently isn't used, since `foo/bar`
                    // and `dep:` syntax is not allowed in a dependency.
                    Some(p) => ActivateError::Conflict(p, ConflictReason::MissingFeature(dep_name)),
                }
            }
            RequirementError::Cycle(feat) => ActivateError::Fatal(anyhow::format_err!(
                "cyclic feature dependency: feature `{}` depends on itself",
                feat
            )),
        }
    }
}

/// Collect any features which enable the optional dependency "target_dep".
///
/// The returned value will be sorted.
fn features_enabling_dependency_sorted(
    summary: &Summary,
    target_dep: InternedString,
) -> impl Iterator<Item = InternedString> + '_ {
    let iter = summary
        .features()
        .iter()
        .filter(move |(_, values)| {
            for value in *values {
                match value {
                    FeatureValue::Dep { dep_name }
                    | FeatureValue::DepFeature {
                        dep_name,
                        weak: false,
                        ..
                    } if dep_name == &target_dep => return true,
                    _ => (),
                }
            }
            false
        })
        .map(|(name, _)| *name);
    // iter is already sorted because it was constructed from a BTreeMap.
    iter
}
