use std::cell::RefCell;
use std::collections::HashSet;
use std::collections::hash_map::{HashMap, Occupied, Vacant};
use std::fmt;
use std::rc::Rc;
use semver;

use core::{PackageId, Registry, SourceId, Summary, Dependency};
use core::PackageIdSpec;
use util::{CargoResult, Graph, human, ChainError};
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
#[deriving(PartialEq, Eq, Clone)]
pub struct Resolve {
    graph: Graph<PackageId>,
    features: HashMap<PackageId, HashSet<String>>,
    root: PackageId,
    metadata: Option<Metadata>,
}

pub enum ResolveMethod<'a> {
    ResolveEverything,
    ResolveRequired(/* dev_deps = */ bool,
                    /* features = */ &'a [String],
                    /* uses_default_features = */ bool,
                    /* target_platform = */ Option<&'a str>),
}

impl Resolve {
    fn new(root: PackageId) -> Resolve {
        let mut g = Graph::new();
        g.add(root.clone(), []);
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
                                      spec.get_name(), spec);
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
                let slot = match version_cnt.entry(id.get_version()) {
                    Occupied(e) => e.into_mut(),
                    Vacant(e) => e.set(0u),
                };
                *slot += 1;
            }
            for id in ids.iter() {
                if version_cnt[id.get_version()] == 1 {
                    msg.push_str(format!("\n  {}:{}", spec.get_name(),
                                 id.get_version()).as_slice());
                } else {
                    msg.push_str(format!("\n  {}",
                                         PackageIdSpec::from_package_id(*id))
                                        .as_slice());
                }
            }
        }
    }

    pub fn features(&self, pkg: &PackageId) -> Option<&HashSet<String>> {
        self.features.find(pkg)
    }
}

impl fmt::Show for Resolve {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        self.graph.fmt(fmt)
    }
}

#[deriving(Clone)]
struct Context {
    activations: HashMap<(String, SourceId), Vec<Rc<Summary>>>,
    resolve: Resolve,
    visited: Rc<RefCell<HashSet<PackageId>>>,
}

/// Builds the list of all packages required to build the first argument.
pub fn resolve<R: Registry>(summary: &Summary, method: ResolveMethod,
                            registry: &mut R) -> CargoResult<Resolve> {
    log!(5, "resolve; summary={}", summary);

    let mut cx = Context {
        resolve: Resolve::new(summary.get_package_id().clone()),
        activations: HashMap::new(),
        visited: Rc::new(RefCell::new(HashSet::new())),
    };
    let _p = profile::start(format!("resolving: {}", summary));
    cx.activations.insert((summary.get_name().to_string(),
                           summary.get_source_id().clone()),
                          vec![Rc::new(summary.clone())]);
    match try!(activate(cx, registry, summary, method)) {
        Ok(cx) => Ok(cx.resolve),
        Err(e) => Err(e),
    }
}

fn activate<R: Registry>(mut cx: Context,
                         registry: &mut R,
                         parent: &Summary,
                         method: ResolveMethod)
                         -> CargoResult<CargoResult<Context>> {
    // Extracting the platform request.
    let platform = match method {
        ResolveRequired(_, _, _, platform) => platform,
        ResolveEverything => None,
    };

    // First, figure out our set of dependencies based on the requsted set of
    // features. This also calculates what features we're going to enable for
    // our own dependencies.
    let deps = try!(resolve_features(&mut cx, parent, method));

    // Next, transform all dependencies into a list of possible candidates which
    // can satisfy that dependency.
    let mut deps = try!(deps.into_iter().map(|(_dep_name, (dep, features))| {
        let mut candidates = try!(registry.query(dep));
        // When we attempt versions for a package, we'll want to start at the
        // maximum version and work our way down.
        candidates.as_mut_slice().sort_by(|a, b| {
            b.get_version().cmp(a.get_version())
        });
        let candidates = candidates.into_iter().map(Rc::new).collect::<Vec<_>>();
        Ok((dep, candidates, features))
    }).collect::<CargoResult<Vec<_>>>());

    // When we recurse, attempt to resolve dependencies with fewer candidates
    // before recursing on dependencies with more candidates. This way if the
    // dependency with only one candidate can't be resolved we don't have to do
    // a bunch of work before we figure that out.
    deps.as_mut_slice().sort_by(|&(_, ref a, _), &(_, ref b, _)| {
        a.len().cmp(&b.len())
    });

    activate_deps(cx, registry, parent, platform, deps.as_slice(), 0)
}

