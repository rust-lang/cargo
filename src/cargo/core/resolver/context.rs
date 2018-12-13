use std::collections::{BTreeMap, HashMap, HashSet};
use std::rc::Rc;

#[allow(unused_imports)] // "ensure" seems to require "bail" be in scope (macro hygiene issue?)
use failure::{bail, ensure};
use log::debug;

use crate::core::interning::InternedString;
use crate::core::{Dependency, FeatureValue, PackageId, SourceId, Summary};
use crate::util::CargoResult;
use crate::util::Graph;

use super::errors::ActivateResult;
use super::types::{ConflictReason, DepInfo, GraphNode, Method, RcList, RegistryQueryer};

pub use super::encode::{EncodableDependency, EncodablePackageId, EncodableResolve};
pub use super::encode::{Metadata, WorkspaceResolve};
pub use super::resolve::Resolve;

// A `Context` is basically a bunch of local resolution information which is
// kept around for all `BacktrackFrame` instances. As a result, this runs the
// risk of being cloned *a lot* so we want to make this as cheap to clone as
// possible.
#[derive(Clone)]
pub struct Context {
    pub activations: Activations,
    pub resolve_features: im_rc::HashMap<PackageId, Rc<HashSet<InternedString>>>,
    pub links: im_rc::HashMap<InternedString, PackageId>,

    // These are two cheaply-cloneable lists (O(1) clone) which are effectively
    // hash maps but are built up as "construction lists". We'll iterate these
    // at the very end and actually construct the map that we're making.
    pub resolve_graph: RcList<GraphNode>,
    pub resolve_replacements: RcList<(PackageId, PackageId)>,

    // These warnings are printed after resolution.
    pub warnings: RcList<String>,
}

pub type Activations = im_rc::HashMap<(InternedString, SourceId), Rc<Vec<Summary>>>;

impl Context {
    pub fn new() -> Context {
        Context {
            resolve_graph: RcList::new(),
            resolve_features: im_rc::HashMap::new(),
            links: im_rc::HashMap::new(),
            resolve_replacements: RcList::new(),
            activations: im_rc::HashMap::new(),
            warnings: RcList::new(),
        }
    }

    /// Activate this summary by inserting it into our list of known activations.
    ///
    /// Returns true if this summary with the given method is already activated.
    pub fn flag_activated(&mut self, summary: &Summary, method: &Method<'_>) -> CargoResult<bool> {
        let id = summary.package_id();
        let prev = self
            .activations
            .entry((id.name(), id.source_id()))
            .or_insert_with(|| Rc::new(Vec::new()));
        if !prev.iter().any(|c| c == summary) {
            self.resolve_graph.push(GraphNode::Add(id));
            if let Some(link) = summary.links() {
                ensure!(
                    self.links.insert(link, id).is_none(),
                    "Attempting to resolve a with more then one crate with the links={}. \n\
                     This will not build as is. Consider rebuilding the .lock file.",
                    &*link
                );
            }
            Rc::make_mut(prev).push(summary.clone());
            return Ok(false);
        }
        debug!("checking if {} is already activated", summary.package_id());
        let (features, use_default) = match *method {
            Method::Everything
            | Method::Required {
                all_features: true, ..
            } => return Ok(false),
            Method::Required {
                features,
                uses_default_features,
                ..
            } => (features, uses_default_features),
        };

        let has_default_feature = summary.features().contains_key("default");
        Ok(match self.resolve_features.get(&id) {
            Some(prev) => {
                features.iter().all(|f| prev.contains(f))
                    && (!use_default || prev.contains("default") || !has_default_feature)
            }
            None => features.is_empty() && (!use_default || !has_default_feature),
        })
    }

    pub fn build_deps(
        &mut self,
        registry: &mut RegistryQueryer<'_>,
        parent: Option<&Summary>,
        candidate: &Summary,
        method: &Method<'_>,
    ) -> ActivateResult<Vec<DepInfo>> {
        // First, figure out our set of dependencies based on the requested set
        // of features. This also calculates what features we're going to enable
        // for our own dependencies.
        let deps = self.resolve_features(parent, candidate, method)?;

        // Next, transform all dependencies into a list of possible candidates
        // which can satisfy that dependency.
        let mut deps = deps
            .into_iter()
            .map(|(dep, features)| {
                let candidates = registry.query(&dep)?;
                Ok((dep, candidates, Rc::new(features)))
            })
            .collect::<CargoResult<Vec<DepInfo>>>()?;

        // Attempt to resolve dependencies with fewer candidates before trying
        // dependencies with more candidates.  This way if the dependency with
        // only one candidate can't be resolved we don't have to do a bunch of
        // work before we figure that out.
        deps.sort_by_key(|&(_, ref a, _)| a.len());

        Ok(deps)
    }

