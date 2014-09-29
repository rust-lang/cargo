use std::collections::{HashMap, HashSet};
use std::collections::hashmap::{Occupied, Vacant};
use std::fmt;
use semver;

use serialize::{Encodable, Encoder, Decodable, Decoder};

use core::{PackageId, Registry, SourceId, Summary, Dependency};
use core::PackageIdSpec;
use util::{CargoResult, Graph, human, internal, ChainError};
use util::profile;
use util::graph::{Nodes, Edges};

#[deriving(PartialEq, Eq)]
pub struct Resolve {
    graph: Graph<PackageId>,
    features: HashMap<PackageId, HashSet<String>>,
    root: PackageId
}

pub enum ResolveMethod<'a> {
    ResolveEverything,
    ResolveRequired(/* dev_deps = */ bool,
                    /* features = */ &'a [String],
                    /* uses_default_features = */ bool),
}

#[deriving(Encodable, Decodable, Show)]
pub struct EncodableResolve {
    package: Option<Vec<EncodableDependency>>,
    root: EncodableDependency
}

impl EncodableResolve {
    pub fn to_resolve(&self, default: &SourceId) -> CargoResult<Resolve> {
        let mut g = Graph::new();

        try!(add_pkg_to_graph(&mut g, &self.root, default));

        match self.package {
            Some(ref packages) => {
                for dep in packages.iter() {
                    try!(add_pkg_to_graph(&mut g, dep, default));
                }
            }
            None => {}
        }

        let root = self.root.to_package_id(default);
        Ok(Resolve { graph: g, root: try!(root), features: HashMap::new() })
    }
}

fn add_pkg_to_graph(g: &mut Graph<PackageId>,
                    dep: &EncodableDependency,
                    default: &SourceId)
                    -> CargoResult<()>
{
    let package_id = try!(dep.to_package_id(default));
    g.add(package_id.clone(), []);

    match dep.dependencies {
        Some(ref deps) => {
            for edge in deps.iter() {
                g.link(package_id.clone(), try!(edge.to_package_id(default)));
            }
        },
        _ => ()
    };

    Ok(())
}

#[deriving(Encodable, Decodable, Show, PartialOrd, Ord, PartialEq, Eq)]
pub struct EncodableDependency {
    name: String,
    version: String,
    source: Option<SourceId>,
    dependencies: Option<Vec<EncodablePackageId>>
}

impl EncodableDependency {
    fn to_package_id(&self, default_source: &SourceId) -> CargoResult<PackageId> {
        PackageId::new(
            self.name.as_slice(),
            self.version.as_slice(),
            self.source.as_ref().unwrap_or(default_source))
    }
}

#[deriving(Show, PartialOrd, Ord, PartialEq, Eq)]
pub struct EncodablePackageId {
    name: String,
    version: String,
    source: Option<SourceId>
}

impl<E, S: Encoder<E>> Encodable<S, E> for EncodablePackageId {
    fn encode(&self, s: &mut S) -> Result<(), E> {
        let mut out = format!("{} {}", self.name, self.version);
        self.source.as_ref().map(|s| {
            out.push_str(format!(" ({})", s.to_url()).as_slice())
        });
        out.encode(s)
    }
}

impl<E, D: Decoder<E>> Decodable<D, E> for EncodablePackageId {
    fn decode(d: &mut D) -> Result<EncodablePackageId, E> {
        let string: String = raw_try!(Decodable::decode(d));
        let regex = regex!(r"^([^ ]+) ([^ ]+)(?: \(([^\)]+)\))?$");
        let captures = regex.captures(string.as_slice())
                            .expect("invalid serialized PackageId");

        let name = captures.at(1);
        let version = captures.at(2);

        let source = captures.at(3);

        let source_id = if source == "" {
            None
        } else {
            Some(SourceId::from_url(source.to_string()))
        };

        Ok(EncodablePackageId {
            name: name.to_string(),
            version: version.to_string(),
            source: source_id
        })
    }
}

