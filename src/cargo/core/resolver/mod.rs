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
//!   activatd (as we're only allowed to have one).
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
use util::{CargoResult, Graph, human, ChainError, CargoError};
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
type DepInfo = (Rc<Dependency>, Vec<Rc<Summary>>, Vec<String>);

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

    pub fn query(&self, spec: &str) -> CargoResult<&PackageId> {
        let spec = try!(PackageIdSpec::parse(spec).chain_error(|| {
            human(format!("invalid package id specification: `{}`", spec))
        }));
        let mut ids = self.iter().filter(|p| spec.matches(*p));
        let ret = match ids.next() {
            Some(id) => id,
            None => return Err(human(format!("package id specification `{}` \
                                              matched no packages", spec))),
        };
        return match ids.next() {
            Some(other) => {
                let mut msg = format!("There are multiple `{}` packages in \
                                       your project, and the specification \
                                       `{}` is ambiguous.\n\
                                       Please re-run this command \
                                       with `-p <spec>` where `<spec>` is one \
                                       of the following:",
                                      spec.name(), spec);
                let mut vec = vec![ret, other];
                vec.extend(ids);
                minimize(&mut msg, vec, &spec);
                Err(human(msg))
            }
            None => Ok(ret)
        };

        fn minimize(msg: &mut String,
                    ids: Vec<&PackageId>,
                    spec: &PackageIdSpec) {
            let mut version_cnt = HashMap::new();
            for id in ids.iter() {
                *version_cnt.entry(id.version()).or_insert(0) += 1;
            }
            for id in ids.iter() {
                if version_cnt[id.version()] == 1 {
                    msg.push_str(&format!("\n  {}:{}", spec.name(),
                                          id.version()));
                } else {
                    msg.push_str(&format!("\n  {}",
                                          PackageIdSpec::from_package_id(*id)));
                }
            }
        }
    }

    pub fn features(&self, pkg: &PackageId) -> Option<&HashSet<String>> {
        self.features.get(pkg)
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
    debug!("resolve; summary={}", summary.package_id());
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
    debug!("activating {}", parent.package_id());

    let deps = try!(cx.build_deps(registry, &parent, method));

    Ok(Some(DepsFrame{
        parent: parent,
        remaining_siblings: RcVecIter::new(deps),
        id: id,
    }))
}

#[derive(Clone)]
struct RcVecIter<Elem> {
    vec: Rc<Vec<Elem>>,
    rest: Range<usize>,
}

