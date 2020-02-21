//! Feature resolver.
//!
//! This is a new feature resolver that runs independently of the main
//! dependency resolver. It is intended to make it easier to experiment with
//! new behaviors. When `-Zfeatures` is not used, it will fall back to using
//! the original `Resolve` feature computation. With `-Zfeatures` enabled,
//! this will walk the dependency graph and compute the features using a
//! different algorithm.
//!
//! One of its key characteristics is that it can avoid unifying features for
//! shared dependencies in some situations. See `FeatureOpts` for the
//! different behaviors that can be enabled. If no extra options are enabled,
//! then it should behave exactly the same as the dependency resolver's
//! feature resolution. This can be verified by setting the
//! `__CARGO_FORCE_NEW_FEATURES=compare` environment variable and running
//! Cargo's test suite (or building other projects), and checking if it
//! panics. Note: the `features2` tests will fail because they intentionally
//! compare the old vs new behavior, so forcing the old behavior will
//! naturally fail the tests.
//!
//! The preferred way to engage this new resolver is via
//! `resolve_ws_with_opts`.
//!
//! This does not *replace* feature resolution in the dependency resolver, but
//! instead acts as a second pass which can *narrow* the features selected in
//! the dependency resolver. The dependency resolver still needs to do its own
//! feature resolution in order to avoid selecting optional dependencies that
//! are never enabled. The dependency resolver could, in theory, just assume
//! all optional dependencies on all packages are enabled (and remove all
//! knowledge of features), but that could introduce new requirements that
//! might change old behavior or cause conflicts. Maybe some day in the future
//! we could experiment with that, but it seems unlikely to work or be all
//! that helpful.
//!
//! There are many assumptions made about the dependency resolver. This
//! feature resolver assumes validation has already been done on the feature
//! maps, and doesn't do any validation itself. It assumes dev-dependencies
//! within a dependency have been removed. There are probably other
//! assumptions that I am forgetting.

use crate::core::compiler::{CompileKind, RustcTargetData};
use crate::core::dependency::{DepKind, Dependency};
use crate::core::resolver::types::FeaturesSet;
use crate::core::resolver::Resolve;
use crate::core::{FeatureValue, InternedString, PackageId, PackageIdSpec, Workspace};
use crate::util::{CargoResult, Config};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::rc::Rc;

/// Map of activated features.
///
/// The key is `(PackageId, bool)` where the bool is `true` if these
/// are features for a build dependency.
type ActivateMap = HashMap<(PackageId, bool), BTreeSet<InternedString>>;

/// Set of all activated features for all packages in the resolve graph.
pub struct ResolvedFeatures {
    activated_features: ActivateMap,
    /// This is only here for legacy support when `-Zfeatures` is not enabled.
    legacy: Option<HashMap<PackageId, Vec<InternedString>>>,
    opts: FeatureOpts,
}

/// Options for how the feature resolver works.
#[derive(Default)]
struct FeatureOpts {
    /// -Zpackage-features, changes behavior of feature flags in a workspace.
    package_features: bool,
    /// -Zfeatures is enabled, use new resolver.
    new_resolver: bool,
    /// Build deps will not share share features with other dep kinds.
    decouple_build_deps: bool,
    /// Dev dep features will not be activated unless needed.
    decouple_dev_deps: bool,
    /// Targets that are not in use will not activate features.
    ignore_inactive_targets: bool,
    /// If enabled, compare against old resolver (for testing).
    compare: bool,
}

/// Flag to indicate if Cargo is building *any* dev units (tests, examples, etc.).
///
/// This disables decoupling of dev dependencies. It may be possible to relax
/// this in the future, but it will require significant changes to how unit
/// dependencies are computed, and can result in longer build times with
/// `cargo test` because the lib may need to be built 3 times instead of
/// twice.
pub enum HasDevUnits {
    Yes,
    No,
}

/// Flag to indicate if features are requested for a build dependency or not.
#[derive(PartialEq)]
pub enum FeaturesFor {
    NormalOrDev,
    BuildDep,
}

