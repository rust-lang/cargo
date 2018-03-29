use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use core::{Dependency, PackageId, SourceId, Summary};
use core::interning::InternedString;
use util::Graph;
use util::CargoResult;

use super::types::{ActivateResult, ConflictReason, DepInfo, GraphNode, Method, RcList};
use super::types::RegistryQueryer;

pub use super::encode::{EncodableDependency, EncodablePackageId, EncodableResolve};
pub use super::encode::{Metadata, WorkspaceResolve};
pub use super::resolve::Resolve;

// A `Context` is basically a bunch of local resolution information which is
// kept around for all `BacktrackFrame` instances. As a result, this runs the
// risk of being cloned *a lot* so we want to make this as cheap to clone as
// possible.
#[derive(Clone)]
pub struct Context {
    // TODO: Both this and the two maps below are super expensive to clone. We should
    //       switch to persistent hash maps if we can at some point or otherwise
    //       make these much cheaper to clone in general.
    pub activations: Activations,
    pub resolve_features: HashMap<PackageId, HashSet<InternedString>>,
    pub links: HashMap<InternedString, PackageId>,

    // These are two cheaply-cloneable lists (O(1) clone) which are effectively
    // hash maps but are built up as "construction lists". We'll iterate these
    // at the very end and actually construct the map that we're making.
    pub resolve_graph: RcList<GraphNode>,
    pub resolve_replacements: RcList<(PackageId, PackageId)>,

    // These warnings are printed after resolution.
    pub warnings: RcList<String>,
}

impl Context {
    /// Activate this summary by inserting it into our list of known activations.
    ///
    /// Returns true if this summary with the given method is already activated.
    pub fn flag_activated(&mut self, summary: &Summary, method: &Method) -> CargoResult<bool> {
        let id = summary.package_id();
        let prev = self.activations
            .entry((id.name(), id.source_id().clone()))
            .or_insert_with(|| Rc::new(Vec::new()));
        if !prev.iter().any(|c| c == summary) {
            self.resolve_graph.push(GraphNode::Add(id.clone()));
            if let Some(link) = summary.links() {
                ensure!(
                    self.links.insert(link, id.clone()).is_none(),
                    "Attempting to resolve a with more then one crate with the links={}. \n\
                     This will not build as is. Consider rebuilding the .lock file.",
                    &*link
                );
            }
            let mut inner: Vec<_> = (**prev).clone();
            inner.push(summary.clone());
            *prev = Rc::new(inner);
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
        Ok(match self.resolve_features.get(id) {
            Some(prev) => {
                features
                    .iter()
                    .all(|f| prev.contains(&InternedString::new(f)))
                    && (!use_default || prev.contains(&InternedString::new("default"))
                        || !has_default_feature)
            }
            None => features.is_empty() && (!use_default || !has_default_feature),
        })
    }

    pub fn build_deps(
        &mut self,
        registry: &mut RegistryQueryer,
        parent: Option<&Summary>,
        candidate: &Summary,
        method: &Method,
    ) -> ActivateResult<Vec<DepInfo>> {
        // First, figure out our set of dependencies based on the requested set
        // of features. This also calculates what features we're going to enable
        // for our own dependencies.
        let deps = self.resolve_features(parent, candidate, method)?;

        // Next, transform all dependencies into a list of possible candidates
        // which can satisfy that dependency.
        let mut deps = deps.into_iter()
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
            .get(&(dep.name(), dep.source_id().clone()))
            .map(|v| &v[..])
            .unwrap_or(&[])
    }

    fn is_active(&self, id: &PackageId) -> bool {
        self.activations
            .get(&(id.name(), id.source_id().clone()))
            .map(|v| v.iter().any(|s| s.package_id() == id))
            .unwrap_or(false)
    }

    /// checks whether all of `parent` and the keys of `conflicting activations`
    /// are still active
    pub fn is_conflicting(
        &self,
        parent: Option<&PackageId>,
        conflicting_activations: &HashMap<PackageId, ConflictReason>,
    ) -> bool {
        conflicting_activations
            .keys()
            .chain(parent)
            .all(|id| self.is_active(id))
    }

    /// Return all dependencies and the features we want from them.
    fn resolve_features<'b>(
        &mut self,
        parent: Option<&Summary>,
        s: &'b Summary,
        method: &'b Method,
    ) -> ActivateResult<Vec<(Dependency, Vec<String>)>> {
        let dev_deps = match *method {
            Method::Everything => true,
            Method::Required { dev_deps, .. } => dev_deps,
        };

        // First, filter by dev-dependencies
        let deps = s.dependencies();
        let deps = deps.iter().filter(|d| d.is_transitive() || dev_deps);

        let mut reqs = build_requirements(s, method)?;
        let mut ret = Vec::new();

        // Next, collect all actually enabled dependencies and their features.
        for dep in deps {
            // Skip optional dependencies, but not those enabled through a feature
            if dep.is_optional() && !reqs.deps.contains_key(&*dep.name()) {
                continue;
            }
            // So we want this dependency.  Move the features we want from `feature_deps`
            // to `ret`.
            let base = reqs.deps.remove(&*dep.name()).unwrap_or((false, vec![]));
            if !dep.is_optional() && base.0 {
                self.warnings.push(format!(
                    "Package `{}` does not have feature `{}`. It has a required dependency \
                     with that name, but only optional dependencies can be used as features. \
                     This is currently a warning to ease the transition, but it will become an \
                     error in the future.",
                    s.package_id(),
                    dep.name()
                ));
            }
            let mut base = base.1;
            base.extend(dep.features().iter().cloned());
            for feature in base.iter() {
                if feature.contains('/') {
                    return Err(
                        format_err!("feature names may not contain slashes: `{}`", feature).into(),
                    );
                }
            }
            ret.push((dep.clone(), base));
        }

        // Any remaining entries in feature_deps are bugs in that the package does not actually
        // have those dependencies.  We classified them as dependencies in the first place
        // because there is no such feature, either.
        if !reqs.deps.is_empty() {
            let unknown = reqs.deps.keys().map(|s| &s[..]).collect::<Vec<&str>>();
            let features = unknown.join(", ");
            return Err(match parent {
                None => format_err!(
                    "Package `{}` does not have these features: `{}`",
                    s.package_id(),
                    features
                ).into(),
                Some(p) => (
                    p.package_id().clone(),
                    ConflictReason::MissingFeatures(features),
                ).into(),
            });
        }

        // Record what list of features is active for this package.
        if !reqs.used.is_empty() {
            let pkgid = s.package_id();

            let set = self.resolve_features
                .entry(pkgid.clone())
                .or_insert_with(HashSet::new);
            for feature in reqs.used {
                set.insert(InternedString::new(feature));
            }
        }

        Ok(ret)
    }