fn activate_deps<'a, R: Registry>(cx: Context,
                                  registry: &mut R,
                                  parent: &Summary,
                                  platform: Option<&'a str>,
                                  deps: &'a [(&Dependency, Vec<Rc<Summary>>, Vec<String>)],
                                  cur: uint) -> CargoResult<CargoResult<Context>> {
    if cur == deps.len() { return Ok(Ok(cx)) }
    let (dep, ref candidates, ref features) = deps[cur];
    let method = ResolveRequired(false, features.as_slice(),
                                  dep.uses_default_features(), platform);

    let key = (dep.get_name().to_string(), dep.get_source_id().clone());
    let prev_active = cx.activations.find(&key)
                                    .map(|v| v.as_slice()).unwrap_or(&[]);
    log!(5, "{}[{}]>{} {} candidates", parent.get_name(), cur, dep.get_name(),
         candidates.len());
    log!(5, "{}[{}]>{} {} prev activations", parent.get_name(), cur,
         dep.get_name(), prev_active.len());

    // Filter the set of candidates based on the previously activated
    // versions for this dependency. We can actually use a version if it
    // precisely matches an activated version or if it is otherwise
    // incompatible with all other activated versions. Note that we define
    // "compatible" here in terms of the semver sense where if the left-most
    // nonzero digit is the same they're considered compatible.
    let mut my_candidates = candidates.iter().filter(|&b| {
        prev_active.iter().any(|a| a == b) ||
            prev_active.iter().all(|a| {
                !compatible(a.get_version(), b.get_version())
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
        log!(5, "{}[{}]>{} trying {}", parent.get_name(), cur, dep.get_name(),
             candidate.get_version());
        let mut my_cx = cx.clone();
        let early_return = {
            my_cx.resolve.graph.link(parent.get_package_id().clone(),
                                     candidate.get_package_id().clone());
            let prev = match my_cx.activations.entry(key.clone()) {
                Occupied(e) => e.into_mut(),
                Vacant(e) => e.set(Vec::new()),
            };
            if prev.iter().any(|c| c == candidate) {
                match cx.resolve.features(candidate.get_package_id()) {
                    Some(prev_features) => {
                        features.iter().all(|f| prev_features.contains(f))
                    }
                    None => features.len() == 0,
                }
            } else {
                my_cx.resolve.graph.add(candidate.get_package_id().clone(), []);
                prev.push(candidate.clone());
                false
            }
        };

        let my_cx = if early_return {
            my_cx
        } else {
            // Dependency graphs are required to be a DAG. Non-transitive
            // dependencies (dev-deps), however, can never introduce a cycle, so we
            // skip them.
            if dep.is_transitive() &&
               !cx.visited.borrow_mut().insert(candidate.get_package_id().clone()) {
                return Err(human(format!("cyclic package dependency: package `{}` \
                                          depends on itself",
                                         candidate.get_package_id())))
            }
            let my_cx = try!(activate(my_cx, registry, &**candidate, method));
            if dep.is_transitive() {
                cx.visited.borrow_mut().remove(candidate.get_package_id());
            }
            match my_cx {
                Ok(cx) => cx,
                Err(e) => { last_err = Some(e); continue }
            }
        };
        match try!(activate_deps(my_cx, registry, parent, platform, deps, cur + 1)) {
            Ok(cx) => return Ok(Ok(cx)),
            Err(e) => { last_err = Some(e); }
        }
    }
    log!(5, "{}[{}]>{} -- {}", parent.get_name(), cur, dep.get_name(), last_err);

    // Oh well, we couldn't activate any of the candidates, so we just can't
    // activate this dependency at all
    Ok(match last_err {
        Some(e) => Err(e),
        None if candidates.len() > 0 => {
            let mut msg = format!("failed to select a version for `{}` \
                                   (required by `{}`):\n\
                                   all possible versions conflict with \
                                   previously selected versions of `{}`",
                                  dep.get_name(), parent.get_name(),
                                  dep.get_name());
            'outer: for v in prev_active.iter() {
                for node in cx.resolve.graph.iter() {
                    let mut edges = match cx.resolve.graph.edges(node) {
                        Some(edges) => edges,
                        None => continue,
                    };
                    for edge in edges {
                        if edge != v.get_package_id() { continue }

                        msg.push_str(format!("\n  version {} in use by {}",
                                             v.get_version(), edge).as_slice());
                        continue 'outer;
                    }
                }
                msg.push_str(format!("\n  version {} in use by ??",
                                     v.get_version()).as_slice());
            }

            msg.push_str(format!("\n  possible versions to select: {}",
                                 candidates.iter().map(|v| v.get_version())
                                           .collect::<Vec<_>>()).as_slice());

            Err(human(msg))
        }
        None => {
            Err(human(format!("no package named `{}` found (required by `{}`)\n\
                               location searched: {}\n\
                               version required: {}",
                              dep.get_name(), parent.get_name(),
                              dep.get_source_id(),
                              dep.get_version_req())))
        }
    })
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
                        method: ResolveMethod)
                        -> CargoResult<HashMap<&'a str,
                                               (&'a Dependency, Vec<String>)>> {
    let dev_deps = match method {
        ResolveEverything => true,
        ResolveRequired(dev_deps, _, _, _) => dev_deps,
    };

    // First, filter by dev-dependencies
    let deps = parent.get_dependencies();
    let deps = deps.iter().filter(|d| d.is_transitive() || dev_deps);

    // Second, ignoring dependencies that should not be compiled for this platform
    let mut deps = deps.filter(|d| {
        match method {
            ResolveRequired(_, _, _, Some(ref platform)) => {
                d.is_active_for_platform(platform.as_slice())
            },
            _ => true
        }
    });

    let (mut feature_deps, used_features) = try!(build_features(parent, method));
    let mut ret = HashMap::new();

    // Next, sanitize all requested features by whitelisting all the requested
    // features that correspond to optional dependencies
    for dep in deps {
        // weed out optional dependencies, but not those required
        if dep.is_optional() && !feature_deps.contains_key_equiv(dep.get_name()) {
            continue
        }
        let mut base = feature_deps.pop_equiv(dep.get_name())
                                   .unwrap_or(Vec::new());
        for feature in dep.get_features().iter() {
            base.push(feature.clone());
            if feature.as_slice().contains("/") {
                return Err(human(format!("features in dependencies \
                                          cannot enable features in \
                                          other dependencies: `{}`",
                                         feature)));
            }
        }
        ret.insert(dep.get_name(), (dep, base));
    }

    // All features can only point to optional dependencies, in which case they
    // should have all been weeded out by the above iteration. Any remaining
    // features are bugs in that the package does not actually have those
    // features.
    if feature_deps.len() > 0 {
        let unknown = feature_deps.keys().map(|s| s.as_slice())
                                  .collect::<Vec<&str>>();
        if unknown.len() > 0 {
            let features = unknown.connect(", ");
            return Err(human(format!("Package `{}` does not have these features: \
                                      `{}`", parent.get_package_id(), features)))
        }
    }

    // Record what list of features is active for this package.
    if used_features.len() > 0 {
        let pkgid = parent.get_package_id().clone();
        match cx.resolve.features.entry(pkgid) {
            Occupied(entry) => entry.into_mut(),
            Vacant(entry) => entry.set(HashSet::new()),
        }.extend(used_features.into_iter());
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
fn build_features(s: &Summary, method: ResolveMethod)
                  -> CargoResult<(HashMap<String, Vec<String>>, HashSet<String>)> {
    let mut deps = HashMap::new();
    let mut used = HashSet::new();
    let mut visited = HashSet::new();
    match method {
        ResolveEverything => {
            for key in s.get_features().keys() {
                try!(add_feature(s, key.as_slice(), &mut deps, &mut used,
                                 &mut visited));
            }
            for dep in s.get_dependencies().iter().filter(|d| d.is_optional()) {
                try!(add_feature(s, dep.get_name(), &mut deps, &mut used,
                                 &mut visited));
            }
        }
        ResolveRequired(_, requested_features, _, _) =>  {
            for feat in requested_features.iter() {
                try!(add_feature(s, feat.as_slice(), &mut deps, &mut used,
                                 &mut visited));
            }
        }
    }
    match method {
        ResolveEverything | ResolveRequired(_, _, true, _) => {
            if s.get_features().find_equiv("default").is_some() &&
               !visited.contains_equiv("default") {
                try!(add_feature(s, "default", &mut deps, &mut used,
                                 &mut visited));
            }
        }
        _ => {}
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
        let mut parts = feat.splitn(1, '/');
        let feat_or_package = parts.next().unwrap();
        match parts.next() {
            Some(feat) => {
                let package = feat_or_package;
                match deps.entry(package.to_string()) {
                    Occupied(e) => e.into_mut(),
                    Vacant(e) => e.set(Vec::new()),
                }.push(feat.to_string());
            }
            None => {
                let feat = feat_or_package;
                if !visited.insert(feat.to_string()) {
                    return Err(human(format!("Cyclic feature dependency: \
                                              feature `{}` depends on itself",
                                              feat)))
                }
                used.insert(feat.to_string());
                match s.get_features().find_equiv(feat) {
                    Some(recursive) => {
                        for f in recursive.iter() {
                            try!(add_feature(s, f.as_slice(), deps, used,
                                             visited));
                        }
                    }
                    None => {
                        match deps.entry(feat.to_string()) {
                            Occupied(..) => {} // already activated
                            Vacant(e) => { e.set(Vec::new()); }
                        }
                    }
                }
                visited.remove(&feat.to_string());
            }
        }
        Ok(())
    }
}