impl FeatureOpts {
    fn new(config: &Config, has_dev_units: HasDevUnits) -> CargoResult<FeatureOpts> {
        let mut opts = FeatureOpts::default();
        let unstable_flags = config.cli_unstable();
        opts.package_features = unstable_flags.package_features;
        let mut enable = |feat_opts: &Vec<String>| {
            opts.new_resolver = true;
            for opt in feat_opts {
                match opt.as_ref() {
                    "build_dep" => opts.decouple_build_deps = true,
                    "dev_dep" => opts.decouple_dev_deps = true,
                    "itarget" => opts.ignore_inactive_targets = true,
                    "all" => {
                        opts.decouple_build_deps = true;
                        opts.decouple_dev_deps = true;
                        opts.ignore_inactive_targets = true;
                    }
                    "compare" => opts.compare = true,
                    "ws" => unimplemented!(),
                    "host" => unimplemented!(),
                    s => anyhow::bail!("-Zfeatures flag `{}` is not supported", s),
                }
            }
            Ok(())
        };
        if let Some(feat_opts) = unstable_flags.features.as_ref() {
            enable(feat_opts)?;
        }
        // This env var is intended for testing only.
        if let Ok(env_opts) = std::env::var("__CARGO_FORCE_NEW_FEATURES") {
            if env_opts == "1" {
                opts.new_resolver = true;
            } else {
                let env_opts = env_opts.split(',').map(|s| s.to_string()).collect();
                enable(&env_opts)?;
            }
        }
        if let HasDevUnits::Yes = has_dev_units {
            opts.decouple_dev_deps = false;
        }
        Ok(opts)
    }
}

/// Features flags requested for a package.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct RequestedFeatures {
    pub features: FeaturesSet,
    pub all_features: bool,
    pub uses_default_features: bool,
}

impl RequestedFeatures {
    /// Creates a new RequestedFeatures from the given command-line flags.
    pub fn from_command_line(
        features: &[String],
        all_features: bool,
        uses_default_features: bool,
    ) -> RequestedFeatures {
        RequestedFeatures {
            features: Rc::new(RequestedFeatures::split_features(features)),
            all_features,
            uses_default_features,
        }
    }

    /// Creates a new RequestedFeatures with the given `all_features` setting.
    pub fn new_all(all_features: bool) -> RequestedFeatures {
        RequestedFeatures {
            features: Rc::new(BTreeSet::new()),
            all_features,
            uses_default_features: true,
        }
    }

    fn split_features(features: &[String]) -> BTreeSet<InternedString> {
        features
            .iter()
            .flat_map(|s| s.split_whitespace())
            .flat_map(|s| s.split(','))
            .filter(|s| !s.is_empty())
            .map(InternedString::new)
            .collect::<BTreeSet<InternedString>>()
    }
}

impl ResolvedFeatures {
    /// Returns the list of features that are enabled for the given package.
    pub fn activated_features(
        &self,
        pkg_id: PackageId,
        features_for: FeaturesFor,
    ) -> Vec<InternedString> {
        self.activated_features_int(pkg_id, features_for, true)
    }

    /// Variant of `activated_features` that returns an empty Vec if this is
    /// not a valid pkg_id/is_build combination. Used by `cargo clean` which
    /// doesn't know the exact set.
    pub fn activated_features_unverified(
        &self,
        pkg_id: PackageId,
        features_for: FeaturesFor,
    ) -> Vec<InternedString> {
        self.activated_features_int(pkg_id, features_for, false)
    }

    fn activated_features_int(
        &self,
        pkg_id: PackageId,
        features_for: FeaturesFor,
        verify: bool,
    ) -> Vec<InternedString> {
        if let Some(legacy) = &self.legacy {
            legacy.get(&pkg_id).map_or_else(Vec::new, |v| v.clone())
        } else {
            let is_build = self.opts.decouple_build_deps && features_for == FeaturesFor::BuildDep;
            if let Some(fs) = self.activated_features.get(&(pkg_id, is_build)) {
                fs.iter().cloned().collect()
            } else if verify {
                panic!("features did not find {:?} {:?}", pkg_id, is_build)
            } else {
                Vec::new()
            }
        }
    }
}

pub struct FeatureResolver<'a, 'cfg> {
    ws: &'a Workspace<'cfg>,
    target_data: &'a RustcTargetData,
    /// The platform to build for, requested by the user.
    requested_target: CompileKind,
    resolve: &'a Resolve,
    /// Options that change how the feature resolver operates.
    opts: FeatureOpts,
    /// Map of features activated for each package.
    activated_features: ActivateMap,
    /// Keeps track of which packages have had its dependencies processed.
    /// Used to avoid cycles, and to speed up processing.
    processed_deps: HashSet<(PackageId, bool)>,
}

impl<'a, 'cfg> FeatureResolver<'a, 'cfg> {
    /// Runs the resolution algorithm and returns a new `ResolvedFeatures`
    /// with the result.
    pub fn resolve(
        ws: &Workspace<'cfg>,
        target_data: &RustcTargetData,
        resolve: &Resolve,
        requested_features: &RequestedFeatures,
        specs: &[PackageIdSpec],
        requested_target: CompileKind,
        has_dev_units: HasDevUnits,
    ) -> CargoResult<ResolvedFeatures> {
        use crate::util::profile;
        let _p = profile::start("resolve features");

        let opts = FeatureOpts::new(ws.config(), has_dev_units)?;
        if !opts.new_resolver {
            // Legacy mode.
            return Ok(ResolvedFeatures {
                activated_features: HashMap::new(),
                legacy: Some(resolve.features_clone()),
                opts,
            });
        }
        let mut r = FeatureResolver {
            ws,
            target_data,
            requested_target,
            resolve,
            opts,
            activated_features: HashMap::new(),
            processed_deps: HashSet::new(),
        };
        r.do_resolve(specs, requested_features)?;
        log::debug!("features={:#?}", r.activated_features);
        if r.opts.compare {
            r.compare();
        }
        Ok(ResolvedFeatures {
            activated_features: r.activated_features,
            legacy: None,
            opts: r.opts,
        })
    }

