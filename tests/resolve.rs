#![deny(warnings)]

extern crate hamcrest;
extern crate cargo;

use std::collections::HashMap;

use hamcrest::{assert_that, equal_to, contains, not};

use cargo::core::source::{SourceId, GitReference};
use cargo::core::dependency::Kind::{self, Development};
use cargo::core::{Dependency, PackageId, Summary, Registry};
use cargo::util::{CargoResult, ToUrl};
use cargo::core::resolver::{self, Method};

fn resolve(pkg: PackageId, deps: Vec<Dependency>, registry: &[Summary])
    -> CargoResult<Vec<PackageId>>
{
    struct MyRegistry<'a>(&'a [Summary]);
    impl<'a> Registry for MyRegistry<'a> {
        fn query(&mut self,
                 dep: &Dependency,
                 f: &mut FnMut(Summary)) -> CargoResult<()> {
            for summary in self.0.iter() {
                if dep.matches(summary) {
                    f(summary.clone());
                }
            }
            Ok(())
        }
    }
    let mut registry = MyRegistry(registry);
    let summary = Summary::new(pkg.clone(), deps, HashMap::new()).unwrap();
    let method = Method::Everything;
    let resolve = resolver::resolve(&[(summary, method)], &[], &mut registry, None)?;
    let res = resolve.iter().cloned().collect();
    Ok(res)
}

trait ToDep {
    fn to_dep(self) -> Dependency;
}

impl ToDep for &'static str {
    fn to_dep(self) -> Dependency {
        let url = "http://example.com".to_url().unwrap();
        let source_id = SourceId::for_registry(&url).unwrap();
        Dependency::parse_no_deprecated(self, Some("1.0.0"), &source_id).unwrap()
    }
}

impl ToDep for Dependency {
    fn to_dep(self) -> Dependency {
        self
    }
}

trait ToPkgId {
    fn to_pkgid(&self) -> PackageId;
}

impl ToPkgId for &'static str {
    fn to_pkgid(&self) -> PackageId {
        PackageId::new(*self, "1.0.0", &registry_loc()).unwrap()
    }
}

impl ToPkgId for (&'static str, &'static str) {
    fn to_pkgid(&self) -> PackageId {
        let (name, vers) = *self;
        PackageId::new(name, vers, &registry_loc()).unwrap()
    }
}

macro_rules! pkg {
    ($pkgid:expr => [$($deps:expr),+]) => ({
        let d: Vec<Dependency> = vec![$($deps.to_dep()),+];

        Summary::new($pkgid.to_pkgid(), d, HashMap::new()).unwrap()
    });

    ($pkgid:expr) => (
        Summary::new($pkgid.to_pkgid(), Vec::new(), HashMap::new()).unwrap()
    )
}

fn registry_loc() -> SourceId {
    let remote = "http://example.com".to_url().unwrap();
    SourceId::for_registry(&remote).unwrap()
}

fn pkg(name: &str) -> Summary {
    Summary::new(pkg_id(name), Vec::new(), HashMap::new()).unwrap()
}

fn pkg_id(name: &str) -> PackageId {
    PackageId::new(name, "1.0.0", &registry_loc()).unwrap()
}

fn pkg_id_loc(name: &str, loc: &str) -> PackageId {
    let remote = loc.to_url();
    let master = GitReference::Branch("master".to_string());
    let source_id = SourceId::for_git(&remote.unwrap(), master).unwrap();

    PackageId::new(name, "1.0.0", &source_id).unwrap()
}

fn pkg_loc(name: &str, loc: &str) -> Summary {
    Summary::new(pkg_id_loc(name, loc), Vec::new(), HashMap::new()).unwrap()
}

fn dep(name: &str) -> Dependency { dep_req(name, "1.0.0") }
fn dep_req(name: &str, req: &str) -> Dependency {
    let url = "http://example.com".to_url().unwrap();
    let source_id = SourceId::for_registry(&url).unwrap();
    Dependency::parse_no_deprecated(name, Some(req), &source_id).unwrap()
}

fn dep_loc(name: &str, location: &str) -> Dependency {
    let url = location.to_url().unwrap();
    let master = GitReference::Branch("master".to_string());
    let source_id = SourceId::for_git(&url, master).unwrap();
    Dependency::parse_no_deprecated(name, Some("1.0.0"), &source_id).unwrap()
}
fn dep_kind(name: &str, kind: Kind) -> Dependency {
    dep(name).set_kind(kind).clone()
}

fn registry(pkgs: Vec<Summary>) -> Vec<Summary> {
    pkgs
}

fn names<P: ToPkgId>(names: &[P]) -> Vec<PackageId> {
    names.iter().map(|name| name.to_pkgid()).collect()
}

