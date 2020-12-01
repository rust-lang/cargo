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

use crate::core::resolver::context::Context;
use crate::core::resolver::errors::describe_path;
use crate::core::resolver::types::{ConflictReason, DepInfo, FeaturesSet};
use crate::core::resolver::{ActivateError, ActivateResult, ResolveOpts};
use crate::core::{Dependency, FeatureValue, PackageId, PackageIdSpec, Registry, Summary};
use crate::core::{GitReference, SourceId};
use crate::util::errors::{CargoResult, CargoResultExt};
use crate::util::interning::InternedString;
use crate::util::Config;
use log::debug;
use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::rc::Rc;

pub struct RegistryQueryer<'a> {
    pub registry: &'a mut (dyn Registry + 'a),
    replacements: &'a [(PackageIdSpec, Dependency)],
    try_to_use: &'a HashSet<PackageId>,
    /// If set the list of dependency candidates will be sorted by minimal
    /// versions first. That allows `cargo update -Z minimal-versions` which will
    /// specify minimum dependency versions to be used.
    minimal_versions: bool,
    /// a cache of `Candidate`s that fulfil a `Dependency`
    registry_cache: HashMap<Dependency, Rc<Vec<Summary>>>,
    /// a cache of `Dependency`s that are required for a `Summary`
    summary_cache: HashMap<
        (Option<PackageId>, Summary, ResolveOpts),
        Rc<(HashSet<InternedString>, Rc<Vec<DepInfo>>)>,
    >,
    /// all the cases we ended up using a supplied replacement
    used_replacements: HashMap<PackageId, Summary>,
    /// Where to print warnings, if configured.
    config: Option<&'a Config>,
    /// Sources that we've already wared about possibly colliding in the future.
    warned_git_collisions: HashSet<SourceId>,
}

impl<'a> RegistryQueryer<'a> {
    pub fn new(
        registry: &'a mut dyn Registry,
        replacements: &'a [(PackageIdSpec, Dependency)],
        try_to_use: &'a HashSet<PackageId>,
        minimal_versions: bool,
        config: Option<&'a Config>,
    ) -> Self {
        RegistryQueryer {
            registry,
            replacements,
            try_to_use,
            minimal_versions,
            registry_cache: HashMap::new(),
            summary_cache: HashMap::new(),
            used_replacements: HashMap::new(),
            config,
            warned_git_collisions: HashSet::new(),
        }
    }

    pub fn used_replacement_for(&self, p: PackageId) -> Option<(PackageId, PackageId)> {
        self.used_replacements.get(&p).map(|r| (p, r.package_id()))
    }

    pub fn replacement_summary(&self, p: PackageId) -> Option<&Summary> {
        self.used_replacements.get(&p)
    }

    /// Issues a future-compatible warning targeted at removing reliance on
    /// unifying behavior between these two dependency directives:
    ///
    /// ```toml
    /// [dependencies]
    /// a = { git = 'https://example.org/foo' }
    /// a = { git = 'https://example.org/foo', branch = 'master }
    /// ```
    ///
    /// Historical versions of Cargo considered these equivalent but going
    /// forward we'd like to fix this. For more details see the comments in
    /// src/cargo/sources/git/utils.rs
    fn warn_colliding_git_sources(&mut self, id: SourceId) -> CargoResult<()> {
        let config = match self.config {
            Some(config) => config,
            None => return Ok(()),
        };
        let prev = match self.warned_git_collisions.replace(id) {
            Some(prev) => prev,
            None => return Ok(()),
        };
        match (id.git_reference(), prev.git_reference()) {
            (Some(GitReference::DefaultBranch), Some(GitReference::Branch(b)))
            | (Some(GitReference::Branch(b)), Some(GitReference::DefaultBranch))
                if b == "master" => {}
            _ => return Ok(()),
        }

        config.shell().warn(&format!(
            "two git dependencies found for `{}` \
             where one uses `branch = \"master\"` and the other doesn't; \
             this will break in a future version of Cargo, so please \
             ensure the dependency forms are consistent",
            id.url(),
        ))?;
        Ok(())
    }

