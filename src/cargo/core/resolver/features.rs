//! Resolves conditional compilation for [`features` section] in the manifest.
//!
//! This is a [new feature resolver] that runs independently of the main
//! dependency resolver. It has several options which can enable new feature
//! resolution behavior.
//!
//! One of its key characteristics is that it can avoid unifying features for
//! shared dependencies in some situations. See [`FeatureOpts`] for the
//! different behaviors that can be enabled. If no extra options are enabled,
//! then it should behave exactly the same as the dependency resolver's
//! feature resolution.
//!
//! The preferred way to engage this new resolver is via [`resolve_ws_with_opts`].
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
//! ## Assumptions
//!
//! There are many assumptions made about the dependency resolver:
//!
//! * Assumes feature validation has already been done during the construction
//!   of feature maps, so the feature resolver doesn't do that validation at all.
//! * Assumes `dev-dependencies` within a dependency have been removed
//!   in the given [`Resolve`].
//!
//! There are probably other assumptions that I am forgetting.
//!
//! [`features` section]: https://doc.rust-lang.org/nightly/cargo/reference/features.html
//! [new feature resolver]: https://doc.rust-lang.org/nightly/cargo/reference/resolver.html#feature-resolver-version-2
//! [`resolve_ws_with_opts`]: crate::ops::resolve_ws_with_opts

use crate::core::compiler::{CompileKind, CompileTarget, RustcTargetData};
use crate::core::dependency::{ArtifactTarget, DepKind, Dependency};
use crate::core::resolver::types::FeaturesSet;
use crate::core::resolver::{Resolve, ResolveBehavior};
use crate::core::{FeatureValue, PackageId, PackageIdSpec, PackageSet, Workspace};
use crate::util::interning::InternedString;
use crate::util::CargoResult;
use anyhow::{bail, Context};
use itertools::Itertools;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::rc::Rc;

/// The key used in various places to store features for a particular dependency.
/// The actual discrimination happens with the [`FeaturesFor`] type.
type PackageFeaturesKey = (PackageId, FeaturesFor);
/// Map of activated features.
type ActivateMap = HashMap<PackageFeaturesKey, BTreeSet<InternedString>>;

/// Set of all activated features for all packages in the resolve graph.
pub struct ResolvedFeatures {
    activated_features: ActivateMap,
    /// Optional dependencies that should be built.
    ///
    /// The value is the `name_in_toml` of the dependencies.
    activated_dependencies: ActivateMap,
    opts: FeatureOpts,
}

/// Options for how the feature resolver works.
#[derive(Default)]
pub struct FeatureOpts {
    /// Build deps and proc-macros will not share features with other dep kinds,
    /// and so won't artifact targets.
    /// In other terms, if true, features associated with certain kinds of dependencies
    /// will only be unified together.
    /// If false, there is only one namespace for features, unifying all features across
    /// all dependencies, no matter what kind.
    decouple_host_deps: bool,
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
#[derive(Copy, Clone, PartialEq)]
pub enum HasDevUnits {
    Yes,
    No,
}

/// Flag to indicate that target-specific filtering should be disabled.
#[derive(Copy, Clone, PartialEq)]
pub enum ForceAllTargets {
    Yes,
    No,
}

/// Flag to indicate if features are requested for a certain type of dependency.
///
/// This is primarily used for constructing a [`PackageFeaturesKey`] to decouple
/// activated features of the same package with different types of dependency.
#[derive(Default, Copy, Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub enum FeaturesFor {
    /// Normal or dev dependency.
    #[default]
    NormalOrDev,
    /// Build dependency or proc-macro.
    HostDep,
    /// Any dependency with both artifact and target specified.
    ///
    /// That is, `dep = { …, artifact = <crate-type>, target = <triple> }`
    ArtifactDep(CompileTarget),
}

impl std::fmt::Display for FeaturesFor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FeaturesFor::HostDep => f.write_str("host"),
            FeaturesFor::ArtifactDep(target) => f.write_str(&target.rustc_target()),
            FeaturesFor::NormalOrDev => Ok(()),
        }
    }
}

