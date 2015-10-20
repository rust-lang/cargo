//! Resolution of the entire dependency graph for a crate
//!
//! This module implements the core logic in taking the world of crates and
//! constraints and creating a resolved graph with locked versions for all
//! crates and their dependencies. This is separate from the registry module
//! which is more worried about discovering crates from various sources, this
//! module just uses the Registry trait as a source to learn about crates from.
//!
//! Actually solving a constraint graph is an NP-hard (or NP-complete, I forget
//! which) problem, this the algorithm is basically a nice heuristic to make
//! sure we get roughly the best answer most of the time. The constraints that
//! we're working with are:
//!
//! 1. Each crate can have any number of dependencies. Each dependency can
//!    declare a version range that it is compatible with.
//! 2. Crates can be activated with multiple version (e.g. show up in the
//!    dependency graph twice) so long as each pairwise instance have
//!    semver-incompatible versions.
//!
//! The algorithm employed here is fairly simple, we simply do a DFS, activating
//! the "newest crate" (highest version) first and then going to the next
//! option. The heuristics we employ are:
//!
//! * Never try to activate a crate version which is incompatible. This means we
//!   only try crates which will actually satisfy a dependency and we won't ever
//!   try to activate a crate that's semver compatible with something else
//!   activated (as we're only allowed to have one).
//! * Always try to activate the highest version crate first. The default
//!   dependency in Cargo (e.g. when you write `foo = "0.1.2"`) is
//!   semver-compatible, so selecting the highest version possible will allow us
//!   to hopefully satisfy as many dependencies at once.
//!
//! Beyond that, what's implemented below is just a naive backtracking version
//! which should in theory try all possible combinations of dependencies and
//! versions to see if one works. The first resolution that works causes
//! everything to bail out immediately and return success, and only if *nothing*
//! works do we actually return an error up the stack.
//!
//! ## Performance
//!
//! Note that this is a relatively performance-critical portion of Cargo. The
//! data that we're processing is proportional to the size of the dependency
//! graph, which can often be quite large (e.g. take a look at Servo). To make
//! matters worse the DFS algorithm we're implemented is inherently quite
//! inefficient. When we add the requirement of backtracking on top it means
//! that we're implementing something that probably shouldn't be allocating all
//! over the place.

use std::collections::HashSet;
use std::collections::hash_map::HashMap;
use std::fmt;
use std::ops::Range;
use std::rc::Rc;
use semver;

use core::{PackageId, Registry, SourceId, Summary, Dependency};
use core::PackageIdSpec;
use util::{CargoResult, Graph, human, CargoError};
use util::profile;
use util::graph::{Nodes, Edges};

pub use self::encode::{EncodableResolve, EncodableDependency, EncodablePackageId};
pub use self::encode::Metadata;

mod encode;

/// Represents a fully resolved package dependency graph. Each node in the graph
/// is a package and edges represent dependencies between packages.
///
/// Each instance of `Resolve` also understands the full set of features used
/// for each package as well as what the root package is.
#[derive(PartialEq, Eq, Clone)]
pub struct Resolve {
    graph: Graph<PackageId>,
    features: HashMap<PackageId, HashSet<String>>,
    root: PackageId,
    metadata: Option<Metadata>,
}

#[derive(Clone, Copy)]
pub enum Method<'a> {
    Everything,
    Required {
        dev_deps: bool,
        features: &'a [String],
        uses_default_features: bool,
    },
}

// Err(..) == standard transient error (e.g. I/O error)
// Ok(Err(..)) == resolve error, but is human readable
// Ok(Ok(..)) == success in resolving
type ResolveResult = CargoResult<CargoResult<Box<Context>>>;

// Information about the dependencies for a crate, a tuple of:
//
// (dependency info, candidates, features activated)
type DepInfo = (Dependency, Vec<Rc<Summary>>, Vec<String>);

impl Resolve {
    fn new(root: PackageId) -> Resolve {
        let mut g = Graph::new();
        g.add(root.clone(), &[]);
        Resolve { graph: g, root: root, features: HashMap::new(), metadata: None }
    }

    pub fn copy_metadata(&mut self, other: &Resolve) {
        self.metadata = other.metadata.clone();
    }

