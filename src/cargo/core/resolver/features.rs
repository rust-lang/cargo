//! Feature resolver.
//!
//! This is a new feature resolver that runs independently of the main
//! dependency resolver. It is intended to make it easier to experiment with
//! new behaviors. When `-Zfeatures` is not used, it will fall back to using
//! the original `Resolve` feature computation. With `-Zfeatures` enabled,
//! this will walk the dependency graph and compute the features using a
//! different algorithm. One of its key characteristics is that it can avoid
//! unifying features for shared dependencies in some situations.
//!
//! The preferred way to engage this new resolver is via
//! `resolve_ws_with_opts`.
//!
//! There are many assumptions made about the resolver itself. It assumes
//! validation has already been done on the feature maps, and doesn't do any
//! validation itself. It assumes dev-dependencies within a dependency have
//! been removed.

use crate::core::compiler::{CompileKind, RustcTargetData};
use crate::core::dependency::{DepKind, Dependency};
use crate::core::resolver::types::FeaturesSet;
use crate::core::resolver::Resolve;
use crate::core::{FeatureValue, InternedString, PackageId, PackageIdSpec, Workspace};
use crate::util::{CargoResult, Config};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::rc::Rc;

/// Map of activated features for a PackageId/DepKind/CompileKind.
///
/// `DepKind` is needed, as the same package can be built multiple times with
/// different features. For example, with `decouple_build_deps`, a dependency
/// can be built once as a build dependency (for example with a 'std'
/// feature), and once as a normal dependency (without that 'std' feature).
///
/// `CompileKind` is used currently not needed.
type ActivateMap = HashMap<(PackageId, DepKind, CompileKind), BTreeSet<InternedString>>;

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

