use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use log::debug;

use crate::core::interning::InternedString;
use crate::core::{Dependency, FeatureValue, PackageId, PackageIdSpec, Registry, Summary};
use crate::util::errors::CargoResult;

use crate::core::resolver::types::{Candidate, ConflictReason};
use crate::core::resolver::{ActivateResult, Method};

pub struct RegistryQueryer<'a> {
    pub registry: &'a mut (dyn Registry + 'a),
    replacements: &'a [(PackageIdSpec, Dependency)],
    try_to_use: &'a HashSet<PackageId>,
    // If set the list of dependency candidates will be sorted by minimal
    // versions first. That allows `cargo update -Z minimal-versions` which will
    // specify minimum dependency versions to be used.
    minimal_versions: bool,
    cache: HashMap<Dependency, Rc<Vec<Candidate>>>,
    used_replacements: HashMap<PackageId, PackageId>,
}

impl<'a> RegistryQueryer<'a> {
    pub fn new(
        registry: &'a mut dyn Registry,
        replacements: &'a [(PackageIdSpec, Dependency)],
        try_to_use: &'a HashSet<PackageId>,
        minimal_versions: bool,
    ) -> Self {
        RegistryQueryer {
            registry,
            replacements,
            try_to_use,
            minimal_versions,
            cache: HashMap::new(),
            used_replacements: HashMap::new(),
        }
    }

    pub fn used_replacement_for(&self, p: PackageId) -> Option<(PackageId, PackageId)> {
        self.used_replacements.get(&p).map(|&r| (p, r))
    }