    pub fn prev_active(&self, dep: &Dependency) -> &[Summary] {
        self.activations
            .get(&(dep.package_name(), dep.source_id()))
            .map(|v| &v[..])
            .unwrap_or(&[])
    }

    pub fn is_active(&self, id: PackageId) -> bool {
        self.activations
            .get(&(id.name(), id.source_id()))
            .map(|v| v.iter().any(|s| s.package_id() == id))
            .unwrap_or(false)
    }

    /// checks whether all of `parent` and the keys of `conflicting activations`
    /// are still active
    pub fn is_conflicting(
        &self,
        parent: Option<PackageId>,
        conflicting_activations: &BTreeMap<PackageId, ConflictReason>,
    ) -> bool {
        conflicting_activations
            .keys()
            .chain(parent.as_ref())
            .all(|&id| self.is_active(id))
    }

    /// Return all dependencies and the features we want from them.
    fn resolve_features<'b>(
        &mut self,
        parent: Option<&Summary>,
        s: &'b Summary,
        method: &'b Method<'_>,
    ) -> ActivateResult<Vec<(Dependency, Vec<InternedString>)>> {
        let dev_deps = match *method {
            Method::Everything => true,
            Method::Required { dev_deps, .. } => dev_deps,
        };

        // First, filter by dev-dependencies
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
                self.warnings.push(format!(
                    "Package `{}` does not have feature `{}`. It has a required dependency \
                     with that name, but only optional dependencies can be used as features. \
                     This is currently a warning to ease the transition, but it will become an \
                     error in the future.",
                    s.package_id(),
                    dep.name_in_toml()
                ));
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
                Some(p) => (p.package_id(), ConflictReason::MissingFeatures(features)).into(),
            });
        }

        // Record what list of features is active for this package.
        if !reqs.used.is_empty() {
            let pkgid = s.package_id();

            let set = Rc::make_mut(
                self.resolve_features
                    .entry(pkgid)
                    .or_insert_with(|| Rc::new(HashSet::new())),
            );

            for feature in reqs.used {
                set.insert(feature);
            }
        }

        Ok(ret)
    }

    pub fn resolve_replacements(&self) -> HashMap<PackageId, PackageId> {
        let mut replacements = HashMap::new();
        let mut cur = &self.resolve_replacements;
        while let Some(ref node) = cur.head {
            let (k, v) = node.0;
            replacements.insert(k, v);
            cur = &node.1;
        }
        replacements
    }

    pub fn graph(&self) -> Graph<PackageId, Vec<Dependency>> {
        let mut graph: Graph<PackageId, Vec<Dependency>> = Graph::new();
        let mut cur = &self.resolve_graph;
        while let Some(ref node) = cur.head {
            match node.0 {
                GraphNode::Add(ref p) => graph.add(p.clone()),
                GraphNode::Link(ref a, ref b, ref dep) => {
                    graph.link(a.clone(), b.clone()).push(dep.clone());
                }
            }
            cur = &node.1;
        }
        graph
    }
}

/// Takes requested features for a single package from the input Method and
/// recurses to find all requested features, dependencies and requested
/// dependency features in a Requirements object, returning it to the resolver.
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

impl<'r> Requirements<'r> {
    fn new(summary: &Summary) -> Requirements<'_> {
        Requirements {
            summary,
            deps: HashMap::new(),
            used: HashSet::new(),
            visited: HashSet::new(),
        }
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
                    "Cyclic feature dependency: feature `{}` depends on itself",
                    feat
                ),
                _ => {}
            }
            self.require_value(fv)?;
        }
        Ok(())
    }

    fn require_value<'f>(&mut self, fv: &'f FeatureValue) -> CargoResult<()> {
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