    pub fn iter(&self) -> Nodes<PackageId> {
        self.graph.iter()
    }

    pub fn root(&self) -> &PackageId { &self.root }

    pub fn deps(&self, pkg: &PackageId) -> Option<Edges<PackageId>> {
        self.graph.edges(pkg)
    }

    pub fn features(&self, pkg: &PackageId) -> Option<&HashSet<String>> {
        self.features.get(pkg)
    }

    pub fn query(&self, spec: &str) -> CargoResult<&PackageId> {
        PackageIdSpec::query_str(spec, self.iter())
    }
}

impl fmt::Debug for Resolve {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(fmt, "graph: {:?}\n", self.graph));
        try!(write!(fmt, "\nfeatures: {{\n"));
        for (pkg, features) in &self.features {
            try!(write!(fmt, "  {}: {:?}\n", pkg, features));
        }
        write!(fmt, "}}")
    }
}

#[derive(Clone)]
struct Context {
    activations: HashMap<(String, SourceId), Vec<Rc<Summary>>>,
    resolve: Resolve,
    visited: HashSet<PackageId>,
}

/// Builds the list of all packages required to build the first argument.
pub fn resolve(summary: &Summary, method: &Method,
               registry: &mut Registry) -> CargoResult<Resolve> {
    trace!("resolve; summary={}", summary.package_id());
    let summary = Rc::new(summary.clone());

    let cx = Context {
        resolve: Resolve::new(summary.package_id().clone()),
        activations: HashMap::new(),
        visited: HashSet::new(),
    };
    let _p = profile::start(format!("resolving: {}", summary.package_id()));
    activate_deps_loop(cx, registry, summary, method)
}

/// Attempts to activate the summary `parent` in the context `cx`.
///
/// This function will pull dependency summaries from the registry provided, and
/// the dependencies of the package will be determined by the `method` provided.
/// If `parent` was activated, this function returns the dependency frame to
/// iterate through next.
fn activate(cx: &mut Context,
            registry: &mut Registry,
            parent: Rc<Summary>,
            method: &Method)
            -> CargoResult<Option<DepsFrame>> {
    // Dependency graphs are required to be a DAG, so we keep a set of
    // packages we're visiting and bail if we hit a dupe.
    let id = parent.package_id().clone();
    if !cx.visited.insert(id.clone()) {
        return Err(human(format!("cyclic package dependency: package `{}` \
                                  depends on itself", id)))
    }

    // If we're already activated, then that was easy!
    if cx.flag_activated(&parent, method) {
        cx.visited.remove(&id);
        return Ok(None);
    }
    trace!("activating {}", parent.package_id());

    let deps = try!(cx.build_deps(registry, &parent, method));

    Ok(Some(DepsFrame{
        parent: parent,
        remaining_siblings: RcVecIter::new(deps),
        id: id,
    }))
}

#[derive(Clone)]
struct RcVecIter<T> {
    vec: Rc<Vec<T>>,
    rest: Range<usize>,
}

impl<T> RcVecIter<T> {
    fn new(vec: Vec<T>) -> RcVecIter<T> {
        RcVecIter {
            rest: 0..vec.len(),
            vec: Rc::new(vec),
        }
    }
    fn cur_index(&self) -> usize {
        self.rest.start - 1
    }
}
impl<T> Iterator for RcVecIter<T> where T: Clone {
    type Item = (usize, T);
    fn next(&mut self) -> Option<(usize, T)> {
        self.rest.next().and_then(|i| {
            self.vec.get(i).map(|val| (i, val.clone()))
        })
    }
}

#[derive(Clone)]
struct DepsFrame {
    parent: Rc<Summary>,
    remaining_siblings: RcVecIter<DepInfo>,
    id: PackageId,
}

struct BacktrackFrame {
    context_backup: Context,
    deps_backup: Vec<DepsFrame>,
    remaining_candidates: RcVecIter<Rc<Summary>>,
    parent: Rc<Summary>,
    dep: Dependency,
}