    /// Queries the `registry` to return a list of candidates for `dep`.
    ///
    /// This method is the location where overrides are taken into account. If
    /// any candidates are returned which match an override then the override is
    /// applied by performing a second query for what the override should
    /// return.
    pub fn query(&mut self, dep: &Dependency) -> CargoResult<Rc<Vec<Candidate>>> {
        if let Some(out) = self.cache.get(dep).cloned() {
            return Ok(out);
        }

        let mut ret = Vec::new();
        self.registry.query(
            dep,
            &mut |s| {
                ret.push(Candidate {
                    summary: s,
                    replace: None,
                });
            },
            false,
        )?;
        for candidate in ret.iter_mut() {
            let summary = &candidate.summary;

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
                failure::format_err!(
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
                failure::bail!(
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
                failure::bail!(
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
            if let Some(r) = &replace {
                self.used_replacements
                    .insert(summary.package_id(), r.package_id());
            }

            candidate.replace = replace;
        }

        // When we attempt versions for a package we'll want to do so in a
        // sorted fashion to pick the "best candidates" first. Currently we try
        // prioritized summaries (those in `try_to_use`) and failing that we
        // list everything from the maximum version to the lowest version.
        ret.sort_unstable_by(|a, b| {
            let a_in_previous = self.try_to_use.contains(&a.summary.package_id());
            let b_in_previous = self.try_to_use.contains(&b.summary.package_id());
            let previous_cmp = a_in_previous.cmp(&b_in_previous).reverse();
            match previous_cmp {
                Ordering::Equal => {
                    let cmp = a.summary.version().cmp(b.summary.version());
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

        self.cache.insert(dep.clone(), out.clone());

        Ok(out)
    }
}

/// Returns all dependencies and the features we want from them.
pub fn resolve_features<'b>(
    parent: Option<PackageId>,
    s: &'b Summary,
    method: &'b Method<'_>,
) -> ActivateResult<(
    HashSet<InternedString>,
    Vec<(Dependency, Vec<InternedString>)>,
)> {
    let dev_deps = match *method {
        Method::Everything => true,
        Method::Required { dev_deps, .. } => dev_deps,
    };

    // First, filter by dev-dependencies.
    let deps = s.dependencies();
    let deps = deps.iter().filter(|d| d.is_transitive() || dev_deps);

    let reqs = build_requirements(s, method)?;
    let mut ret = Vec::new();
    let mut used_features = HashSet::new();
    let default_dep = (false, Vec::new());

    // Next, collect all actually enabled dependencies and their features.
    for dep in deps {
        // Skip optional dependencies, but not those enabled through a
        // feature
        if dep.is_optional() && !reqs.deps.contains_key(&dep.name_in_toml()) {
            continue;
        }
        // So we want this dependency. Move the features we want from
        // `feature_deps` to `ret` and register ourselves as using this
        // name.
        let base = reqs.deps.get(&dep.name_in_toml()).unwrap_or(&default_dep);
        used_features.insert(dep.name_in_toml());
        let always_required = !dep.is_optional()
            && !s
                .dependencies()
                .iter()
                .any(|d| d.is_optional() && d.name_in_toml() == dep.name_in_toml());
        if always_required && base.0 {
            return Err(match parent {
                None => failure::format_err!(
                    "Package `{}` does not have feature `{}`. It has a required dependency \
                     with that name, but only optional dependencies can be used as features.",
                    s.package_id(),
                    dep.name_in_toml()
                )
                .into(),
                Some(p) => (
                    p,
                    ConflictReason::RequiredDependencyAsFeatures(dep.name_in_toml()),
                )
                    .into(),
            });
        }
        let mut base = base.1.clone();
        base.extend(dep.features().iter());
        for feature in base.iter() {
            if feature.contains('/') {
                return Err(failure::format_err!(
                    "feature names may not contain slashes: `{}`",
                    feature
                )
                .into());
            }
        }
        ret.push((dep.clone(), base));
    }

    // Any entries in `reqs.dep` which weren't used are bugs in that the
    // package does not actually have those dependencies. We classified
    // them as dependencies in the first place because there is no such
    // feature, either.
    let remaining = reqs
        .deps
        .keys()
        .cloned()
        .filter(|s| !used_features.contains(s))
        .collect::<Vec<_>>();
    if !remaining.is_empty() {
        let features = remaining.join(", ");
        return Err(match parent {
            None => failure::format_err!(
                "Package `{}` does not have these features: `{}`",
                s.package_id(),
                features
            )
            .into(),
            Some(p) => (p, ConflictReason::MissingFeatures(features)).into(),
        });
    }

    Ok((reqs.into_used(), ret))
}

/// Takes requested features for a single package from the input `Method` and
/// recurses to find all requested features, dependencies and requested
/// dependency features in a `Requirements` object, returning it to the resolver.
fn build_requirements<'a, 'b: 'a>(
    s: &'a Summary,
    method: &'b Method<'_>,
) -> CargoResult<Requirements<'a>> {
    let mut reqs = Requirements::new(s);

    match *method {
        Method::Everything
        | Method::Required {
            all_features: true, ..
        } => {
            for key in s.features().keys() {
                reqs.require_feature(*key)?;
            }
            for dep in s.dependencies().iter().filter(|d| d.is_optional()) {
                reqs.require_dependency(dep.name_in_toml());
            }
        }
        Method::Required {
            all_features: false,
            features: requested,
            ..
        } => {
            for &f in requested.iter() {
                reqs.require_value(&FeatureValue::new(f, s))?;
            }
        }
    }
    match *method {
        Method::Everything
        | Method::Required {
            uses_default_features: true,
            ..
        } => {
            if s.features().contains_key("default") {
                reqs.require_feature(InternedString::new("default"))?;
            }
        }
        Method::Required {
            uses_default_features: false,
            ..
        } => {}
    }
    Ok(reqs)
}

struct Requirements<'a> {
    summary: &'a Summary,
    // The deps map is a mapping of package name to list of features enabled.
    // Each package should be enabled, and each package should have the
    // specified set of features enabled. The boolean indicates whether this
    // package was specifically requested (rather than just requesting features
    // *within* this package).
    deps: HashMap<InternedString, (bool, Vec<InternedString>)>,
    // The used features set is the set of features which this local package had
    // enabled, which is later used when compiling to instruct the code what
    // features were enabled.
    used: HashSet<InternedString>,
    visited: HashSet<InternedString>,
}

impl Requirements<'_> {
    fn new(summary: &Summary) -> Requirements<'_> {
        Requirements {
            summary,
            deps: HashMap::new(),
            used: HashSet::new(),
            visited: HashSet::new(),
        }
    }

    fn into_used(self) -> HashSet<InternedString> {
        self.used
    }

    fn require_crate_feature(&mut self, package: InternedString, feat: InternedString) {
        self.used.insert(package);
        self.deps
            .entry(package)
            .or_insert((false, Vec::new()))
            .1
            .push(feat);
    }

    fn seen(&mut self, feat: InternedString) -> bool {
        if self.visited.insert(feat) {
            self.used.insert(feat);
            false
        } else {
            true
        }
    }

    fn require_dependency(&mut self, pkg: InternedString) {
        if self.seen(pkg) {
            return;
        }
        self.deps.entry(pkg).or_insert((false, Vec::new())).0 = true;
    }

    fn require_feature(&mut self, feat: InternedString) -> CargoResult<()> {
        if feat.is_empty() || self.seen(feat) {
            return Ok(());
        }
        for fv in self
            .summary
            .features()
            .get(feat.as_str())
            .expect("must be a valid feature")
        {
            match *fv {
                FeatureValue::Feature(ref dep_feat) if **dep_feat == *feat => failure::bail!(
                    "cyclic feature dependency: feature `{}` depends on itself",
                    feat
                ),
                _ => {}
            }
            self.require_value(fv)?;
        }
        Ok(())
    }

    fn require_value(&mut self, fv: &FeatureValue) -> CargoResult<()> {
        match fv {
            FeatureValue::Feature(feat) => self.require_feature(*feat)?,
            FeatureValue::Crate(dep) => self.require_dependency(*dep),
            FeatureValue::CrateFeature(dep, dep_feat) => {
                self.require_crate_feature(*dep, *dep_feat)
            }
        };
        Ok(())
    }
}