    pub fn resolve_replacements(&self) -> HashMap<PackageId, PackageId> {
        let mut replacements = HashMap::new();
        let mut cur = &self.resolve_replacements;
        while let Some(ref node) = cur.head {
            let (k, v) = node.0.clone();
            replacements.insert(k, v);
            cur = &node.1;
        }
        replacements
    }

    pub fn graph(&self) -> Graph<PackageId> {
        let mut graph = Graph::new();
        let mut cur = &self.resolve_graph;
        while let Some(ref node) = cur.head {
            match node.0 {
                GraphNode::Add(ref p) => graph.add(p.clone(), &[]),
                GraphNode::Link(ref a, ref b) => graph.link(a.clone(), b.clone()),
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
    method: &'b Method,
) -> CargoResult<Requirements<'a>> {
    let mut reqs = Requirements::new(s);
    match *method {
        Method::Everything
        | Method::Required {
            all_features: true, ..
        } => {
            for key in s.features().keys() {
                reqs.require_feature(key)?;
            }
            for dep in s.dependencies().iter().filter(|d| d.is_optional()) {
                reqs.require_dependency(dep.name().to_inner());
            }
        }
        Method::Required {
            features: requested_features,
            ..
        } => for feat in requested_features.iter() {
            reqs.add_feature(feat)?;
        },
    }
    match *method {
        Method::Everything
        | Method::Required {
            uses_default_features: true,
            ..
        } => {
            if s.features().get("default").is_some() {
                reqs.require_feature("default")?;
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
    deps: HashMap<&'a str, (bool, Vec<String>)>,
    // The used features set is the set of features which this local package had
    // enabled, which is later used when compiling to instruct the code what
    // features were enabled.
    used: HashSet<&'a str>,
    visited: HashSet<&'a str>,
}

impl<'r> Requirements<'r> {
    fn new<'a>(summary: &'a Summary) -> Requirements<'a> {
        Requirements {
            summary,
            deps: HashMap::new(),
            used: HashSet::new(),
            visited: HashSet::new(),
        }
    }

    fn require_crate_feature(&mut self, package: &'r str, feat: &'r str) {
        self.used.insert(package);
        self.deps
            .entry(package)
            .or_insert((false, Vec::new()))
            .1
            .push(feat.to_string());
    }

    fn seen(&mut self, feat: &'r str) -> bool {
        if self.visited.insert(feat) {
            self.used.insert(feat);
            false
        } else {
            true
        }
    }

    fn require_dependency(&mut self, pkg: &'r str) {
        if self.seen(pkg) {
            return;
        }
        self.deps.entry(pkg).or_insert((false, Vec::new())).0 = true;
    }

    fn require_feature(&mut self, feat: &'r str) -> CargoResult<()> {
        if self.seen(feat) {
            return Ok(());
        }
        for f in self.summary
            .features()
            .get(feat)
            .expect("must be a valid feature")
        {
            if f == feat {
                bail!(
                    "Cyclic feature dependency: feature `{}` depends on itself",
                    feat
                );
            }
            self.add_feature(f)?;
        }
        Ok(())
    }

    fn add_feature(&mut self, feat: &'r str) -> CargoResult<()> {
        if feat.is_empty() {
            return Ok(());
        }

        // If this feature is of the form `foo/bar`, then we just lookup package
        // `foo` and enable its feature `bar`. Otherwise this feature is of the
        // form `foo` and we need to recurse to enable the feature `foo` for our
        // own package, which may end up enabling more features or just enabling
        // a dependency.
        let mut parts = feat.splitn(2, '/');
        let feat_or_package = parts.next().unwrap();
        match parts.next() {
            Some(feat) => {
                self.require_crate_feature(feat_or_package, feat);
            }
            None => {
                if self.summary.features().contains_key(feat_or_package) {
                    self.require_feature(feat_or_package)?;
                } else {
                    self.require_dependency(feat_or_package);
                }
            }
        }
        Ok(())
    }
}

pub type Activations = HashMap<(InternedString, SourceId), Rc<Vec<Summary>>>;