/// Recursively activates the dependencies for `top`, in depth-first order,
/// backtracking across possible candidates for each dependency as necessary.
///
/// If all dependencies can be activated and resolved to a version in the
/// dependency graph, cx.resolve is returned.
fn activate_deps_loop(mut cx: Context,
                      registry: &mut Registry,
                      top: Rc<Summary>,
                      top_method: &Method) -> CargoResult<Resolve> {
    let mut backtrack_stack = Vec::new();
    let mut remaining_deps = Vec::new();
    remaining_deps.extend(try!(activate(&mut cx, registry, top, &top_method)));

    // Main resolution loop, this is the workhorse of the resolution algorithm.
    //
    // You'll note that a few stacks are maintained on the side, which might
    // seem odd when this algorithm looks like it could be implemented
    // recursively. While correct, this is implemented iteratively to avoid
    // blowing the stack (the recusion depth is proportional to the size of the
    // input).
    //
    // The general sketch of this loop is to run until there are no dependencies
    // left to activate, and for each dependency to attempt to activate all of
    // its own dependencies in turn. The `backtrack_stack` is a side table of
    // backtracking states where if we hit an error we can return to in order to
    // attempt to continue resolving.
    while let Some(mut deps_frame) = remaining_deps.pop() {
        let frame = match deps_frame.remaining_siblings.next() {
            Some(sibling) => {
                let parent = deps_frame.parent.clone();
                remaining_deps.push(deps_frame);
                (parent, sibling)
            }
            None => {
                cx.visited.remove(&deps_frame.id);
                continue
            }
        };
        let (mut parent, (mut cur, (mut dep, candidates, features))) = frame;
        assert!(!remaining_deps.is_empty());

        let method = Method::Required {
            dev_deps: false,
            features: &features,
            uses_default_features: dep.uses_default_features(),
        };

        let my_candidates = {
            let prev_active = cx.prev_active(&dep);
            trace!("{}[{}]>{} {} candidates", parent.name(), cur, dep.name(),
                   candidates.len());
            trace!("{}[{}]>{} {} prev activations", parent.name(), cur,
                   dep.name(), prev_active.len());

            // Filter the set of candidates based on the previously activated
            // versions for this dependency. We can actually use a version if it
            // precisely matches an activated version or if it is otherwise
            // incompatible with all other activated versions. Note that we
            // define "compatible" here in terms of the semver sense where if
            // the left-most nonzero digit is the same they're considered
            // compatible.
            candidates.iter().filter(|&b| {
                prev_active.iter().any(|a| a == b) ||
                    prev_active.iter().all(|a| {
                        !compatible(a.version(), b.version())
                    })
            }).cloned().collect()
        };

        // Alright, for each candidate that's gotten this far, it meets the
        // following requirements:
        //
        // 1. The version matches the dependency requirement listed for this
        //    package
        // 2. There are no activated versions for this package which are
        //    semver-compatible, or there's an activated version which is
        //    precisely equal to `candidate`.
        //
        // This means that we're going to attempt to activate each candidate in
        // turn. We could possibly fail to activate each candidate, so we try
        // each one in turn.
        let mut remaining_candidates = RcVecIter::new(my_candidates);
        let candidate = match remaining_candidates.next() {
            Some((_, candidate)) => {
                // We have a candidate. Add an entry to the `backtrack_stack` so
                // we can try the next one if this one fails.
                backtrack_stack.push(BacktrackFrame {
                    context_backup: cx.clone(),
                    deps_backup: remaining_deps.clone(),
                    remaining_candidates: remaining_candidates,
                    parent: parent.clone(),
                    dep: dep.clone(),
                });
                candidate
            }
            None => {
                // This dependency has no valid candidate. Backtrack until we
                // find a dependency that does have a candidate to try, and try
                // to activate that one.  This resets the `remaining_deps` to
                // their state at the found level of the `backtrack_stack`.
                trace!("{}[{}]>{} -- no candidates", parent.name(), cur,
                       dep.name());
                match find_candidate(&mut backtrack_stack, &mut cx,
                                     &mut remaining_deps, &mut parent, &mut cur,
                                     &mut dep) {
                    None => return Err(activation_error(&cx, registry, &parent,
                                                        &dep,
                                                        &cx.prev_active(&dep),
                                                        &candidates)),
                    Some(candidate) => candidate,
                }
            }
        };

        trace!("{}[{}]>{} trying {}", parent.name(), cur, dep.name(),
               candidate.version());
        cx.resolve.graph.link(parent.package_id().clone(),
                              candidate.package_id().clone());

        // If we hit an intransitive dependency then clear out the visitation
        // list as we can't induce a cycle through transitive dependencies.
        if !dep.is_transitive() {
            cx.visited.clear();
        }
        remaining_deps.extend(try!(activate(&mut cx, registry,
                                            candidate, &method)));
    }
    trace!("resolved: {:?}", cx.resolve);
    Ok(cx.resolve)
}

