use std::collections::HashMap;
use std::fmt;
use util::graph::{Nodes,Edges};

use core::{
    Dependency,
    PackageId,
    Registry,
    SourceId,
};

use semver;

use util::{CargoResult, Graph, human, internal};

pub struct Resolve {
    graph: Graph<PackageId>
}

impl Resolve {
    fn new() -> Resolve {
        Resolve { graph: Graph::new() }
    }

    pub fn iter(&self) -> Nodes<PackageId> {
        self.graph.iter()
    }

    pub fn deps(&self, pkg: &PackageId) -> Option<Edges<PackageId>> {
        self.graph.edges(pkg)
    }
}

impl fmt::Show for Resolve {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        self.graph.fmt(fmt)
    }
}

struct Context<'a, R> {
    registry: &'a mut R,
    resolve: Resolve,

    // Eventually, we will have smarter logic for checking for conflicts in the resolve,
    // but without the registry, conflicts should not exist in practice, so this is just
    // a sanity check.
    seen: HashMap<(String, SourceId), semver::Version>
}

impl<'a, R: Registry> Context<'a, R> {
    fn new(registry: &'a mut R) -> Context<'a, R> {
        Context {
            registry: registry,
            resolve: Resolve::new(),
            seen: HashMap::new()
        }
    }
}

pub fn resolve<R: Registry>(root: &PackageId, deps: &[Dependency], registry: &mut R)
                            -> CargoResult<Resolve> {
    log!(5, "resolve; deps={}", deps);

    let mut context = Context::new(registry);
    try!(resolve_deps(root, deps, &mut context));
    log!(5, "  result={}", context.resolve);
    Ok(context.resolve)
}

fn resolve_deps<'a, R: Registry>(parent: &PackageId,
                                 deps: &[Dependency],
                                 ctx: &mut Context<'a, R>) -> CargoResult<()> {
    if deps.is_empty() {
        return Ok(());
    }

    for dep in deps.iter() {
        let pkgs = try!(ctx.registry.query(dep));

        if pkgs.is_empty() {
            return Err(human(format!("No package named `{:s}` found (required by `{:s}`).\n\
                Location searched: {}\n\
                Version required: {}",
                dep.get_name(),
                parent.get_name(),
                dep.get_namespace(),
                dep.get_version_req())));
        }

        if pkgs.len() > 1 {
            return Err(internal(format!("At the moment, Cargo only supports a \
                single source for a particular package name ({}).", dep)));
        }

        let summary = pkgs[0].clone();
        let name = summary.get_name().to_string();
        let source_id = summary.get_source_id().clone();
        let version = summary.get_version().clone();

        ctx.resolve.graph.link(parent.clone(), summary.get_package_id().clone());

        let found = {
            let found = ctx.seen.find(&(name.clone(), source_id.clone()));

            if found.is_some() {
                if found == Some(&version) { continue; }
                return Err(human(format!("Cargo found multiple copies of {} in {}. This \
                                        is not currently supported",
                                        summary.get_name(), summary.get_source_id())));
            } else {
                false
            }
        };

        if !found {
            ctx.seen.insert((name, source_id), version);
        }

        ctx.resolve.graph.add(summary.get_package_id().clone(), []);

        let deps: Vec<Dependency> = summary.get_dependencies().iter()
            .filter(|d| d.is_transitive())
            .map(|d| d.clone())
            .collect();

        try!(resolve_deps(summary.get_package_id(), deps.as_slice(), ctx));
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use hamcrest::{assert_that, equal_to, contains};

    use core::source::{SourceId, RegistryKind, GitKind, Location, Remote};
    use core::{Dependency, PackageId, Summary, Registry};
    use util::{CargoResult, ToUrl};

    fn resolve<R: Registry>(pkg: &PackageId, deps: &[Dependency], registry: &mut R)
                            -> CargoResult<Vec<PackageId>> {
        Ok(try!(super::resolve(pkg, deps, registry)).iter().map(|p| p.clone()).collect())
    }

    trait ToDep {
        fn to_dep(self) -> Dependency;
    }

    impl ToDep for &'static str {
        fn to_dep(self) -> Dependency {
            let url = "http://example.com".to_url().unwrap();
            let source_id = SourceId::new(RegistryKind, Remote(url));
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

            Summary::new(&PackageId::new($name, "1.0.0", &registry_loc()).unwrap(),
                         d.as_slice())
        });