impl FeaturesFor {
    pub fn from_for_host(for_host: bool) -> FeaturesFor {
        if for_host {
            FeaturesFor::HostDep
        } else {
            FeaturesFor::NormalOrDev
        }
    }

    pub fn from_for_host_or_artifact_target(
        for_host: bool,
        artifact_target: Option<CompileTarget>,
    ) -> FeaturesFor {
        match artifact_target {
            Some(target) => FeaturesFor::ArtifactDep(target),
            None => {
                if for_host {
                    FeaturesFor::HostDep
                } else {
                    FeaturesFor::NormalOrDev
                }
            }
        }
    }

    fn apply_opts(self, opts: &FeatureOpts) -> Self {
        if opts.decouple_host_deps {
            self
        } else {
            FeaturesFor::default()
        }
    }
}

impl FeatureOpts {
    pub fn new(
        ws: &Workspace<'_>,
        has_dev_units: HasDevUnits,
        force_all_targets: ForceAllTargets,
    ) -> CargoResult<FeatureOpts> {
        let mut opts = FeatureOpts::default();
        let unstable_flags = ws.config().cli_unstable();
        let mut enable = |feat_opts: &Vec<String>| {
            for opt in feat_opts {
                match opt.as_ref() {
                    "build_dep" | "host_dep" => opts.decouple_host_deps = true,
                    "dev_dep" => opts.decouple_dev_deps = true,
                    "itarget" => opts.ignore_inactive_targets = true,
                    "all" => {
                        opts.decouple_host_deps = true;
                        opts.decouple_dev_deps = true;
                        opts.ignore_inactive_targets = true;
                    }
                    "compare" => opts.compare = true,
                    "ws" => unimplemented!(),
                    s => bail!("-Zfeatures flag `{}` is not supported", s),
                }
            }
            Ok(())
        };
        if let Some(feat_opts) = unstable_flags.features.as_ref() {
            enable(feat_opts)?;
        }
        match ws.resolve_behavior() {
            ResolveBehavior::V1 => {}
            ResolveBehavior::V2 => {
                enable(&vec!["all".to_string()]).unwrap();
            }
        }
        if let HasDevUnits::Yes = has_dev_units {
            // Dev deps cannot be decoupled when they are in use.
            opts.decouple_dev_deps = false;
        }
        if let ForceAllTargets::Yes = force_all_targets {
            opts.ignore_inactive_targets = false;
        }
        Ok(opts)
    }

    /// Creates a new FeatureOpts for the given behavior.
    pub fn new_behavior(behavior: ResolveBehavior, has_dev_units: HasDevUnits) -> FeatureOpts {
        match behavior {
            ResolveBehavior::V1 => FeatureOpts::default(),
            ResolveBehavior::V2 => FeatureOpts {
                decouple_host_deps: true,
                decouple_dev_deps: has_dev_units == HasDevUnits::No,
                ignore_inactive_targets: true,
                compare: false,
            },
        }
    }
}

/// Features flags requested for a package.
///
/// This should be cheap and fast to clone, it is used in the resolver for
/// various caches.
///
/// This is split into enum variants because the resolver needs to handle
/// features coming from different places (command-line and dependency
/// declarations), but those different places have different constraints on
/// which syntax is allowed. This helps ensure that every place dealing with
/// features is properly handling those syntax restrictions.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum RequestedFeatures {
    /// Features requested on the command-line with flags.
    CliFeatures(CliFeatures),
    /// Features specified in a dependency declaration.
    DepFeatures {
        /// The `features` dependency field.
        features: FeaturesSet,
        /// The `default-features` dependency field.
        uses_default_features: bool,
    },
}

/// Features specified on the command-line.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct CliFeatures {
    /// Features from the `--features` flag.
    pub features: Rc<BTreeSet<FeatureValue>>,
    /// The `--all-features` flag.
    pub all_features: bool,
    /// Inverse of `--no-default-features` flag.
    pub uses_default_features: bool,
}

