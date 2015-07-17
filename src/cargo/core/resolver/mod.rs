use std::cell::RefCell;
use std::collections::HashSet;
use std::collections::hash_map::HashMap;
use std::fmt;
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
    Required{ dev_deps: bool,
              features: &'a [String],
              uses_default_features: bool,
              target_platform: Option<&'a str>},
}

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
    visited: Rc<RefCell<HashSet<PackageId>>>,
}

/// Builds the list of all packages required to build the first argument.
pub fn resolve(summary: &Summary, method: Method,
               registry: &mut Registry) -> CargoResult<Resolve> {
    trace!("resolve; summary={}", summary.package_id());
    let summary = Rc::new(summary.clone());

    let cx = Box::new(Context {
        resolve: Resolve::new(summary.package_id().clone()),
        activations: HashMap::new(),
        visited: Rc::new(RefCell::new(HashSet::new())),
    });
    let _p = profile::start(format!("resolving: {}", summary.package_id()));
    match try!(activate(cx, registry, &summary, method)) {
        Ok(cx) => {
            debug!("resolved: {:?}", cx.resolve);
            Ok(cx.resolve)
        }
        Err(e) => Err(e),
    }
}

fn activate(mut cx: Box<Context>,
            registry: &mut Registry,
            parent: &Rc<Summary>,
            method: Method)
            -> CargoResult<CargoResult<Box<Context>>> {
    // Dependency graphs are required to be a DAG, so we keep a set of
    // packages we're visiting and bail if we hit a dupe.
    let id = parent.package_id();
    if !cx.visited.borrow_mut().insert(id.clone()) {
        return Err(human(format!("cyclic package dependency: package `{}` \
                                  depends on itself", id)))
    }

    // If we're already activated, then that was easy!
    if flag_activated(&mut *cx, parent, &method) {
        cx.visited.borrow_mut().remove(id);
        return Ok(Ok(cx))
    }
    debug!("activating {}", parent.package_id());

    // Extracting the platform request.
    let platform = match method {
        Method::Required{target_platform: platform, ..} => platform,
        Method::Everything => None,
    };

    // First, figure out our set of dependencies based on the requsted set of
    // features. This also calculates what features we're going to enable for
    // our own dependencies.
    let deps = try!(resolve_features(&mut cx, parent, method));

    // Next, transform all dependencies into a list of possible candidates which
    // can satisfy that dependency.
    let mut deps = try!(deps.into_iter().map(|(dep, features)| {
        let mut candidates = try!(registry.query(dep));
        // When we attempt versions for a package, we'll want to start at the
        // maximum version and work our way down.
        candidates.sort_by(|a, b| {
            b.version().cmp(a.version())
        });
        let candidates = candidates.into_iter().map(Rc::new).collect::<Vec<_>>();
        Ok((dep, candidates, features))
    }).collect::<CargoResult<Vec<_>>>());

    // When we recurse, attempt to resolve dependencies with fewer candidates
    // before recursing on dependencies with more candidates. This way if the
    // dependency with only one candidate can't be resolved we don't have to do
    // a bunch of work before we figure that out.
    deps.sort_by(|&(_, ref a, _), &(_, ref b, _)| {
        a.len().cmp(&b.len())
    });

    // Workaround compilation error: `deps` does not live long enough
    let platform = platform.map(|s| &*s);

    Ok(match try!(activate_deps(cx, registry, parent, platform, &deps, 0)) {
        Ok(cx) => {
            cx.visited.borrow_mut().remove(parent.package_id());
            Ok(cx)
        }
        Err(e) => Err(e),
    })
}