fn loc_names(names: &[(&'static str, &'static str)]) -> Vec<PackageId> {
    names.iter()
        .map(|&(name, loc)| pkg_id_loc(name, loc)).collect()
}

#[test]
fn test_resolving_empty_dependency_list() {
    let res = resolve(pkg_id("root"), Vec::new(),
                      &mut registry(vec![])).unwrap();

    assert_that(&res, equal_to(&names(&["root"])));
}

#[test]
fn test_resolving_only_package() {
    let reg = registry(vec![pkg("foo")]);
    let res = resolve(pkg_id("root"), vec![dep("foo")], &reg);

    assert_that(&res.unwrap(), contains(names(&["root", "foo"])).exactly());
}

#[test]
fn test_resolving_one_dep() {
    let reg = registry(vec![pkg("foo"), pkg("bar")]);
    let res = resolve(pkg_id("root"), vec![dep("foo")], &reg);

    assert_that(&res.unwrap(), contains(names(&["root", "foo"])).exactly());
}

#[test]
fn test_resolving_multiple_deps() {
    let reg = registry(vec![pkg!("foo"), pkg!("bar"), pkg!("baz")]);
    let res = resolve(pkg_id("root"), vec![dep("foo"), dep("baz")],
                      &reg).unwrap();

    assert_that(&res, contains(names(&["root", "foo", "baz"])).exactly());
}

#[test]
fn test_resolving_transitive_deps() {
    let reg = registry(vec![pkg!("foo"), pkg!("bar" => ["foo"])]);
    let res = resolve(pkg_id("root"), vec![dep("bar")], &reg).unwrap();

    assert_that(&res, contains(names(&["root", "foo", "bar"])));
}

#[test]
fn test_resolving_common_transitive_deps() {
    let reg = registry(vec![pkg!("foo" => ["bar"]), pkg!("bar")]);
    let res = resolve(pkg_id("root"), vec![dep("foo"), dep("bar")],
                      &reg).unwrap();

    assert_that(&res, contains(names(&["root", "foo", "bar"])));
}

#[test]
fn test_resolving_with_same_name() {
    let list = vec![pkg_loc("foo", "http://first.example.com"),
                    pkg_loc("bar", "http://second.example.com")];

    let reg = registry(list);
    let res = resolve(pkg_id("root"),
                      vec![dep_loc("foo", "http://first.example.com"),
                           dep_loc("bar", "http://second.example.com")],
                      &reg);

    let mut names = loc_names(&[("foo", "http://first.example.com"),
                                ("bar", "http://second.example.com")]);

    names.push(pkg_id("root"));

    assert_that(&res.unwrap(), contains(names).exactly());
}

#[test]
fn test_resolving_with_dev_deps() {
    let reg = registry(vec![
        pkg!("foo" => ["bar", dep_kind("baz", Development)]),
        pkg!("baz" => ["bat", dep_kind("bam", Development)]),
        pkg!("bar"),
        pkg!("bat")
    ]);

    let res = resolve(pkg_id("root"),
                      vec![dep("foo"), dep_kind("baz", Development)],
                      &reg).unwrap();

    assert_that(&res, contains(names(&["root", "foo", "bar", "baz"])));
}

#[test]
fn resolving_with_many_versions() {
    let reg = registry(vec![
        pkg!(("foo", "1.0.1")),
        pkg!(("foo", "1.0.2")),
    ]);

    let res = resolve(pkg_id("root"), vec![dep("foo")], &reg).unwrap();

    assert_that(&res, contains(names(&[("root", "1.0.0"),
                                       ("foo", "1.0.2")])));
}

#[test]
fn resolving_with_specific_version() {
    let reg = registry(vec![
        pkg!(("foo", "1.0.1")),
        pkg!(("foo", "1.0.2")),
    ]);

    let res = resolve(pkg_id("root"), vec![dep_req("foo", "=1.0.1")],
                      &reg).unwrap();

    assert_that(&res, contains(names(&[("root", "1.0.0"),
                                       ("foo", "1.0.1")])));
}

#[test]
fn test_resolving_maximum_version_with_transitive_deps() {
    let reg = registry(vec![
        pkg!(("util", "1.2.2")),
        pkg!(("util", "1.0.0")),
        pkg!(("util", "1.1.1")),
        pkg!("foo" => [dep_req("util", "1.0.0")]),
        pkg!("bar" => [dep_req("util", ">=1.0.1")]),
    ]);

    let res = resolve(pkg_id("root"), vec![dep_req("foo", "1.0.0"), dep_req("bar", "1.0.0")],
                      &reg).unwrap();

    assert_that(&res, contains(names(&[("root", "1.0.0"),
                                       ("foo", "1.0.0"),
                                       ("bar", "1.0.0"),
                                       ("util", "1.2.2")])));
    assert_that(&res, not(contains(names(&[("util", "1.0.1")]))));
    assert_that(&res, not(contains(names(&[("util", "1.1.1")]))));
}