impl CliFeatures {
    /// Creates a new CliFeatures from the given command-line flags.
    pub fn from_command_line(
        features: &[String],
        all_features: bool,
        uses_default_features: bool,
    ) -> CargoResult<CliFeatures> {
        let features = Rc::new(CliFeatures::split_features(features));
        // Some early validation to ensure correct syntax.
        for feature in features.iter() {
            match feature {
                // Maybe call validate_feature_name here once it is an error?
                FeatureValue::Feature(_) => {}
                FeatureValue::Dep { .. } => {
                    bail!(
                        "feature `{}` is not allowed to use explicit `dep:` syntax",
                        feature
                    );
                }
                FeatureValue::DepFeature { dep_feature, .. } => {
                    if dep_feature.contains('/') {
                        bail!("multiple slashes in feature `{}` is not allowed", feature);
                    }
                }
            }
        }
        Ok(CliFeatures {
            features,
            all_features,
            uses_default_features,
        })
    }

    /// Creates a new CliFeatures with the given `all_features` setting.
    pub fn new_all(all_features: bool) -> CliFeatures {
        CliFeatures {
            features: Rc::new(BTreeSet::new()),
            all_features,
            uses_default_features: true,
        }
    }

    fn split_features(features: &[String]) -> BTreeSet<FeatureValue> {
        features
            .iter()
            .flat_map(|s| s.split_whitespace())
            .flat_map(|s| s.split(','))
            .filter(|s| !s.is_empty())
            .map(InternedString::new)
            .map(FeatureValue::new)
            .collect()
    }
}

impl ResolvedFeatures {
    /// Returns the list of features that are enabled for the given package.
    pub fn activated_features(
        &self,
        pkg_id: PackageId,
        features_for: FeaturesFor,
    ) -> Vec<InternedString> {
        self.activated_features_int(pkg_id, features_for)
            .expect("activated_features for invalid package")
    }

    /// Returns if the given dependency should be included.
    ///
    /// This handles dependencies disabled via `cfg` expressions and optional
    /// dependencies which are not enabled.
    pub fn is_dep_activated(
        &self,
        pkg_id: PackageId,
        features_for: FeaturesFor,
        dep_name: InternedString,
    ) -> bool {
        let key = features_for.apply_opts(&self.opts);
        self.activated_dependencies
            .get(&(pkg_id, key))
            .map(|deps| deps.contains(&dep_name))
            .unwrap_or(false)
    }

    /// Variant of `activated_features` that returns `None` if this is
    /// not a valid pkg_id/is_build combination. Used in places which do
    /// not know which packages are activated (like `cargo clean`).
    pub fn activated_features_unverified(
        &self,
        pkg_id: PackageId,
        features_for: FeaturesFor,
    ) -> Option<Vec<InternedString>> {
        self.activated_features_int(pkg_id, features_for).ok()
    }

    fn activated_features_int(
        &self,
        pkg_id: PackageId,
        features_for: FeaturesFor,
    ) -> CargoResult<Vec<InternedString>> {
        let fk = features_for.apply_opts(&self.opts);
        if let Some(fs) = self.activated_features.get(&(pkg_id, fk)) {
            Ok(fs.iter().cloned().collect())
        } else {
            bail!("features did not find {:?} {:?}", pkg_id, fk)
        }
    }

    /// Compares the result against the original resolver behavior.
    ///
    /// Used by `cargo fix --edition` to display any differences.
    pub fn compare_legacy(&self, legacy: &ResolvedFeatures) -> DiffMap {
        self.activated_features
            .iter()
            .filter_map(|((pkg_id, for_host), new_features)| {
                let old_features = legacy
                    .activated_features
                    .get(&(*pkg_id, *for_host))
                    // The new features may have for_host entries where the old one does not.
                    .or_else(|| {
                        legacy
                            .activated_features
                            .get(&(*pkg_id, FeaturesFor::default()))
                    })
                    .map(|feats| feats.iter().cloned().collect())
                    .unwrap_or_else(|| BTreeSet::new());
                // The new resolver should never add features.
                assert_eq!(new_features.difference(&old_features).next(), None);
                let removed_features: BTreeSet<_> =
                    old_features.difference(new_features).cloned().collect();
                if removed_features.is_empty() {
                    None
                } else {
                    Some(((*pkg_id, *for_host), removed_features))
                }
            })
            .collect()
    }
}