// Searches up `backtrack_stack` until it finds a dependency with remaining
// candidates. Resets `cx` and `remaining_deps` to that level and returns the
// next candidate. If all candidates have been exhausted, returns None.
fn find_candidate(backtrack_stack: &mut Vec<BacktrackFrame>,
                  cx: &mut Context, remaining_deps: &mut Vec<DepsFrame>,
                  parent: &mut Rc<Summary>, cur: &mut usize,
                  dep: &mut Dependency) -> Option<Rc<Summary>> {
    while let Some(mut frame) = backtrack_stack.pop() {
        if let Some((_, candidate)) = frame.remaining_candidates.next() {
            *cx = frame.context_backup.clone();
            *remaining_deps = frame.deps_backup.clone();
            *parent = frame.parent.clone();
            *cur = remaining_deps.last().unwrap().remaining_siblings.cur_index();
            *dep = frame.dep.clone();
            backtrack_stack.push(frame);
            return Some(candidate)
        }
    }
    return None
}

#[allow(deprecated)] // connect => join in 1.3
fn activation_error(cx: &Context,
                    registry: &mut Registry,
                    parent: &Summary,
                    dep: &Dependency,
                    prev_active: &[Rc<Summary>],
                    candidates: &[Rc<Summary>]) -> Box<CargoError> {
    if candidates.len() > 0 {
        let mut msg = format!("failed to select a version for `{}` \
                               (required by `{}`):\n\
                               all possible versions conflict with \
                               previously selected versions of `{}`",
                              dep.name(), parent.name(),
                              dep.name());
        'outer: for v in prev_active.iter() {
            for node in cx.resolve.graph.iter() {
                let edges = match cx.resolve.graph.edges(node) {
                    Some(edges) => edges,
                    None => continue,
                };
                for edge in edges {
                    if edge != v.package_id() { continue }

                    msg.push_str(&format!("\n  version {} in use by {}",
                                          v.version(), edge));
                    continue 'outer;
                }
            }
            msg.push_str(&format!("\n  version {} in use by ??",
                                  v.version()));
        }

        msg.push_str(&format!("\n  possible versions to select: {}",
                              candidates.iter()
                                        .map(|v| v.version())
                                        .map(|v| v.to_string())
                                        .collect::<Vec<_>>()
                                        .connect(", ")));

        return human(msg)
    }

    // Once we're all the way down here, we're definitely lost in the
    // weeds! We didn't actually use any candidates above, so we need to
    // give an error message that nothing was found.
    //
    // Note that we re-query the registry with a new dependency that
    // allows any version so we can give some nicer error reporting
    // which indicates a few versions that were actually found.
    let msg = format!("no matching package named `{}` found \
                       (required by `{}`)\n\
                       location searched: {}\n\
                       version required: {}",
                      dep.name(), parent.name(),
                      dep.source_id(),
                      dep.version_req());
    let mut msg = msg;
    let all_req = semver::VersionReq::parse("*").unwrap();
    let new_dep = dep.clone_inner().set_version_req(all_req).into_dependency();
    let mut candidates = match registry.query(&new_dep) {
        Ok(candidates) => candidates,
        Err(e) => return e,
    };
    candidates.sort_by(|a, b| {
        b.version().cmp(a.version())
    });
    if candidates.len() > 0 {
        msg.push_str("\nversions found: ");
        for (i, c) in candidates.iter().take(3).enumerate() {
            if i != 0 { msg.push_str(", "); }
            msg.push_str(&c.version().to_string());
        }
        if candidates.len() > 3 {
            msg.push_str(", ...");
        }
    }

    // If we have a path dependency with a locked version, then this may
    // indicate that we updated a sub-package and forgot to run `cargo
    // update`. In this case try to print a helpful error!
    if dep.source_id().is_path() &&
       dep.version_req().to_string().starts_with("=") &&
       candidates.len() > 0 {
        msg.push_str("\nconsider running `cargo update` to update \
                      a path dependency's locked version");

    }
    human(msg)
}