impl FeatureOpts {
    fn new(config: &Config, has_dev_units: bool) -> CargoResult<FeatureOpts> {
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
        if has_dev_units {
            // Decoupling of dev deps is not allowed if any test/bench/example
            // is being built. It may be possible to relax this in the future,
            // but it will require significant changes to how unit
            // dependencies are computed, and can result in longer build times
            // with `cargo test` because the lib may need to be built 3 times
            // instead of twice.
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
        dep_kind: DepKind,
        compile_kind: CompileKind,
    ) -> Vec<InternedString> {
        if let Some(legacy) = &self.legacy {
            legacy.get(&pkg_id).map_or_else(Vec::new, |v| v.clone())
        } else {
            let dep_kind = if (!self.opts.decouple_build_deps && dep_kind == DepKind::Build)
                || (!self.opts.decouple_dev_deps && dep_kind == DepKind::Development)
            {
                // Decoupling disabled, everything is unified under "Normal".
                DepKind::Normal
            } else {
                dep_kind
            };
            // TODO: Remove panic, return empty set.
            let fs = self
                .activated_features
                .get(&(pkg_id, dep_kind, compile_kind))
                .unwrap_or_else(|| panic!("features did not find {:?} {:?}", pkg_id, dep_kind));
            fs.iter().cloned().collect()
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
    processed_deps: HashSet<(PackageId, DepKind, CompileKind)>,
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
        has_dev_units: bool,
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
        for (member, requested_features) in member_features {
            self.activate_member(member.package_id(), &requested_features)?;
        }
        Ok(())
    }

    /// Enable the given features on the given workspace member.
    fn activate_member(
        &mut self,
        pkg_id: PackageId,
        requested_features: &RequestedFeatures,
    ) -> CargoResult<()> {
        let fvs = self.fvs_from_requested(pkg_id, CompileKind::Host, requested_features);
        self.activate_member_fvs(pkg_id, CompileKind::Host, &fvs)?;
        if let CompileKind::Target(_) = self.requested_target {
            let fvs = self.fvs_from_requested(pkg_id, self.requested_target, requested_features);
            self.activate_member_fvs(pkg_id, self.requested_target, &fvs)?;
        }
        Ok(())
    }

    fn activate_member_fvs(
        &mut self,
        pkg_id: PackageId,
        compile_kind: CompileKind,
        fvs: &[FeatureValue],
    ) -> CargoResult<()> {
        self.activate_with_platform(pkg_id, DepKind::Normal, compile_kind, &fvs)?;
        if self.opts.decouple_dev_deps {
            // Activate the member as a dev dep, assuming it has at least one
            // test, bench, or example. This ensures the member's normal deps get
            // unified with its dev deps.
            self.activate_with_platform(pkg_id, DepKind::Development, compile_kind, &fvs)?;
        }
        Ok(())
    }

    fn activate_with_platform(
        &mut self,
        pkg_id: PackageId,
        dep_kind: DepKind,
        compile_kind: CompileKind,
        fvs: &[FeatureValue],
    ) -> CargoResult<()> {
        // Add an empty entry to ensure everything is covered. This is intended for
        // finding bugs where the resolver missed something it should have visited.
        // Remove this in the future if `activated_features` uses an empty default.
        self.activated_features
            .entry((pkg_id, dep_kind, compile_kind))
            .or_insert_with(BTreeSet::new);
        for fv in fvs {
            self.activate_fv(pkg_id, dep_kind, compile_kind, fv)?;
        }
        if !self.processed_deps.insert((pkg_id, dep_kind, compile_kind)) {
            // Already processed dependencies.
            return Ok(());
        }
        // Activate any of its dependencies.
        for (dep_pkg_id, deps) in self.deps(pkg_id, compile_kind) {
            for dep in deps {
                if dep.is_optional() {
                    continue;
                }
                // Recurse into the dependency.
                let fvs = self.fvs_from_dependency(dep_pkg_id, dep);
                self.activate_with_platform(
                    dep_pkg_id,
                    self.sticky_dep_kind(dep_kind, dep.kind()),
                    compile_kind,
                    &fvs,
                )?;
            }
        }
        return Ok(());
    }

    fn activate_fv(
        &mut self,
        pkg_id: PackageId,
        dep_kind: DepKind,
        compile_kind: CompileKind,
        fv: &FeatureValue,
    ) -> CargoResult<()> {
        match fv {
            FeatureValue::Feature(f) => {
                self.activate_rec(pkg_id, dep_kind, compile_kind, *f)?;
            }
            FeatureValue::Crate(dep_name) => {
                // Activate the feature name on self.
                self.activate_rec(pkg_id, dep_kind, compile_kind, *dep_name)?;
                // Activate the optional dep.
                for (dep_pkg_id, deps) in self.deps(pkg_id, compile_kind) {
                    for dep in deps {
                        if dep.name_in_toml() == *dep_name {
                            let fvs = self.fvs_from_dependency(dep_pkg_id, dep);
                            self.activate_with_platform(
                                dep_pkg_id,
                                self.sticky_dep_kind(dep_kind, dep.kind()),
                                compile_kind,
                                &fvs,
                            )?;
                        }
                    }
                }
            }
            FeatureValue::CrateFeature(dep_name, dep_feature) => {
                // Activate a feature within a dependency.
                for (dep_pkg_id, deps) in self.deps(pkg_id, compile_kind) {
                    for dep in deps {
                        if dep.name_in_toml() == *dep_name {
                            if dep.is_optional() {
                                // Activate the crate on self.
                                let fv = FeatureValue::Crate(*dep_name);
                                self.activate_fv(pkg_id, dep_kind, compile_kind, &fv)?;
                            }
                            // Activate the feature on the dependency.
                            let summary = self.resolve.summary(dep_pkg_id);
                            let fv = FeatureValue::new(*dep_feature, summary);
                            self.activate_fv(
                                dep_pkg_id,
                                self.sticky_dep_kind(dep_kind, dep.kind()),
                                compile_kind,
                                &fv,
                            )?;
                        }
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
        dep_kind: DepKind,
        compile_kind: CompileKind,
        feature_to_enable: InternedString,
    ) -> CargoResult<()> {
        let enabled = self
            .activated_features
            .entry((pkg_id, dep_kind, compile_kind))
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
            self.activate_fv(pkg_id, dep_kind, compile_kind, fv)?;
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
        compile_kind: CompileKind,
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
            for (_dep_pkg_id, deps) in self.deps(pkg_id, compile_kind) {
                for dep in deps {
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
        compile_kind: CompileKind,
    ) -> Vec<(PackageId, Vec<&'a Dependency>)> {
        self.resolve
            .deps(pkg_id)
            .map(|(dep_id, deps)| {
                let deps = deps
                    .iter()
                    .filter(|dep| {
                        !dep.platform().is_some()
                            || !self.opts.ignore_inactive_targets
                            || self.target_data.dep_platform_activated(dep, compile_kind)
                    })
                    .collect::<Vec<_>>();
                (dep_id, deps)
            })
            .collect()
    }

    /// Convert a DepKind from a package to one of its dependencies.
    ///
    /// The rules here determine how decoupling works.
    fn sticky_dep_kind(&self, from: DepKind, to: DepKind) -> DepKind {
        if self.opts.decouple_build_deps {
            if from == DepKind::Build || to == DepKind::Build {
                return DepKind::Build;
            }
        }
        if self.opts.decouple_dev_deps {
            if to == DepKind::Development {
                return DepKind::Development;
            }
            if from == DepKind::Development && to != DepKind::Build {
                return DepKind::Development;
            }
        }
        return DepKind::Normal;
    }

    /// Compare the activated features to the resolver. Used for testing.
    fn compare(&self) {
        let mut found = false;
        for ((pkg_id, dep_kind, compile_kind), features) in &self.activated_features {
            let r_features = self.resolve.features(*pkg_id);
            if !r_features.iter().eq(features.iter()) {
                eprintln!(
                    "{}/{:?}/{:?} features mismatch\nresolve: {:?}\nnew: {:?}\n",
                    pkg_id, dep_kind, compile_kind, r_features, features
                );
                found = true;
            }
        }
        if found {
            panic!("feature mismatch");
        }
    }
}