/// Map of differences.
///
/// Key is `(pkg_id, for_host)`. Value is a set of features or dependencies removed.
pub type DiffMap = BTreeMap<PackageFeaturesKey, BTreeSet<InternedString>>;

/// The new feature resolver that [`resolve`]s your project.
///
/// For more information, please see the [module-level documentation].
///
/// [`resolve`]: Self::resolve
/// [module-level documentation]: crate::core::resolver::features
pub struct FeatureResolver<'a, 'cfg> {
    ws: &'a Workspace<'cfg>,
    target_data: &'a mut RustcTargetData<'cfg>,
    /// The platforms to build for, requested by the user.
    requested_targets: &'a [CompileKind],
    resolve: &'a Resolve,
    package_set: &'a PackageSet<'cfg>,
    /// Options that change how the feature resolver operates.
    opts: FeatureOpts,
    /// Map of features activated for each package.
    activated_features: ActivateMap,
    /// Map of optional dependencies activated for each package.
    activated_dependencies: ActivateMap,
    /// Keeps track of which packages have had its dependencies processed.
    /// Used to avoid cycles, and to speed up processing.
    processed_deps: HashSet<PackageFeaturesKey>,
    /// If this is `true`, then a non-default `feature_key` needs to be tracked while
    /// traversing the graph.
    ///
    /// This is only here to avoid calling `is_proc_macro` when all feature
    /// options are disabled (because `is_proc_macro` can trigger downloads).
    /// This has to be separate from `FeatureOpts.decouple_host_deps` because
    /// `for_host` tracking is also needed for `itarget` to work properly.
    track_for_host: bool,
    /// `dep_name?/feat_name` features that will be activated if `dep_name` is
    /// ever activated.
    ///
    /// The key is the `(package, for_host, dep_name)` of the package whose
    /// dependency will trigger the addition of new features. The value is the
    /// set of features to activate.
    deferred_weak_dependencies:
        HashMap<(PackageId, FeaturesFor, InternedString), HashSet<InternedString>>,
}

impl<'a, 'cfg> FeatureResolver<'a, 'cfg> {
    /// Runs the resolution algorithm and returns a new [`ResolvedFeatures`]
    /// with the result.
    pub fn resolve(
        ws: &Workspace<'cfg>,
        target_data: &'a mut RustcTargetData<'cfg>,
        resolve: &Resolve,
        package_set: &'a PackageSet<'cfg>,
        cli_features: &CliFeatures,
        specs: &[PackageIdSpec],
        requested_targets: &[CompileKind],
        opts: FeatureOpts,
    ) -> CargoResult<ResolvedFeatures> {
        use crate::util::profile;
        let _p = profile::start("resolve features");
        let track_for_host = opts.decouple_host_deps || opts.ignore_inactive_targets;
        let mut r = FeatureResolver {
            ws,
            target_data,
            requested_targets,
            resolve,
            package_set,
            opts,
            activated_features: HashMap::new(),
            activated_dependencies: HashMap::new(),
            processed_deps: HashSet::new(),
            track_for_host,
            deferred_weak_dependencies: HashMap::new(),
        };
        r.do_resolve(specs, cli_features)?;
        tracing::debug!("features={:#?}", r.activated_features);
        if r.opts.compare {
            r.compare();
        }
        Ok(ResolvedFeatures {
            activated_features: r.activated_features,
            activated_dependencies: r.activated_dependencies,
            opts: r.opts,
        })
    }