// Returns if `a` and `b` are compatible in the semver sense. This is a
// commutative operation.
//
// Versions `a` and `b` are compatible if their left-most nonzero digit is the
// same.
fn compatible(a: &semver::Version, b: &semver::Version) -> bool {
    if a.major != b.major { return false }
    if a.major != 0 { return true }
    if a.minor != b.minor { return false }
    if a.minor != 0 { return true }
    a.patch == b.patch
}

// Returns a pair of (feature dependencies, all used features)
//
// The feature dependencies map is a mapping of package name to list of features
// enabled. Each package should be enabled, and each package should have the
// specified set of features enabled.
//
// The all used features set is the set of features which this local package had
// enabled, which is later used when compiling to instruct the code what
// features were enabled.
fn build_features(s: &Summary, method: &Method)
                  -> CargoResult<(HashMap<String, Vec<String>>, HashSet<String>)> {
    let mut deps = HashMap::new();
    let mut used = HashSet::new();
    let mut visited = HashSet::new();
    match *method {
        Method::Everything => {
            for key in s.features().keys() {
                try!(add_feature(s, key, &mut deps, &mut used, &mut visited));
            }
            for dep in s.dependencies().iter().filter(|d| d.is_optional()) {
                try!(add_feature(s, dep.name(), &mut deps, &mut used,
                                 &mut visited));
            }
        }
        Method::Required { features: requested_features, .. } =>  {
            for feat in requested_features.iter() {
                try!(add_feature(s, feat, &mut deps, &mut used, &mut visited));
            }
        }
    }
    match *method {
        Method::Everything |
        Method::Required { uses_default_features: true, .. } => {
            if s.features().get("default").is_some() {
                try!(add_feature(s, "default", &mut deps, &mut used,
                                 &mut visited));
            }
        }
        Method::Required { uses_default_features: false, .. } => {}
    }
    return Ok((deps, used));

    fn add_feature(s: &Summary, feat: &str,
                   deps: &mut HashMap<String, Vec<String>>,
                   used: &mut HashSet<String>,
                   visited: &mut HashSet<String>) -> CargoResult<()> {
        if feat.is_empty() { return Ok(()) }

        // If this feature is of the form `foo/bar`, then we just lookup package
        // `foo` and enable its feature `bar`. Otherwise this feature is of the
        // form `foo` and we need to recurse to enable the feature `foo` for our
        // own package, which may end up enabling more features or just enabling
        // a dependency.
        let mut parts = feat.splitn(2, '/');
        let feat_or_package = parts.next().unwrap();
        match parts.next() {
            Some(feat) => {
                let package = feat_or_package;
                used.insert(package.to_string());
                deps.entry(package.to_string())
                    .or_insert(Vec::new())
                    .push(feat.to_string());
            }
            None => {
                let feat = feat_or_package;
                if !visited.insert(feat.to_string()) {
                    return Err(human(format!("Cyclic feature dependency: \
                                              feature `{}` depends on itself",
                                              feat)))
                }
                used.insert(feat.to_string());
                match s.features().get(feat) {
                    Some(recursive) => {
                        for f in recursive {
                            try!(add_feature(s, f, deps, used, visited));
                        }
                    }
                    None => {
                        deps.entry(feat.to_string()).or_insert(Vec::new());
                    }
                }
                visited.remove(&feat.to_string());
            }
        }
        Ok(())
    }
}