// Activate this summary by inserting it into our list of known activations.
//
// Returns if this summary with the given method is already activated.
fn flag_activated(cx: &mut Context,
                  summary: &Rc<Summary>,
                  method: &Method) -> bool {
    let id = summary.package_id();
    let key = (id.name().to_string(), id.source_id().clone());
    let prev = cx.activations.entry(key).or_insert(Vec::new());
    if !prev.iter().any(|c| c == summary) {
        cx.resolve.graph.add(id.clone(), &[]);
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
    match cx.resolve.features(id) {
        Some(prev) => {
            features.iter().all(|f| prev.contains(f)) &&
                (!use_default || prev.contains("default") || !has_default_feature)
        }
        None => features.len() == 0 && (!use_default || !has_default_feature)
    }
}

fn activate_deps<'a>(cx: Box<Context>,
                     registry: &mut Registry,
                     parent: &Summary,
                     platform: Option<&'a str>,
                     deps: &'a [(&Dependency, Vec<Rc<Summary>>, Vec<String>)],
                     cur: usize) -> CargoResult<CargoResult<Box<Context>>> {
    if cur == deps.len() { return Ok(Ok(cx)) }
    let (dep, ref candidates, ref features) = deps[cur];

    let method = Method::Required{
        dev_deps: false,
        features: &features,
        uses_default_features: dep.uses_default_features(),
        target_platform: platform};

    let key = (dep.name().to_string(), dep.source_id().clone());
    let prev_active = cx.activations.get(&key)
                                    .map(|v| &v[..]).unwrap_or(&[]);
    trace!("{}[{}]>{} {} candidates", parent.name(), cur, dep.name(),
         candidates.len());
    trace!("{}[{}]>{} {} prev activations", parent.name(), cur,
         dep.name(), prev_active.len());

    // Filter the set of candidates based on the previously activated
    // versions for this dependency. We can actually use a version if it
    // precisely matches an activated version or if it is otherwise
    // incompatible with all other activated versions. Note that we define
    // "compatible" here in terms of the semver sense where if the left-most
    // nonzero digit is the same they're considered compatible.
    let my_candidates = candidates.iter().filter(|&b| {
        prev_active.iter().any(|a| a == b) ||
            prev_active.iter().all(|a| {
                !compatible(a.version(), b.version())
            })
    });

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
    let mut last_err = None;
    for candidate in my_candidates {
        trace!("{}[{}]>{} trying {}", parent.name(), cur, dep.name(),
             candidate.version());
        let mut my_cx = cx.clone();
        my_cx.resolve.graph.link(parent.package_id().clone(),
                                 candidate.package_id().clone());

        // If we hit an intransitive dependency then clear out the visitation
        // list as we can't induce a cycle through transitive dependencies.
        if !dep.is_transitive() {
            my_cx.visited.borrow_mut().clear();
        }
        let my_cx = match try!(activate(my_cx, registry, candidate, method)) {
            Ok(cx) => cx,
            Err(e) => { last_err = Some(e); continue }
        };
        match try!(activate_deps(my_cx, registry, parent, platform, deps,
                                 cur + 1)) {
            Ok(cx) => return Ok(Ok(cx)),
            Err(e) => { last_err = Some(e); }
        }
    }
    trace!("{}[{}]>{} -- {:?}", parent.name(), cur, dep.name(),
         last_err);

    // Oh well, we couldn't activate any of the candidates, so we just can't
    // activate this dependency at all
    Ok(activation_error(&cx, registry, last_err, parent, dep, prev_active,
                        &candidates))
}

fn activation_error(cx: &Context,
                    registry: &mut Registry,
                    err: Option<Box<CargoError>>,
                    parent: &Summary,
                    dep: &Dependency,
                    prev_active: &[Rc<Summary>],
                    candidates: &[Rc<Summary>]) -> CargoResult<Box<Context>> {
    match err {
        Some(e) => return Err(e),
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

        return Err(human(msg))
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
    let mut candidates = try!(registry.query(&new_dep));
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
    Err(human(msg))
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

fn resolve_features<'a>(cx: &mut Context, parent: &'a Summary,
                        method: Method)
                        -> CargoResult<Vec<(&'a Dependency, Vec<String>)>> {
    let dev_deps = match method {
        Method::Everything => true,
        Method::Required { dev_deps, .. } => dev_deps,
    };

    // First, filter by dev-dependencies
    let deps = parent.dependencies();
    let deps = deps.iter().filter(|d| d.is_transitive() || dev_deps);

    // Second, ignoring dependencies that should not be compiled for this platform
    let deps = deps.filter(|d| {
        match method {
            Method::Required{target_platform: Some(ref platform), ..} => {
                d.is_active_for_platform(platform)
            },
            _ => true
        }
    });

    let (mut feature_deps, used_features) = try!(build_features(parent, method));
    let mut ret = Vec::new();

    // Next, sanitize all requested features by whitelisting all the requested
    // features that correspond to optional dependencies
    for dep in deps {
        // weed out optional dependencies, but not those required
        if dep.is_optional() && !feature_deps.contains_key(dep.name()) {
            continue
        }
        let mut base = feature_deps.remove(dep.name()).unwrap_or(Vec::new());
        for feature in dep.features().iter() {
            base.push(feature.clone());
            if feature.contains("/") {
                return Err(human(format!("features in dependencies \
                                          cannot enable features in \
                                          other dependencies: `{}`",
                                         feature)));
            }
        }
        ret.push((dep, base));
    }

    // All features can only point to optional dependencies, in which case they
    // should have all been weeded out by the above iteration. Any remaining
    // features are bugs in that the package does not actually have those
    // features.
    if feature_deps.len() > 0 {
        let unknown = feature_deps.keys().map(|s| &s[..])
                                  .collect::<Vec<&str>>();
        if unknown.len() > 0 {
            let features = unknown.connect(", ");
            return Err(human(format!("Package `{}` does not have these features: \
                                      `{}`", parent.package_id(), features)))
        }
    }

    // Record what list of features is active for this package.
    if used_features.len() > 0 {
        let pkgid = parent.package_id();
        cx.resolve.features.entry(pkgid.clone())
          .or_insert(HashSet::new())
          .extend(used_features);
    }

    Ok(ret)
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
fn build_features(s: &Summary, method: Method)
                  -> CargoResult<(HashMap<String, Vec<String>>, HashSet<String>)> {
    let mut deps = HashMap::new();
    let mut used = HashSet::new();
    let mut visited = HashSet::new();
    match method {
        Method::Everything => {
            for key in s.features().keys() {
                try!(add_feature(s, key, &mut deps, &mut used, &mut visited));
            }
            for dep in s.dependencies().iter().filter(|d| d.is_optional()) {
                try!(add_feature(s, dep.name(), &mut deps, &mut used,
                                 &mut visited));
            }
        }
        Method::Required{features: requested_features, ..} =>  {
            for feat in requested_features.iter() {
                try!(add_feature(s, feat, &mut deps, &mut used, &mut visited));
            }
        }
    }
    match method {
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