    /// Performs the process of resolving all features for the resolve graph.
    fn do_resolve(
        &mut self,
        specs: &[PackageIdSpec],
        cli_features: &CliFeatures,
    ) -> CargoResult<()> {
        let member_features = self.ws.members_with_features(specs, cli_features)?;
        for (member, cli_features) in &member_features {
            let fvs = self.fvs_from_requested(member.package_id(), cli_features);
            let fk = if self.track_for_host && self.is_proc_macro(member.package_id()) {
                // Also activate for normal dependencies. This is needed if the
                // proc-macro includes other targets (like binaries or tests),
                // or running in `cargo test`. Note that in a workspace, if
                // the proc-macro is selected on the command like (like with
                // `--workspace`), this forces feature unification with normal
                // dependencies. This is part of the bigger problem where
                // features depend on which packages are built.
                self.activate_pkg(member.package_id(), FeaturesFor::default(), &fvs)?;
                FeaturesFor::HostDep
            } else {
                FeaturesFor::default()
            };
            self.activate_pkg(member.package_id(), fk, &fvs)?;
        }
        Ok(())
    }

    /// Activates [`FeatureValue`]s on the given package.
    ///
    /// This is the main entrance into the recursion of feature activation
    /// for a package.
    fn activate_pkg(
        &mut self,
        pkg_id: PackageId,
        fk: FeaturesFor,
        fvs: &[FeatureValue],
    ) -> CargoResult<()> {
        tracing::trace!("activate_pkg {} {}", pkg_id.name(), fk);
        // Add an empty entry to ensure everything is covered. This is intended for
        // finding bugs where the resolver missed something it should have visited.
        // Remove this in the future if `activated_features` uses an empty default.
        self.activated_features
            .entry((pkg_id, fk.apply_opts(&self.opts)))
            .or_insert_with(BTreeSet::new);
        for fv in fvs {
            self.activate_fv(pkg_id, fk, fv)?;
        }
        if !self.processed_deps.insert((pkg_id, fk)) {
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
            // `FeatureValue::DepFeature` branch, and then immediately
            // recurse into that optional dependency. This also holds true for
            // features that enable other features.
            return Ok(());
        }
        for (dep_pkg_id, deps) in self.deps(pkg_id, fk)? {
            for (dep, dep_fk) in deps {
                if dep.is_optional() {
                    // Optional dependencies are enabled in `activate_fv` when
                    // a feature enables it.
                    continue;
                }
                // Recurse into the dependency.
                let fvs = self.fvs_from_dependency(dep_pkg_id, dep);
                self.activate_pkg(dep_pkg_id, dep_fk, &fvs)?;
            }
        }
        Ok(())
    }

    /// Activate a single FeatureValue for a package.
    fn activate_fv(
        &mut self,
        pkg_id: PackageId,
        fk: FeaturesFor,
        fv: &FeatureValue,
    ) -> CargoResult<()> {
        tracing::trace!("activate_fv {} {} {}", pkg_id.name(), fk, fv);
        match fv {
            FeatureValue::Feature(f) => {
                self.activate_rec(pkg_id, fk, *f)?;
            }
            FeatureValue::Dep { dep_name } => {
                self.activate_dependency(pkg_id, fk, *dep_name)?;
            }
            FeatureValue::DepFeature {
                dep_name,
                dep_feature,
                weak,
            } => {
                self.activate_dep_feature(pkg_id, fk, *dep_name, *dep_feature, *weak)?;
            }
        }
        Ok(())
    }