#[test]
fn resolving_incompat_versions() {
    let reg = registry(vec![
        pkg!(("foo", "1.0.1")),
        pkg!(("foo", "1.0.2")),
        pkg!("bar" => [dep_req("foo", "=1.0.2")]),
    ]);

    assert!(resolve(pkg_id("root"), vec![
        dep_req("foo", "=1.0.1"),
        dep("bar"),
    ], &reg).is_err());
}

#[test]
fn resolving_backtrack() {
    let reg = registry(vec![
        pkg!(("foo", "1.0.2") => [dep("bar")]),
        pkg!(("foo", "1.0.1") => [dep("baz")]),
        pkg!("bar" => [dep_req("foo", "=2.0.2")]),
        pkg!("baz"),
    ]);

    let res = resolve(pkg_id("root"), vec![
        dep_req("foo", "^1"),
    ], &reg).unwrap();

    assert_that(&res, contains(names(&[("root", "1.0.0"),
                                       ("foo", "1.0.1"),
                                       ("baz", "1.0.0")])));
}

#[test]
fn resolving_allows_multiple_compatible_versions() {
    let reg = registry(vec![
        pkg!(("foo", "1.0.0")),
        pkg!(("foo", "2.0.0")),
        pkg!(("foo", "0.1.0")),
        pkg!(("foo", "0.2.0")),

        pkg!("bar" => ["d1", "d2", "d3", "d4"]),
        pkg!("d1" => [dep_req("foo", "1")]),
        pkg!("d2" => [dep_req("foo", "2")]),
        pkg!("d3" => [dep_req("foo", "0.1")]),
        pkg!("d4" => [dep_req("foo", "0.2")]),
    ]);

    let res = resolve(pkg_id("root"), vec![
        dep("bar"),
    ], &reg).unwrap();

    assert_that(&res, contains(names(&[("root", "1.0.0"),
                                       ("foo", "1.0.0"),
                                       ("foo", "2.0.0"),
                                       ("foo", "0.1.0"),
                                       ("foo", "0.2.0"),
                                       ("d1", "1.0.0"),
                                       ("d2", "1.0.0"),
                                       ("d3", "1.0.0"),
                                       ("d4", "1.0.0"),
                                       ("bar", "1.0.0")])));
}

#[test]
fn resolving_with_deep_backtracking() {
    let reg = registry(vec![
        pkg!(("foo", "1.0.1") => [dep_req("bar", "1")]),
        pkg!(("foo", "1.0.0") => [dep_req("bar", "2")]),

        pkg!(("bar", "1.0.0") => [dep_req("baz", "=1.0.2"),
                                  dep_req("other", "1")]),
        pkg!(("bar", "2.0.0") => [dep_req("baz", "=1.0.1")]),

        pkg!(("baz", "1.0.2") => [dep_req("other", "2")]),
        pkg!(("baz", "1.0.1")),

        pkg!(("dep_req", "1.0.0")),
        pkg!(("dep_req", "2.0.0")),
    ]);

    let res = resolve(pkg_id("root"), vec![
        dep_req("foo", "1"),
    ], &reg).unwrap();

    assert_that(&res, contains(names(&[("root", "1.0.0"),
                                       ("foo", "1.0.0"),
                                       ("bar", "2.0.0"),
                                       ("baz", "1.0.1")])));
}

#[test]
fn resolving_but_no_exists() {
    let reg = registry(vec![
    ]);

    let res = resolve(pkg_id("root"), vec![
        dep_req("foo", "1"),
    ], &reg);
    assert!(res.is_err());

    assert_eq!(res.err().unwrap().to_string(), "\
no matching package named `foo` found (required by `root`)
location searched: registry http://example.com/
version required: ^1\
");
}

#[test]
fn resolving_cycle() {
    let reg = registry(vec![
        pkg!("foo" => ["foo"]),
    ]);

    let _ = resolve(pkg_id("root"), vec![
        dep_req("foo", "1"),
    ], &reg);
}

#[test]
fn hard_equality() {
    let reg = registry(vec![
        pkg!(("foo", "1.0.1")),
        pkg!(("foo", "1.0.0")),

        pkg!(("bar", "1.0.0") => [dep_req("foo", "1.0.0")]),
    ]);

    let res = resolve(pkg_id("root"), vec![
        dep_req("bar", "1"),
        dep_req("foo", "=1.0.0"),
    ], &reg).unwrap();

    assert_that(&res, contains(names(&[("root", "1.0.0"),
                                       ("foo", "1.0.0"),
                                       ("bar", "1.0.0")])));
}