impl EncodablePackageId {
    fn to_package_id(&self, default_source: &SourceId) -> CargoResult<PackageId> {
        PackageId::new(
            self.name.as_slice(),
            self.version.as_slice(),
            self.source.as_ref().unwrap_or(default_source))
    }
}

impl<E, S: Encoder<E>> Encodable<S, E> for Resolve {
    fn encode(&self, s: &mut S) -> Result<(), E> {
        let mut ids: Vec<&PackageId> = self.graph.iter().collect();
        ids.sort();

        let encodable = ids.iter().filter_map(|&id| {
            if self.root == *id { return None; }

            Some(encodable_resolve_node(id, &self.root, &self.graph))
        }).collect::<Vec<EncodableDependency>>();

        EncodableResolve {
            package: Some(encodable),
            root: encodable_resolve_node(&self.root, &self.root, &self.graph)
        }.encode(s)
    }
}

fn encodable_resolve_node(id: &PackageId, root: &PackageId,
                          graph: &Graph<PackageId>) -> EncodableDependency {
    let deps = graph.edges(id).map(|edge| {
        let mut deps = edge.map(|e| {
            encodable_package_id(e, root)
        }).collect::<Vec<EncodablePackageId>>();
        deps.sort();
        deps
    });

    let source = if id.get_source_id() == root.get_source_id() {
        None
    } else {
        Some(id.get_source_id().clone())
    };

    EncodableDependency {
        name: id.get_name().to_string(),
        version: id.get_version().to_string(),
        source: source,
        dependencies: deps,
    }
}

fn encodable_package_id(id: &PackageId, root: &PackageId) -> EncodablePackageId {
    let source = if id.get_source_id() == root.get_source_id() {
        None
    } else {
        Some(id.get_source_id().clone())
    };
    EncodablePackageId {
        name: id.get_name().to_string(),
        version: id.get_version().to_string(),
        source: source,
    }
}