impl Context {
    // Activate this summary by inserting it into our list of known activations.
    //
    // Returns if this summary with the given method is already activated.
    fn flag_activated(&mut self,
                      summary: &Rc<Summary>,
                      method: &Method) -> bool {
        let id = summary.package_id();
        let key = (id.name().to_string(), id.source_id().clone());
        let prev = self.activations.entry(key).or_insert(Vec::new());
        if !prev.iter().any(|c| c == summary) {
            self.resolve.graph.add(id.clone(), &[]);
            prev.push(summary.clone());
            return false
        }
        debug!("checking if {} is already activated", summary.package_id());
        let (features, use_default) = match *method {
            Method::Required { features, uses_default_features, .. } => {
                (features, uses_default_features)
            }
            Method::Everything => return false,
        };

        let has_default_feature = summary.features().contains_key("default");
        match self.resolve.features(id) {
            Some(prev) => {
                features.iter().all(|f| prev.contains(f)) &&
                    (!use_default || prev.contains("default") ||
                     !has_default_feature)
            }
            None => features.len() == 0 && (!use_default || !has_default_feature)
        }
    }

    fn build_deps(&mut self, registry: &mut Registry,
                  parent: &Summary,
                  method: &Method) -> CargoResult<Vec<DepInfo>> {
        // First, figure out our set of dependencies based on the requsted set
        // of features. This also calculates what features we're going to enable
        // for our own dependencies.
        let deps = try!(self.resolve_features(parent, method));

        // Next, transform all dependencies into a list of possible candidates
        // which can satisfy that dependency.
        let mut deps = try!(deps.into_iter().map(|(dep, features)| {
            let mut candidates = try!(registry.query(&dep));
            // When we attempt versions for a package, we'll want to start at
            // the maximum version and work our way down.
            candidates.sort_by(|a, b| {
                b.version().cmp(a.version())
            });
            let candidates = candidates.into_iter().map(Rc::new).collect();
            Ok((dep, candidates, features))
        }).collect::<CargoResult<Vec<DepInfo>>>());

        // Attempt to resolve dependencies with fewer candidates before trying
        // dependencies with more candidates.  This way if the dependency with
        // only one candidate can't be resolved we don't have to do a bunch of
        // work before we figure that out.
        deps.sort_by(|&(_, ref a, _), &(_, ref b, _)| {
            a.len().cmp(&b.len())
        });

        Ok(deps)
    }

    fn prev_active(&self, dep: &Dependency) -> &[Rc<Summary>] {
        let key = (dep.name().to_string(), dep.source_id().clone());
        self.activations.get(&key).map(|v| &v[..]).unwrap_or(&[])
    }

    #[allow(deprecated)] // connect => join in 1.3
    fn resolve_features(&mut self, parent: &Summary, method: &Method)
            -> CargoResult<Vec<(Dependency, Vec<String>)>> {
        let dev_deps = match *method {
            Method::Everything => true,
            Method::Required { dev_deps, .. } => dev_deps,
        };

        // First, filter by dev-dependencies
        let deps = parent.dependencies();
        let deps = deps.iter().filter(|d| d.is_transitive() || dev_deps);

        let (mut feature_deps, used_features) = try!(build_features(parent,
                                                                    method));
        let mut ret = Vec::new();

        // Next, sanitize all requested features by whitelisting all the
        // requested features that correspond to optional dependencies
        for dep in deps {
            // weed out optional dependencies, but not those required
            if dep.is_optional() && !feature_deps.contains_key(dep.name()) {
                continue
            }
            let mut base = feature_deps.remove(dep.name()).unwrap_or(vec![]);
            for feature in dep.features().iter() {
                base.push(feature.clone());
                if feature.contains("/") {
                    return Err(human(format!("features in dependencies \
                                              cannot enable features in \
                                              other dependencies: `{}`",
                                             feature)));
                }
            }
            ret.push((dep.clone(), base));
        }

        // All features can only point to optional dependencies, in which case
        // they should have all been weeded out by the above iteration. Any
        // remaining features are bugs in that the package does not actually
        // have those features.
        if feature_deps.len() > 0 {
            let unknown = feature_deps.keys().map(|s| &s[..])
                                      .collect::<Vec<&str>>();
            if unknown.len() > 0 {
                let features = unknown.connect(", ");
                return Err(human(format!("Package `{}` does not have these \
                                          features: `{}`", parent.package_id(),
                                         features)))
            }
        }

        // Record what list of features is active for this package.
        if used_features.len() > 0 {
            let pkgid = parent.package_id();
            self.resolve.features.entry(pkgid.clone())
                .or_insert(HashSet::new())
                .extend(used_features);
        }

        Ok(ret)
    }
}