    /// Performs the process of resolving all features for the resolve graph.
    fn do_resolve(
        &mut self,
        specs: &[PackageIdSpec],
        requested_features: &RequestedFeatures,
    ) -> CargoResult<()> {
        let member_features = self.ws.members_with_features(specs, requested_features)?;
        for (member, requested_features) in &member_features {
            let fvs = self.fvs_from_requested(member.package_id(), requested_features);
            self.activate_pkg(member.package_id(), &fvs, false)?;
        }
        Ok(())
    }

    fn activate_pkg(
        &mut self,
        pkg_id: PackageId,
        fvs: &[FeatureValue],
        for_build: bool,
    ) -> CargoResult<()> {
        // Add an empty entry to ensure everything is covered. This is intended for
        // finding bugs where the resolver missed something it should have visited.
        // Remove this in the future if `activated_features` uses an empty default.
        self.activated_features
            .entry((pkg_id, for_build))
            .or_insert_with(BTreeSet::new);
        for fv in fvs {
            self.activate_fv(pkg_id, fv, for_build)?;
        }
        if !self.processed_deps.insert((pkg_id, for_build)) {
            // Already processed dependencies. There's no need to process them
            // again. This is primarily to avoid cycles, but also helps speed
            // things up.
            //
            // This is safe because if another package comes along and adds a
            // feature on this package, it will immediately add it (in
            // `activate_fv`), and recurse as necessary right then and there.
            // For example, consider we've already processed our dependencies,
            // and another package comes along and enables one of our optional
            // dependencies, it will do so immediately in the
            // `FeatureValue::CrateFeature` branch, and then immediately
            // recurse into that optional dependency. This also holds true for
            // features that enable other features.
            return Ok(());
        }
        for (dep_pkg_id, deps) in self.deps(pkg_id, for_build) {
            for (dep, dep_for_build) in deps {
                if dep.is_optional() {
                    // Optional dependencies are enabled in `activate_fv` when
                    // a feature enables it.
                    continue;
                }
                // Recurse into the dependency.
                let fvs = self.fvs_from_dependency(dep_pkg_id, dep);
                self.activate_pkg(dep_pkg_id, &fvs, dep_for_build)?;
            }
        }
        Ok(())
    }