impl Resolve {
    fn new(root: PackageId) -> Resolve {
        let mut g = Graph::new();
        g.add(root.clone(), []);
        Resolve { graph: g, root: root, features: HashMap::new() }
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
        match ids.next() {
            Some(other) => {
                let mut msg = format!("Ambiguous package id specification: \
                                       `{}`\nMatching packages:\n  {}\n  {}",
                                      spec, ret, other);
                for id in ids {
                    msg = format!("{}\n  {}", msg, id);
                }
                Err(human(msg))
            }
            None => Ok(ret)
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

struct Context<'a, R:'a> {
    registry: &'a mut R,
    resolve: Resolve,
    // cycle detection
    visited: HashSet<PackageId>,

    // Try not to re-resolve too much
    resolved: HashMap<PackageId, HashSet<String>>,

    // Eventually, we will have smarter logic for checking for conflicts in the
    // resolve, but without the registry, conflicts should not exist in
    // practice, so this is just a sanity check.
    seen: HashMap<(String, SourceId), semver::Version>,
}

impl<'a, R: Registry> Context<'a, R> {
    fn new(registry: &'a mut R, root: PackageId) -> Context<'a, R> {
        Context {
            registry: registry,
            resolve: Resolve::new(root),
            seen: HashMap::new(),
            visited: HashSet::new(),
            resolved: HashMap::new(),
        }
    }
}

pub fn resolve<R: Registry>(summary: &Summary, method: ResolveMethod,
                            registry: &mut R) -> CargoResult<Resolve> {
    log!(5, "resolve; summary={}", summary);
    let _p = profile::start(format!("resolving: {}", summary));

    let mut context = Context::new(registry, summary.get_package_id().clone());
    try!(resolve_deps(summary, method, &mut context));
    log!(5, "  result={}", context.resolve);
    Ok(context.resolve)
}

fn resolve_deps<'a, R: Registry>(parent: &Summary,
                                 method: ResolveMethod,
                                 ctx: &mut Context<'a, R>)
                                 -> CargoResult<()> {
    // Dependency graphs are required to be a DAG
    if !ctx.visited.insert(parent.get_package_id().clone()) {
        return Err(human(format!("Cyclic package dependency: package `{}` \
                                  depends on itself", parent.get_package_id())))
    }

    let dev_deps = match method {
        ResolveEverything => true,
        ResolveRequired(dev_deps, _, _) => dev_deps,
    };

    // First, filter by dev-dependencies
    let deps = parent.get_dependencies();
    let deps = deps.iter().filter(|d| d.is_transitive() || dev_deps);

    // Second, weed out optional dependencies, but not those required
    let (mut feature_deps, used_features) = try!(build_features(parent, method));
    let deps = deps.filter(|d| {
        !d.is_optional() || feature_deps.remove(&d.get_name().to_string())
    }).collect::<Vec<&Dependency>>();

    // All features can only point to optional dependencies, in which case they
    // should have all been weeded out by the above iteration. Any remaining
    // features are bugs in that the package does not actually have those
    // features.
    if feature_deps.len() > 0 {
        let features = feature_deps.iter().map(|s| s.as_slice())
                                   .collect::<Vec<&str>>().connect(", ");
        return Err(human(format!("Package `{}` does not have these features: \
                                  `{}`", parent.get_package_id(), features)))
    }

    // Record what list of features is active for this package.
    {
        let pkgid = parent.get_package_id().clone();
        match ctx.resolve.features.entry(pkgid) {
            Occupied(entry) => entry.into_mut(),
            Vacant(entry) => entry.set(HashSet::new()),
        }.extend(used_features.into_iter());
    }

    // Recursively resolve all dependencies
    for &dep in deps.iter() {
        if !match ctx.resolved.entry(parent.get_package_id().clone()) {
            Occupied(entry) => entry.into_mut(),
            Vacant(entry) => entry.set(HashSet::new()),
        }.insert(dep.get_name().to_string()) {
            continue
        }

        let pkgs = try!(ctx.registry.query(dep));
        if pkgs.is_empty() {
            return Err(human(format!("No package named `{}` found \
                                      (required by `{}`).\n\
                                      Location searched: {}\n\
                                      Version required: {}",
                                     dep.get_name(),
                                     parent.get_package_id().get_name(),
                                     dep.get_source_id(),
                                     dep.get_version_req())));
        } else if pkgs.len() > 1 {
            return Err(internal(format!("At the moment, Cargo only supports a \
                single source for a particular package name ({}).", dep)));
        }

        let summary = &pkgs[0];
        let name = summary.get_name().to_string();
        let source_id = summary.get_source_id().clone();
        let version = summary.get_version();

        ctx.resolve.graph.link(parent.get_package_id().clone(),
                               summary.get_package_id().clone());

        let found = match ctx.seen.find(&(name.clone(), source_id.clone())) {
            Some(v) if v == version => true,
            Some(..) => {
                return Err(human(format!("Cargo found multiple copies of {} in \
                                          {}. This is not currently supported",
                                         summary.get_name(),
                                         summary.get_source_id())));
            }
            None => false,
        };
        if !found {
            ctx.seen.insert((name, source_id), version.clone());
            ctx.resolve.graph.add(summary.get_package_id().clone(), []);
        }
        try!(resolve_deps(summary,
                          ResolveRequired(false, dep.get_features(),
                                          dep.uses_default_features()),
                          ctx));
    }

    ctx.visited.remove(parent.get_package_id());
    Ok(())
}

// Returns a pair of (feature dependencies, all used features)
fn build_features(s: &Summary, method: ResolveMethod)
                  -> CargoResult<(HashSet<String>, HashSet<String>)> {
    let mut deps = HashSet::new();
    let mut used = HashSet::new();
    let mut visited = HashSet::new();
    match method {
        ResolveEverything => {
            for key in s.get_features().keys() {
                try!(add_feature(s, key.as_slice(), &mut deps, &mut used,
                                 &mut visited));
            }
        }
        ResolveRequired(_, requested_features, _) =>  {
            for feat in requested_features.iter() {
                try!(add_feature(s, feat.as_slice(), &mut deps, &mut used,
                                 &mut visited));
            }
        }
    }
    match method {
        ResolveEverything | ResolveRequired(_, _, true) => {
            if s.get_features().find_equiv(&"default").is_some() &&
               !visited.contains_equiv(&"default") {
                try!(add_feature(s, "default", &mut deps, &mut used,
                                 &mut visited));
            }
        }
        _ => {}
    }
    return Ok((deps, used));

    fn add_feature(s: &Summary, feat: &str,
                   deps: &mut HashSet<String>,
                   used: &mut HashSet<String>,
                   visited: &mut HashSet<String>) -> CargoResult<()> {
        if !visited.insert(feat.to_string()) {
            return Err(human(format!("Cyclic feature dependency: feature `{}` \
                                      depends on itself", feat)))
        }
        used.insert(feat.to_string());
        match s.get_features().find_equiv(&feat) {
            Some(recursive) => {
                for f in recursive.iter() {
                    try!(add_feature(s, f.as_slice(), deps, used, visited));
                }
            }
            None => { deps.insert(feat.to_string()); }
        }
        visited.remove(&feat.to_string());
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use hamcrest::{assert_that, equal_to, contains};

    use core::source::{SourceId, RegistryKind, GitKind};
    use core::{Dependency, PackageId, Summary, Registry};
    use util::{CargoResult, ToUrl};

    fn resolve<R: Registry>(pkg: PackageId, deps: Vec<Dependency>,
                            registry: &mut R)
                            -> CargoResult<Vec<PackageId>> {
        let summary = Summary::new(pkg, deps, HashMap::new()).unwrap();
        let method = super::ResolveEverything;
        Ok(try!(super::resolve(&summary, method,
                               registry)).iter().map(|p| p.clone()).collect())
    }

    trait ToDep {
        fn to_dep(self) -> Dependency;
    }

    impl ToDep for &'static str {
        fn to_dep(self) -> Dependency {
            let url = "http://example.com".to_url().unwrap();
            let source_id = SourceId::new(RegistryKind, url);
            Dependency::parse(self, Some("1.0.0"), &source_id).unwrap()
        }
    }

    impl ToDep for Dependency {
        fn to_dep(self) -> Dependency {
            self
        }
    }

    macro_rules! pkg(
        ($name:expr => $($deps:expr),+) => ({
            let d: Vec<Dependency> = vec!($($deps.to_dep()),+);

            Summary::new(PackageId::new($name, "1.0.0", &registry_loc()).unwrap(),
                         d, HashMap::new()).unwrap()
        });

        ($name:expr) => (
            Summary::new(PackageId::new($name, "1.0.0", &registry_loc()).unwrap(),
                         Vec::new(), HashMap::new()).unwrap()
        )
    )

    fn registry_loc() -> SourceId {
        let remote = "http://example.com".to_url().unwrap();
        SourceId::new(RegistryKind, remote)
    }

    fn pkg(name: &str) -> Summary {
        Summary::new(pkg_id(name), Vec::new(), HashMap::new()).unwrap()
    }

    fn pkg_id(name: &str) -> PackageId {
        PackageId::new(name, "1.0.0", &registry_loc()).unwrap()
    }

    fn pkg_id_loc(name: &str, loc: &str) -> PackageId {
        let remote = loc.to_url();
        let source_id = SourceId::new(GitKind("master".to_string()),
                                      remote.unwrap());

        PackageId::new(name, "1.0.0", &source_id).unwrap()
    }

    fn pkg_loc(name: &str, loc: &str) -> Summary {
        Summary::new(pkg_id_loc(name, loc), Vec::new(), HashMap::new()).unwrap()
    }

    fn dep(name: &str) -> Dependency {
        let url = "http://example.com".to_url().unwrap();
        let source_id = SourceId::new(RegistryKind, url);
        Dependency::parse(name, Some("1.0.0"), &source_id).unwrap()
    }

    fn dep_loc(name: &str, location: &str) -> Dependency {
        let url = location.to_url().unwrap();
        let source_id = SourceId::new(GitKind("master".to_string()), url);
        Dependency::parse(name, Some("1.0.0"), &source_id).unwrap()
    }

    fn registry(pkgs: Vec<Summary>) -> Vec<Summary> {
        pkgs
    }

    fn names(names: &[&'static str]) -> Vec<PackageId> {
        names.iter()
            .map(|name| PackageId::new(*name, "1.0.0", &registry_loc()).unwrap())
            .collect()
    }

    fn loc_names(names: &[(&'static str, &'static str)]) -> Vec<PackageId> {
        names.iter()
            .map(|&(name, loc)| pkg_id_loc(name, loc)).collect()
    }

    #[test]
    pub fn test_resolving_empty_dependency_list() {
        let res = resolve(pkg_id("root"), Vec::new(),
                          &mut registry(vec!())).unwrap();

        assert_that(&res, equal_to(&names(["root"])));
    }

    #[test]
    pub fn test_resolving_only_package() {
        let mut reg = registry(vec!(pkg("foo")));
        let res = resolve(pkg_id("root"), vec![dep("foo")], &mut reg);

        assert_that(&res.unwrap(), contains(names(["root", "foo"])).exactly());
    }

    #[test]
    pub fn test_resolving_one_dep() {
        let mut reg = registry(vec!(pkg("foo"), pkg("bar")));
        let res = resolve(pkg_id("root"), vec![dep("foo")], &mut reg);

        assert_that(&res.unwrap(), contains(names(["root", "foo"])).exactly());
    }

    #[test]
    pub fn test_resolving_multiple_deps() {
        let mut reg = registry(vec!(pkg!("foo"), pkg!("bar"), pkg!("baz")));
        let res = resolve(pkg_id("root"), vec![dep("foo"), dep("baz")],
                          &mut reg).unwrap();

        assert_that(&res, contains(names(["root", "foo", "baz"])).exactly());
    }

    #[test]
    pub fn test_resolving_transitive_deps() {
        let mut reg = registry(vec!(pkg!("foo"), pkg!("bar" => "foo")));
        let res = resolve(pkg_id("root"), vec![dep("bar")], &mut reg).unwrap();

        assert_that(&res, contains(names(["root", "foo", "bar"])));
    }

    #[test]
    pub fn test_resolving_common_transitive_deps() {
        let mut reg = registry(vec!(pkg!("foo" => "bar"), pkg!("bar")));
        let res = resolve(pkg_id("root"), vec![dep("foo"), dep("bar")],
                          &mut reg).unwrap();

        assert_that(&res, contains(names(["root", "foo", "bar"])));
    }

    #[test]
    pub fn test_resolving_with_same_name() {
        let list = vec![pkg_loc("foo", "http://first.example.com"),
                        pkg_loc("bar", "http://second.example.com")];

        let mut reg = registry(list);
        let res = resolve(pkg_id("root"),
                          vec![dep_loc("foo", "http://first.example.com"),
                               dep_loc("bar", "http://second.example.com")],
                          &mut reg);

        let mut names = loc_names([("foo", "http://first.example.com"),
                                   ("bar", "http://second.example.com")]);

        names.push(pkg_id("root"));

        assert_that(&res.unwrap(), contains(names).exactly());
    }

    #[test]
    pub fn test_resolving_with_dev_deps() {
        let mut reg = registry(vec!(
            pkg!("foo" => "bar", dep("baz").transitive(false)),
            pkg!("baz" => "bat", dep("bam").transitive(false)),
            pkg!("bar"),
            pkg!("bat")
        ));

        let res = resolve(pkg_id("root"),
                          vec![dep("foo"), dep("baz").transitive(false)],
                          &mut reg).unwrap();

        assert_that(&res, contains(names(["root", "foo", "bar", "baz"])));
    }
}