    /// Queries the `registry` to return a list of candidates for `dep`.
    ///
    /// This method is the location where overrides are taken into account. If
    /// any candidates are returned which match an override then the override is
    /// applied by performing a second query for what the override should
    /// return.
    pub fn query(&mut self, dep: &Dependency) -> CargoResult<Rc<Vec<Summary>>> {
        self.warn_colliding_git_sources(dep.source_id())?;
        if let Some(out) = self.registry_cache.get(dep).cloned() {
            return Ok(out);
        }

        let mut ret = Vec::new();
        self.registry.query(
            dep,
            &mut |s| {
                ret.push(s);
            },
            false,
        )?;
        for summary in ret.iter_mut() {
            let mut potential_matches = self
                .replacements
                .iter()
                .filter(|&&(ref spec, _)| spec.matches(summary.package_id()));

            let &(ref spec, ref dep) = match potential_matches.next() {
                None => continue,
                Some(replacement) => replacement,
            };
            debug!(
                "found an override for {} {}",
                dep.package_name(),
                dep.version_req()
            );

            let mut summaries = self.registry.query_vec(dep, false)?.into_iter();
            let s = summaries.next().ok_or_else(|| {
                anyhow::format_err!(
                    "no matching package for override `{}` found\n\
                     location searched: {}\n\
                     version required: {}",
                    spec,
                    dep.source_id(),
                    dep.version_req()
                )
            })?;
            let summaries = summaries.collect::<Vec<_>>();
            if !summaries.is_empty() {
                let bullets = summaries
                    .iter()
                    .map(|s| format!("  * {}", s.package_id()))
                    .collect::<Vec<_>>();
                anyhow::bail!(
                    "the replacement specification `{}` matched \
                     multiple packages:\n  * {}\n{}",
                    spec,
                    s.package_id(),
                    bullets.join("\n")
                );
            }

            // The dependency should be hard-coded to have the same name and an
            // exact version requirement, so both of these assertions should
            // never fail.
            assert_eq!(s.version(), summary.version());
            assert_eq!(s.name(), summary.name());

            let replace = if s.source_id() == summary.source_id() {
                debug!("Preventing\n{:?}\nfrom replacing\n{:?}", summary, s);
                None
            } else {
                Some(s)
            };
            let matched_spec = spec.clone();

            // Make sure no duplicates
            if let Some(&(ref spec, _)) = potential_matches.next() {
                anyhow::bail!(
                    "overlapping replacement specifications found:\n\n  \
                     * {}\n  * {}\n\nboth specifications match: {}",
                    matched_spec,
                    spec,
                    summary.package_id()
                );
            }

            for dep in summary.dependencies() {
                debug!("\t{} => {}", dep.package_name(), dep.version_req());
            }
            if let Some(r) = replace {
                self.used_replacements.insert(summary.package_id(), r);
            }
        }

        // When we attempt versions for a package we'll want to do so in a
        // sorted fashion to pick the "best candidates" first. Currently we try
        // prioritized summaries (those in `try_to_use`) and failing that we
        // list everything from the maximum version to the lowest version.
        ret.sort_unstable_by(|a, b| {
            let a_in_previous = self.try_to_use.contains(&a.package_id());
            let b_in_previous = self.try_to_use.contains(&b.package_id());
            let previous_cmp = a_in_previous.cmp(&b_in_previous).reverse();
            match previous_cmp {
                Ordering::Equal => {
                    let cmp = a.version().cmp(b.version());
                    if self.minimal_versions {
                        // Lower version ordered first.
                        cmp
                    } else {
                        // Higher version ordered first.
                        cmp.reverse()
                    }
                }
                _ => previous_cmp,
            }
        });

        let out = Rc::new(ret);

        self.registry_cache.insert(dep.clone(), out.clone());

        Ok(out)
    }

    /// Find out what dependencies will be added by activating `candidate`,
    /// with features described in `opts`. Then look up in the `registry`
    /// the candidates that will fulfil each of these dependencies, as it is the
    /// next obvious question.
    pub fn build_deps(
        &mut self,
        cx: &Context,
        parent: Option<PackageId>,
        candidate: &Summary,
        opts: &ResolveOpts,
    ) -> ActivateResult<Rc<(HashSet<InternedString>, Rc<Vec<DepInfo>>)>> {
        // if we have calculated a result before, then we can just return it,
        // as it is a "pure" query of its arguments.
        if let Some(out) = self
            .summary_cache
            .get(&(parent, candidate.clone(), opts.clone()))
            .cloned()
        {
            return Ok(out);
        }
        // First, figure out our set of dependencies based on the requested set
        // of features. This also calculates what features we're going to enable
        // for our own dependencies.
        let (used_features, deps) = resolve_features(parent, candidate, opts)?;

        // Next, transform all dependencies into a list of possible candidates
        // which can satisfy that dependency.
        let mut deps = deps
            .into_iter()
            .map(|(dep, features)| {
                let candidates = self.query(&dep).chain_err(|| {
                    anyhow::format_err!(
                        "failed to get `{}` as a dependency of {}",
                        dep.package_name(),
                        describe_path(&cx.parents.path_to_bottom(&candidate.package_id())),
                    )
                })?;
                Ok((dep, candidates, features))
            })
            .collect::<CargoResult<Vec<DepInfo>>>()?;

        // Attempt to resolve dependencies with fewer candidates before trying
        // dependencies with more candidates. This way if the dependency with
        // only one candidate can't be resolved we don't have to do a bunch of
        // work before we figure that out.
        deps.sort_by_key(|&(_, ref a, _)| a.len());

        let out = Rc::new((used_features, Rc::new(deps)));

        // If we succeed we add the result to the cache so we can use it again next time.
        // We don't cache the failure cases as they don't impl Clone.
        self.summary_cache
            .insert((parent, candidate.clone(), opts.clone()), out.clone());

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
        for feature in base.iter() {
            if feature.contains('/') {
                return Err(anyhow::format_err!(
                    "feature names may not contain slashes: `{}`",
                    feature
                )
                .into());
            }
        }
        ret.push((dep.clone(), Rc::new(base)));
    }