    /// Activate the given feature for the given package, and then recursively
    /// activate any other features that feature enables.
    fn activate_rec(
        &mut self,
        pkg_id: PackageId,
        fk: FeaturesFor,
        feature_to_enable: InternedString,
    ) -> CargoResult<()> {
        tracing::trace!(
            "activate_rec {} {} feat={}",
            pkg_id.name(),
            fk,
            feature_to_enable
        );
        let enabled = self
            .activated_features
            .entry((pkg_id, fk.apply_opts(&self.opts)))
            .or_insert_with(BTreeSet::new);
        if !enabled.insert(feature_to_enable) {
            // Already enabled.
            return Ok(());
        }
        let summary = self.resolve.summary(pkg_id);
        let feature_map = summary.features();
        let Some(fvs) = feature_map.get(&feature_to_enable) else {
            // TODO: this should only happen for optional dependencies.
            // Other cases should be validated by Summary's `build_feature_map`.
            // Figure out some way to validate this assumption.
            tracing::debug!(
                "pkg {:?} does not define feature {}",
                pkg_id,
                feature_to_enable
            );
            return Ok(());
        };
        for fv in fvs {
            self.activate_fv(pkg_id, fk, fv)?;
        }
        Ok(())
    }

    /// Activate a dependency (`dep:dep_name` syntax).
    fn activate_dependency(
        &mut self,
        pkg_id: PackageId,
        fk: FeaturesFor,
        dep_name: InternedString,
    ) -> CargoResult<()> {
        // Mark this dependency as activated.
        let save_decoupled = fk.apply_opts(&self.opts);
        self.activated_dependencies
            .entry((pkg_id, save_decoupled))
            .or_default()
            .insert(dep_name);
        // Check for any deferred features.
        let to_enable = self
            .deferred_weak_dependencies
            .remove(&(pkg_id, fk, dep_name));
        // Activate the optional dep.
        for (dep_pkg_id, deps) in self.deps(pkg_id, fk)? {
            for (dep, dep_fk) in deps {
                if dep.name_in_toml() != dep_name {
                    continue;
                }
                if let Some(to_enable) = &to_enable {
                    for dep_feature in to_enable {
                        tracing::trace!(
                            "activate deferred {} {} -> {}/{}",
                            pkg_id.name(),
                            fk,
                            dep_name,
                            dep_feature
                        );
                        let fv = FeatureValue::new(*dep_feature);
                        self.activate_fv(dep_pkg_id, dep_fk, &fv)?;
                    }
                }
                let fvs = self.fvs_from_dependency(dep_pkg_id, dep);
                self.activate_pkg(dep_pkg_id, dep_fk, &fvs)?;
            }
        }
        Ok(())
    }