        ($name:expr) => (
            Summary::new(&PackageId::new($name, "1.0.0", &registry_loc()).unwrap(),
                         [])
        )
    )

    fn registry_loc() -> SourceId {
        let remote = Location::parse("http://example.com").unwrap();
        SourceId::new(RegistryKind, remote)
    }

    fn pkg(name: &str) -> Summary {
        Summary::new(&pkg_id(name), &[])
    }

    fn pkg_id(name: &str) -> PackageId {
        PackageId::new(name, "1.0.0", &registry_loc()).unwrap()
    }

    fn pkg_id_loc(name: &str, loc: &str) -> PackageId {
        let remote = Location::parse(loc);
        let source_id = SourceId::new(GitKind("master".to_string()),
                                      remote.unwrap());

        PackageId::new(name, "1.0.0", &source_id).unwrap()
    }

    fn pkg_loc(name: &str, loc: &str) -> Summary {
        Summary::new(&pkg_id_loc(name, loc), &[])
    }

    fn dep(name: &str) -> Dependency {
        let url = "http://example.com".to_url().unwrap();
        let source_id = SourceId::new(RegistryKind, Remote(url));
        Dependency::parse(name, Some("1.0.0"), &source_id).unwrap()
    }

    fn dep_loc(name: &str, location: &str) -> Dependency {
        let url = location.to_url().unwrap();
        let source_id = SourceId::new(GitKind("master".to_string()), Remote(url));
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
        let res = resolve(&pkg_id("root"), [], &mut registry(vec!())).unwrap();

        assert_that(&res, equal_to(&names([])));
    }

    #[test]
    pub fn test_resolving_only_package() {
        let mut reg = registry(vec!(pkg("foo")));
        let res = resolve(&pkg_id("root"), [dep("foo")], &mut reg);

        assert_that(&res.unwrap(), contains(names(["root", "foo"])).exactly());
    }

    #[test]
    pub fn test_resolving_one_dep() {
        let mut reg = registry(vec!(pkg("foo"), pkg("bar")));
        let res = resolve(&pkg_id("root"), [dep("foo")], &mut reg);

        assert_that(&res.unwrap(), contains(names(["root", "foo"])).exactly());
    }

    #[test]
    pub fn test_resolving_multiple_deps() {
        let mut reg = registry(vec!(pkg!("foo"), pkg!("bar"), pkg!("baz")));
        let res = resolve(&pkg_id("root"), [dep("foo"), dep("baz")], &mut reg).unwrap();

        assert_that(&res, contains(names(["root", "foo", "baz"])).exactly());
    }

    #[test]
    pub fn test_resolving_transitive_deps() {
        let mut reg = registry(vec!(pkg!("foo"), pkg!("bar" => "foo")));
        let res = resolve(&pkg_id("root"), [dep("bar")], &mut reg).unwrap();

        assert_that(&res, contains(names(["root", "foo", "bar"])));
    }

    #[test]
    pub fn test_resolving_common_transitive_deps() {
        let mut reg = registry(vec!(pkg!("foo" => "bar"), pkg!("bar")));
        let res = resolve(&pkg_id("root"), [dep("foo"), dep("bar")], &mut reg).unwrap();

        assert_that(&res, contains(names(["root", "foo", "bar"])));
    }

    #[test]
    pub fn test_resolving_with_same_name() {
        let list = vec!(pkg_loc("foo", "http://first.example.com"),
                        pkg_loc("foo", "http://second.example.com"));

        let mut reg = registry(list);
        let res = resolve(&pkg_id("root"),
                          [dep_loc("foo", "http://first.example.com"),
                           dep_loc("foo", "http://second.example.com")],
                           &mut reg);

        let mut names = loc_names([("foo", "http://first.example.com"),
                              ("foo", "http://second.example.com")]);

        names.push(pkg_id("root"));

        assert_that(&res.unwrap(), contains(names).exactly());
    }

    #[test]
    pub fn test_resolving_with_dev_deps() {
        let mut reg = registry(vec!(
            pkg!("foo" => "bar", dep("baz").as_dev()),
            pkg!("baz" => "bat", dep("bam").as_dev()),
            pkg!("bar"),
            pkg!("bat")
        ));

        let res = resolve(&pkg_id("root"), [dep("foo"), dep("baz").as_dev()], &mut reg).unwrap();

        assert_that(&res, contains(names(["root", "foo", "bar", "baz"])));
    }
}