    /// Activate a single FeatureValue for a package.
    fn activate_fv(
        &mut self,
        pkg_id: PackageId,
        fv: &FeatureValue,
        for_build: bool,
    ) -> CargoResult<()> {
        match fv {
            FeatureValue::Feature(f) => {
                self.activate_rec(pkg_id, *f, for_build)?;
            }
            FeatureValue::Crate(dep_name) => {
                // Activate the feature name on self.
                self.activate_rec(pkg_id, *dep_name, for_build)?;
                // Activate the optional dep.
                for (dep_pkg_id, deps) in self.deps(pkg_id, for_build) {
                    for (dep, dep_for_build) in deps {
                        if dep.name_in_toml() != *dep_name {
                            continue;
                        }
                        let fvs = self.fvs_from_dependency(dep_pkg_id, dep);
                        self.activate_pkg(dep_pkg_id, &fvs, dep_for_build)?;
                    }
                }
            }
            FeatureValue::CrateFeature(dep_name, dep_feature) => {
                // Activate a feature within a dependency.
                for (dep_pkg_id, deps) in self.deps(pkg_id, for_build) {
                    for (dep, dep_for_build) in deps {
                        if dep.name_in_toml() != *dep_name {
                            continue;
                        }
                        if dep.is_optional() {
                            // Activate the crate on self.
                            let fv = FeatureValue::Crate(*dep_name);
                            self.activate_fv(pkg_id, &fv, for_build)?;
                        }
                        // Activate the feature on the dependency.
                        let summary = self.resolve.summary(dep_pkg_id);
                        let fv = FeatureValue::new(*dep_feature, summary);
                        self.activate_fv(dep_pkg_id, &fv, dep_for_build)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Activate the given feature for the given package, and then recursively
    /// activate any other features that feature enables.
    fn activate_rec(
        &mut self,
        pkg_id: PackageId,
        feature_to_enable: InternedString,
        for_build: bool,
    ) -> CargoResult<()> {
        let enabled = self
            .activated_features
            .entry((pkg_id, for_build))
            .or_insert_with(BTreeSet::new);
        if !enabled.insert(feature_to_enable) {
            // Already enabled.
            return Ok(());
        }
        let summary = self.resolve.summary(pkg_id);
        let feature_map = summary.features();
        let fvs = match feature_map.get(&feature_to_enable) {
            Some(fvs) => fvs,
            None => {
                // TODO: this should only happen for optional dependencies.
                // Other cases should be validated by Summary's `build_feature_map`.
                // Figure out some way to validate this assumption.
                log::debug!(
                    "pkg {:?} does not define feature {}",
                    pkg_id,
                    feature_to_enable
                );
                return Ok(());
            }
        };
        for fv in fvs {
            self.activate_fv(pkg_id, fv, for_build)?;
        }
        Ok(())
    }

    /// Returns Vec of FeatureValues from a Dependency definition.
    fn fvs_from_dependency(&self, dep_id: PackageId, dep: &Dependency) -> Vec<FeatureValue> {
        let summary = self.resolve.summary(dep_id);
        let feature_map = summary.features();
        let mut result: Vec<FeatureValue> = dep
            .features()
            .iter()
            .map(|f| FeatureValue::new(*f, summary))
            .collect();
        let default = InternedString::new("default");
        if dep.uses_default_features() && feature_map.contains_key(&default) {
            result.push(FeatureValue::Feature(default));
        }
        result
    }

    /// Returns Vec of FeatureValues from a set of command-line features.
    fn fvs_from_requested(
        &self,
        pkg_id: PackageId,
        requested_features: &RequestedFeatures,
    ) -> Vec<FeatureValue> {
        let summary = self.resolve.summary(pkg_id);
        let feature_map = summary.features();
        if requested_features.all_features {
            let mut fvs: Vec<FeatureValue> = feature_map
                .keys()
                .map(|k| FeatureValue::Feature(*k))
                .collect();
            // Add optional deps.
            // Top-level requested features can never apply to
            // build-dependencies, so for_build is `false` here.
            for (_dep_pkg_id, deps) in self.deps(pkg_id, false) {
                for (dep, _dep_for_build) in deps {
                    if dep.is_optional() {
                        // This may result in duplicates, but that should be ok.
                        fvs.push(FeatureValue::Crate(dep.name_in_toml()));
                    }
                }
            }
            fvs
        } else {
            let mut result: Vec<FeatureValue> = requested_features
                .features
                .as_ref()
                .iter()
                .map(|f| FeatureValue::new(*f, summary))
                .collect();
            let default = InternedString::new("default");
            if requested_features.uses_default_features && feature_map.contains_key(&default) {
                result.push(FeatureValue::Feature(default));
            }
            result
        }
    }

    /// Returns the dependencies for a package, filtering out inactive targets.
    fn deps(
        &self,
        pkg_id: PackageId,
        for_build: bool,
    ) -> Vec<(PackageId, Vec<(&'a Dependency, bool)>)> {
        // Helper for determining if a platform is activated.
        let platform_activated = |dep: &Dependency| -> bool {
            // We always care about build-dependencies, and they are always
            // Host. If we are computing dependencies "for a build script",
            // even normal dependencies are host-only.
            if for_build || dep.is_build() {
                return self
                    .target_data
                    .dep_platform_activated(dep, CompileKind::Host);
            }
            // Not a build dependency, and not for a build script, so must be Target.
            self.target_data
                .dep_platform_activated(dep, self.requested_target)
        };
        self.resolve
            .deps(pkg_id)
            .map(|(dep_id, deps)| {
                let deps = deps
                    .iter()
                    .filter(|dep| {
                        if dep.platform().is_some()
                            && self.opts.ignore_inactive_targets
                            && !platform_activated(dep)
                        {
                            return false;
                        }
                        if self.opts.decouple_dev_deps && dep.kind() == DepKind::Development {
                            return false;
                        }
                        true
                    })
                    .map(|dep| {
                        let dep_for_build =
                            for_build || (self.opts.decouple_build_deps && dep.is_build());
                        (dep, dep_for_build)
                    })
                    .collect::<Vec<_>>();
                (dep_id, deps)
            })
            .filter(|(_id, deps)| !deps.is_empty())
            .collect()
    }

    /// Compare the activated features to the resolver. Used for testing.
    fn compare(&self) {
        let mut found = false;
        for ((pkg_id, dep_kind), features) in &self.activated_features {
            let r_features = self.resolve.features(*pkg_id);
            if !r_features.iter().eq(features.iter()) {
                eprintln!(
                    "{}/{:?} features mismatch\nresolve: {:?}\nnew: {:?}\n",
                    pkg_id, dep_kind, r_features, features
                );
                found = true;
            }
        }
        if found {
            panic!("feature mismatch");
        }
    }
}