    /// Activate a feature within a dependency (`dep_name/feat_name` syntax).
    fn activate_dep_feature(
        &mut self,
        pkg_id: PackageId,
        fk: FeaturesFor,
        dep_name: InternedString,
        dep_feature: InternedString,
        weak: bool,
    ) -> CargoResult<()> {
        for (dep_pkg_id, deps) in self.deps(pkg_id, fk)? {
            for (dep, dep_fk) in deps {
                if dep.name_in_toml() != dep_name {
                    continue;
                }
                if dep.is_optional() {
                    let save_for_host = fk.apply_opts(&self.opts);
                    if weak
                        && !self
                            .activated_dependencies
                            .get(&(pkg_id, save_for_host))
                            .map(|deps| deps.contains(&dep_name))
                            .unwrap_or(false)
                    {
                        // This is weak, but not yet activated. Defer in case
                        // something comes along later and enables it.
                        tracing::trace!(
                            "deferring feature {} {} -> {}/{}",
                            pkg_id.name(),
                            fk,
                            dep_name,
                            dep_feature
                        );
                        self.deferred_weak_dependencies
                            .entry((pkg_id, fk, dep_name))
                            .or_default()
                            .insert(dep_feature);
                        continue;
                    }

                    // Activate the dependency on self.
                    let fv = FeatureValue::Dep { dep_name };
                    self.activate_fv(pkg_id, fk, &fv)?;
                    if !weak {
                        // The old behavior before weak dependencies were
                        // added is to also enables a feature of the same
                        // name.
                        //
                        // Don't enable if the implicit optional dependency
                        // feature wasn't created due to `dep:` hiding.
                        // See rust-lang/cargo#10788 and rust-lang/cargo#12130
                        let summary = self.resolve.summary(pkg_id);
                        let feature_map = summary.features();
                        if feature_map.contains_key(&dep_name) {
                            self.activate_rec(pkg_id, fk, dep_name)?;
                        }
                    }
                }
                // Activate the feature on the dependency.
                let fv = FeatureValue::new(dep_feature);
                self.activate_fv(dep_pkg_id, dep_fk, &fv)?;
            }
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
            .map(|f| FeatureValue::new(*f))
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
        cli_features: &CliFeatures,
    ) -> Vec<FeatureValue> {
        let summary = self.resolve.summary(pkg_id);
        let feature_map = summary.features();

        let mut result: Vec<FeatureValue> = cli_features.features.iter().cloned().collect();
        let default = InternedString::new("default");
        if cli_features.uses_default_features && feature_map.contains_key(&default) {
            result.push(FeatureValue::Feature(default));
        }

        if cli_features.all_features {
            result.extend(feature_map.keys().map(|k| FeatureValue::Feature(*k)))
        }

        result
    }

    /// Returns the dependencies for a package, filtering out inactive targets.
    fn deps(
        &mut self,
        pkg_id: PackageId,
        fk: FeaturesFor,
    ) -> CargoResult<Vec<(PackageId, Vec<(&'a Dependency, FeaturesFor)>)>> {
        // Helper for determining if a platform is activated.
        fn platform_activated(
            dep: &Dependency,
            fk: FeaturesFor,
            target_data: &RustcTargetData<'_>,
            requested_targets: &[CompileKind],
        ) -> bool {
            // We always count platforms as activated if the target stems from an artifact
            // dependency's target specification. This triggers in conjunction with
            // `[target.'cfg(…)'.dependencies]` manifest sections.
            match (dep.is_build(), fk) {
                (true, _) | (_, FeaturesFor::HostDep) => {
                    // We always care about build-dependencies, and they are always
                    // Host. If we are computing dependencies "for a build script",
                    // even normal dependencies are host-only.
                    target_data.dep_platform_activated(dep, CompileKind::Host)
                }
                (_, FeaturesFor::NormalOrDev) => requested_targets
                    .iter()
                    .any(|kind| target_data.dep_platform_activated(dep, *kind)),
                (_, FeaturesFor::ArtifactDep(target)) => {
                    target_data.dep_platform_activated(dep, CompileKind::Target(target))
                }
            }
        }

        self.resolve
            .deps(pkg_id)
            .map(|(dep_id, deps)| {
                let deps = deps
                    .iter()
                    .filter(|dep| {
                        if dep.platform().is_some()
                            && self.opts.ignore_inactive_targets
                            && !platform_activated(
                                dep,
                                fk,
                                self.target_data,
                                self.requested_targets,
                            )
                        {
                            return false;
                        }
                        if self.opts.decouple_dev_deps && dep.kind() == DepKind::Development {
                            return false;
                        }
                        true
                    })
                    .collect_vec() // collect because the next closure mutably borrows `self.target_data`
                    .into_iter()
                    .map(|dep| {
                        // Each `dep`endency can be built for multiple targets. For one, it
                        // may be a library target which is built as initially configured
                        // by `fk`. If it appears as build dependency, it must be built
                        // for the host.
                        //
                        // It may also be an artifact dependency,
                        // which could be built either
                        //
                        //  - for a specified (aka 'forced') target, specified by
                        //    `dep = { …, target = <triple>` }`
                        //  - as an artifact for use in build dependencies that should
                        //    build for whichever `--target`s are specified
                        //  - like a library would be built
                        //
                        // Generally, the logic for choosing a target for dependencies is
                        // unaltered and used to determine how to build non-artifacts,
                        // artifacts without target specification and no library,
                        // or an artifacts library.
                        //
                        // All this may result in a dependency being built multiple times
                        // for various targets which are either specified in the manifest
                        // or on the cargo command-line.
                        let lib_fk = if fk == FeaturesFor::default() {
                            (self.track_for_host && (dep.is_build() || self.is_proc_macro(dep_id)))
                                .then(|| FeaturesFor::HostDep)
                                .unwrap_or_default()
                        } else {
                            fk
                        };

                        // `artifact_target_keys` are produced to fulfil the needs of artifacts that have a target specification.
                        let artifact_target_keys = dep
                            .artifact()
                            .map(|artifact| {
                                let host_triple = self.target_data.rustc.host;
                                // not all targets may be queried before resolution since artifact dependencies
                                // and per-pkg-targets are not immediately known.
                                let mut activate_target = |target| {
                                    let name = dep.name_in_toml();
                                    self.target_data
                                        .merge_compile_kind(CompileKind::Target(target))
                                        .with_context(|| format!("failed to determine target information for target `{target}`.\n  \
                                        Artifact dependency `{name}` in package `{pkg_id}` requires building for `{target}`", target = target.rustc_target()))
                                };
                                CargoResult::Ok((
                                    artifact.is_lib(),
                                    artifact
                                        .target()
                                        .map(|target| {
                                            CargoResult::Ok(match target {
                                                ArtifactTarget::Force(target) => {
                                                    activate_target(target)?;
                                                    vec![FeaturesFor::ArtifactDep(target)]
                                                }
                                                // FIXME: this needs to interact with the `default-target` and `forced-target` values
                                                // of the dependency
                                                ArtifactTarget::BuildDependencyAssumeTarget => self
                                                    .requested_targets
                                                    .iter()
                                                    .map(|kind| match kind {
                                                        CompileKind::Host => {
                                                            CompileTarget::new(&host_triple)
                                                                .unwrap()
                                                        }
                                                        CompileKind::Target(target) => *target,
                                                    })
                                                    .map(|target| {
                                                        activate_target(target)?;
                                                        Ok(FeaturesFor::ArtifactDep(target))
                                                    })
                                                    .collect::<CargoResult<_>>()?,
                                            })
                                        })
                                        .transpose()?,
                                ))
                            })
                            .transpose()?;

                        let dep_fks = match artifact_target_keys {
                            // The artifact is also a library and does specify custom
                            // targets.
                            // The library's feature key needs to be used alongside
                            // the keys artifact targets.
                            Some((is_lib, Some(mut dep_fks))) if is_lib => {
                                dep_fks.push(lib_fk);
                                dep_fks
                            }
                            // The artifact is not a library, but does specify
                            // custom targets.
                            // Use only these targets feature keys.
                            Some((_, Some(dep_fks))) => dep_fks,
                            // There is no artifact in the current dependency
                            // or there is no target specified on the artifact.
                            // Use the standard feature key without any alteration.
                            Some((_, None)) | None => vec![lib_fk],
                        };
                        Ok(dep_fks.into_iter().map(move |dep_fk| (dep, dep_fk)))
                    })
                    .flatten_ok()
                    .collect::<CargoResult<Vec<_>>>()?;
                Ok((dep_id, deps))
            })
            .filter(|res| res.as_ref().map_or(true, |(_id, deps)| !deps.is_empty()))
            .collect()
    }

    /// Compare the activated features to the resolver. Used for testing.
    fn compare(&self) {
        let mut found = false;
        for ((pkg_id, dep_kind), features) in &self.activated_features {
            let r_features = self.resolve.features(*pkg_id);
            if !r_features.iter().eq(features.iter()) {
                crate::drop_eprintln!(
                    self.ws.config(),
                    "{}/{:?} features mismatch\nresolve: {:?}\nnew: {:?}\n",
                    pkg_id,
                    dep_kind,
                    r_features,
                    features
                );
                found = true;
            }
        }
        if found {
            panic!("feature mismatch");
        }
    }

    fn is_proc_macro(&self, package_id: PackageId) -> bool {
        self.package_set
            .get_one(package_id)
            .expect("packages downloaded")
            .proc_macro()
    }
}