    // This is a special case for command-line `--features
    // dep_name/feat_name` where `dep_name` does not exist. All other
    // validation is done either in `build_requirements` or
    // `build_feature_map`.
    for dep_name in reqs.deps.keys() {
        if !valid_dep_names.contains(dep_name) {
            let e = RequirementError::MissingDependency(*dep_name);
            return Err(e.into_activate_error(parent, s));
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

    if opts.features.all_features {
        for key in s.features().keys() {
            if let Err(e) = reqs.require_feature(*key) {
                return Err(e.into_activate_error(parent, s));
            }
        }
    } else {
        for &f in opts.features.features.iter() {
            let fv = FeatureValue::new(f);
            if fv.has_dep_prefix() {
                return Err(ActivateError::Fatal(anyhow::format_err!(
                    "feature value `{}` is not allowed to use explicit `dep:` syntax",
                    fv
                )));
            }
            if let Err(e) = reqs.require_value(&fv) {
                return Err(e.into_activate_error(parent, s));
            }
        }
    }

    if opts.features.uses_default_features && s.features().contains_key("default") {
        if let Err(e) = reqs.require_feature(InternedString::new("default")) {
            return Err(e.into_activate_error(parent, s));
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
        dep_prefix: bool,
    ) -> Result<(), RequirementError> {
        // If `package` is indeed an optional dependency then we activate the
        // feature named `package`, but otherwise if `package` is a required
        // dependency then there's no feature associated with it.
        if !dep_prefix
            && self
                .summary
                .dependencies()
                .iter()
                .any(|dep| dep.name_in_toml() == package && dep.is_optional())
        {
            self.require_feature(package)?;
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

        let fvs = match self.summary.features().get(&feat) {
            Some(fvs) => fvs,
            None => return Err(RequirementError::MissingFeature(feat)),
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
                dep_prefix,
                // Weak features are always activated in the dependency
                // resolver. They will be narrowed inside the new feature
                // resolver.
                weak: _,
            } => self.require_dep_feature(*dep_name, *dep_feature, *dep_prefix)?,
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
                        None => ActivateError::Fatal(anyhow::format_err!(
                            "Package `{}` does not have the feature `{}`",
                            summary.package_id(),
                            feat
                        )),
                        Some(p) => ActivateError::Conflict(
                            p,
                            ConflictReason::MissingFeatures(feat.to_string()),
                        ),
                    };
                }
                if deps.iter().any(|dep| dep.is_optional()) {
                    match parent {
                        None => ActivateError::Fatal(anyhow::format_err!(
                            "Package `{}` does not have feature `{}`. It has an optional dependency \
                             with that name, but that dependency uses the \"dep:\" \
                             syntax in the features table, so it does not have an implicit feature with that name.",
                            summary.package_id(),
                            feat
                        )),
                        Some(p) => ActivateError::Conflict(
                            p,
                            ConflictReason::NonImplicitDependencyAsFeature(feat),
                        ),
                    }
                } else {
                    match parent {
                        None => ActivateError::Fatal(anyhow::format_err!(
                            "Package `{}` does not have feature `{}`. It has a required dependency \
                             with that name, but only optional dependencies can be used as features.",
                            summary.package_id(),
                            feat
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
                    Some(p) => ActivateError::Conflict(
                        p,
                        ConflictReason::MissingFeatures(dep_name.to_string()),
                    ),
                }
            }
            RequirementError::Cycle(feat) => ActivateError::Fatal(anyhow::format_err!(
                "cyclic feature dependency: feature `{}` depends on itself",
                feat
            )),
        }
    }
}