impl<Elem> RcVecIter<Elem> {
    fn new(vec: Vec<Elem>) -> RcVecIter<Elem> {
        RcVecIter {
            rest: 0..vec.len(),
            vec: Rc::new(vec),
        }
    }
}
impl<Elem> Iterator for RcVecIter<Elem> where Elem: Clone {
    type Item = (usize, Elem);
    fn next(&mut self) -> Option<(usize, Elem)> {
        self.rest.next().and_then(|i| self.vec.get(i).map(|val| (i, val.clone())))
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
    // For building an activation error:
    parent: Rc<Summary>,
    cur: usize,
    dep: Rc<Dependency>,
    prev_active: Vec<Rc<Summary>>,
    all_candidates: Vec<Rc<Summary>>,
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
    let mut backtrack_stack: Vec<BacktrackFrame> = Vec::new();
    let mut remaining_deps: Vec<DepsFrame> = Vec::new();
    remaining_deps.extend(try!(activate(&mut cx, registry, top, &top_method)));
    loop {
        // Retrieves the next dependency to try, from `remaining_deps`.
        let (parent, cur, dep, candidates, features) =
            match remaining_deps.pop() {
                None => break,
                Some(mut deps_frame) => {
                    match deps_frame.remaining_siblings.next() {
                        Some((cur, (dep, candidates, features))) => {
                            let parent = deps_frame.parent.clone();
                            remaining_deps.push(deps_frame);
                            (parent, cur, dep, candidates, features)
                        }
                        None => {
                            cx.visited.remove(&deps_frame.id);
                            continue
                        }
                    }
                }
            };

        let method = Method::Required {
            dev_deps: false,
            features: &features,
            uses_default_features: dep.uses_default_features(),
        };

        let prev_active: Vec<Rc<Summary>> = cx.prev_active(&dep).to_vec();
        debug!("{}[{}]>{} {} candidates", parent.name(), cur, dep.name(),
               candidates.len());
        debug!("{}[{}]>{} {} prev activations", parent.name(), cur,
               dep.name(), prev_active.len());

        // Filter the set of candidates based on the previously activated
        // versions for this dependency. We can actually use a version if it
        // precisely matches an activated version or if it is otherwise
        // incompatible with all other activated versions. Note that we define
        // "compatible" here in terms of the semver sense where if the left-most
        // nonzero digit is the same they're considered compatible.
        let my_candidates: Vec<Rc<Summary>> = candidates.iter().filter(|&b| {
            prev_active.iter().any(|a| a == b) ||
                prev_active.iter().all(|a| {
                    !compatible(a.version(), b.version())
                })
        }).cloned().collect();

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
        let candidate: Rc<Summary> = match remaining_candidates.next() {
            Some((_, candidate)) => {
                // We have a candidate. Add an entry to the `backtrack_stack` so
                // we can try the next one if this one fails.
                backtrack_stack.push(BacktrackFrame {
                    context_backup: cx.clone(),
                    deps_backup: remaining_deps.clone(),
                    remaining_candidates: remaining_candidates,
                    parent: parent.clone(),
                    cur: cur,
                    dep: dep.clone(),
                    prev_active: prev_active,
                    all_candidates: candidates,
                });
                candidate
            }
            None => {
                // This dependency has no valid candidate. Backtrack until we
                // find a dependency that does have a candidate to try, and try
                // to activate that one.  This resets the `remaining_deps` to
                // their state at the found level of the `backtrack_stack`.
                debug!("{}[{}]>{} -- None", parent.name(), cur, dep.name());
                let last_err = activation_error(&cx, registry, None, &parent, &dep,
                                                &prev_active, &candidates);
                try!(find_candidate(&mut backtrack_stack, &mut cx, &mut remaining_deps,
                                    registry, last_err))
            }
        };

        debug!("{}[{}]>{} trying {}", parent.name(), cur, dep.name(),
               candidate.version());
        cx.resolve.graph.link(parent.package_id().clone(),
                              candidate.package_id().clone());

        // If we hit an intransitive dependency then clear out the visitation
        // list as we can't induce a cycle through transitive dependencies.
        if !dep.is_transitive() {
            cx.visited.clear();
        }
        remaining_deps.extend(try!(activate(&mut cx, registry, candidate, &method)));
    }
    debug!("resolved: {:?}", cx.resolve);
    Ok(cx.resolve)
}

// Searches up `backtrack_stack` until it finds a dependency with remaining
// candidates. Resets `cx` and `remaining_deps` to that level and returns the
// next candidate. If all candidates have been exhausted, returns an activation
// error.
fn find_candidate(backtrack_stack: &mut Vec<BacktrackFrame>,
                  cx: &mut Context, remaining_deps: &mut Vec<DepsFrame>,
                  registry: &mut Registry,
                  mut last_err: Box<CargoError>) -> CargoResult<Rc<Summary>> {
    while let Some(mut frame) = backtrack_stack.pop() {
        match frame.remaining_candidates.next() {
            None => {
                debug!("{}[{}]>{} -- {:?}", frame.parent.name(), frame.cur, frame.dep.name(),
                       Some(&last_err));
                last_err = activation_error(cx, registry, Some(last_err), &frame.parent, &frame.dep,
                                            &frame.prev_active, &frame.all_candidates);
            }
            Some((_, candidate)) => {
                *cx = frame.context_backup.clone();
                *remaining_deps = frame.deps_backup.clone();
                backtrack_stack.push(frame);
                return Ok(candidate);
            }
        };
    }
    return Err(last_err);
}

#[allow(deprecated)] // connect => join in 1.3
fn activation_error(cx: &Context,
                    registry: &mut Registry,
                    err: Option<Box<CargoError>>,
                    parent: &Summary,
                    dep: &Dependency,
                    prev_active: &[Rc<Summary>],
                    candidates: &[Rc<Summary>]) -> Box<CargoError> {
    match err {
        Some(e) => return e,
        None => {}
    }
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
    let new_dep = dep.clone().set_version_req(all_req);
    let mut candidates: Vec<Summary> = match registry.query(&new_dep) {
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
        let deps: Vec<(Rc<Dependency>, Vec<String>)> =
            try!(self.resolve_features(parent, method));

        // Next, transform all dependencies into a list of possible candidates
        // which can satisfy that dependency.
        let mut deps: Vec<DepInfo> = try!(deps.into_iter().map(|(dep, features)| {
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
            -> CargoResult<Vec<(Rc<Dependency>, Vec<String>)>> {
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
            ret.push((Rc::new(dep.clone()), base));
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
