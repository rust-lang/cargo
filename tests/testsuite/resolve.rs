use std::collections::{BTreeMap, HashSet};

use support::hamcrest::{assert_that, contains, is_not};

use cargo::core::source::{GitReference, SourceId};
use cargo::core::dependency::Kind::{self, Development};
use cargo::core::{Dependency, PackageId, Registry, Summary, enable_nightly_features};
use cargo::util::{CargoResult, Config, ToUrl};
use cargo::core::resolver::{self, Method};

use support::ChannelChanger;
use support::{execs, project};
use support::registry::Package;

fn resolve(
    pkg: &PackageId,
    deps: Vec<Dependency>,
    registry: &[Summary],
) -> CargoResult<Vec<PackageId>> {
    resolve_with_config(pkg, deps, registry, None)
}

fn resolve_with_config(
    pkg: &PackageId,
    deps: Vec<Dependency>,
    registry: &[Summary],
    config: Option<&Config>,
) -> CargoResult<Vec<PackageId>> {
    struct MyRegistry<'a>(&'a [Summary]);
    impl<'a> Registry for MyRegistry<'a> {
        fn query(&mut self, dep: &Dependency, f: &mut FnMut(Summary), fuzzy: bool) -> CargoResult<()> {
            for summary in self.0.iter() {
                if fuzzy || dep.matches(summary) {
                    f(summary.clone());
                }
            }
            Ok(())
        }
    }
    let mut registry = MyRegistry(registry);
    let summary = Summary::new(pkg.clone(), deps, &BTreeMap::<String, Vec<String>>::new(), None::<String>, false).unwrap();
    let method = Method::Everything;
    let resolve = resolver::resolve(
        &[(summary, method)],
        &[],
        &mut registry,
        &HashSet::new(),
        config,
        false,
    )?;
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

impl<'a> ToPkgId for &'a str {
    fn to_pkgid(&self) -> PackageId {
        PackageId::new(*self, "1.0.0", &registry_loc()).unwrap()
    }
}

impl<'a> ToPkgId for (&'a str, &'a str) {
    fn to_pkgid(&self) -> PackageId {
        let (name, vers) = *self;
        PackageId::new(name, vers, &registry_loc()).unwrap()
    }
}

impl<'a> ToPkgId for (&'a str, String) {
    fn to_pkgid(&self) -> PackageId {
        let (name, ref vers) = *self;
        PackageId::new(name, vers, &registry_loc()).unwrap()
    }
}

macro_rules! pkg {
    ($pkgid:expr => [$($deps:expr),+]) => ({
        let d: Vec<Dependency> = vec![$($deps.to_dep()),+];
        let pkgid = $pkgid.to_pkgid();
        let link = if pkgid.name().ends_with("-sys") {Some(pkgid.name().as_str())} else {None};

        Summary::new(pkgid, d, &BTreeMap::<String, Vec<String>>::new(), link, false).unwrap()
    });

    ($pkgid:expr) => ({
        let pkgid = $pkgid.to_pkgid();
        let link = if pkgid.name().ends_with("-sys") {Some(pkgid.name().as_str())} else {None};
        Summary::new(pkgid, Vec::new(), &BTreeMap::<String, Vec<String>>::new(), link, false).unwrap()
    })
}

fn registry_loc() -> SourceId {
    let remote = "http://example.com".to_url().unwrap();
    SourceId::for_registry(&remote).unwrap()
}

fn pkg(name: &str) -> Summary {
    let link = if name.ends_with("-sys") {
        Some(name)
    } else {
        None
    };
    Summary::new(pkg_id(name), Vec::new(), &BTreeMap::<String, Vec<String>>::new(), link, false).unwrap()
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
    let link = if name.ends_with("-sys") {
        Some(name)
    } else {
        None
    };
    Summary::new(
        pkg_id_loc(name, loc),
        Vec::new(),
        &BTreeMap::<String, Vec<String>>::new(),
        link,
        false,
    ).unwrap()
}

fn dep(name: &str) -> Dependency {
    dep_req(name, "1.0.0")
}
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
    names
        .iter()
        .map(|&(name, loc)| pkg_id_loc(name, loc))
        .collect()
}

#[test]
#[should_panic(expected = "assertion failed: !name.is_empty()")]
fn test_dependency_with_empty_name() {
    // Bug 5229, dependency-names must not be empty
    "".to_dep();
}

#[test]
fn test_resolving_empty_dependency_list() {
    let res = resolve(&pkg_id("root"), Vec::new(), &registry(vec![])).unwrap();

    assert_eq!(res, names(&["root"]));
}

fn assert_same(a: &[PackageId], b: &[PackageId]) {
    assert_eq!(a.len(), b.len());
    for item in a {
        assert!(b.contains(item));
    }
}

#[test]
fn test_resolving_only_package() {
    let reg = registry(vec![pkg("foo")]);
    let res = resolve(&pkg_id("root"), vec![dep("foo")], &reg).unwrap();
    assert_same(&res, &names(&["root", "foo"]));
}

#[test]
fn test_resolving_one_dep() {
    let reg = registry(vec![pkg("foo"), pkg("bar")]);
    let res = resolve(&pkg_id("root"), vec![dep("foo")], &reg).unwrap();
    assert_same(&res, &names(&["root", "foo"]));
}

#[test]
fn test_resolving_multiple_deps() {
    let reg = registry(vec![pkg!("foo"), pkg!("bar"), pkg!("baz")]);
    let res = resolve(&pkg_id("root"), vec![dep("foo"), dep("baz")], &reg).unwrap();
    assert_same(&res, &names(&["root", "foo", "baz"]));
}

#[test]
fn test_resolving_transitive_deps() {
    let reg = registry(vec![pkg!("foo"), pkg!("bar" => ["foo"])]);
    let res = resolve(&pkg_id("root"), vec![dep("bar")], &reg).unwrap();

    assert_that(&res, contains(names(&["root", "foo", "bar"])));
}

#[test]
fn test_resolving_common_transitive_deps() {
    let reg = registry(vec![pkg!("foo" => ["bar"]), pkg!("bar")]);
    let res = resolve(&pkg_id("root"), vec![dep("foo"), dep("bar")], &reg).unwrap();

    assert_that(&res, contains(names(&["root", "foo", "bar"])));
}

#[test]
fn test_resolving_with_same_name() {
    let list = vec![
        pkg_loc("foo", "http://first.example.com"),
        pkg_loc("bar", "http://second.example.com"),
    ];

    let reg = registry(list);
    let res = resolve(
        &pkg_id("root"),
        vec![
            dep_loc("foo", "http://first.example.com"),
            dep_loc("bar", "http://second.example.com"),
        ],
        &reg,
    ).unwrap();

    let mut names = loc_names(&[
        ("foo", "http://first.example.com"),
        ("bar", "http://second.example.com"),
    ]);

    names.push(pkg_id("root"));
    assert_same(&res, &names);
}

#[test]
fn test_resolving_with_dev_deps() {
    let reg = registry(vec![
        pkg!("foo" => ["bar", dep_kind("baz", Development)]),
        pkg!("baz" => ["bat", dep_kind("bam", Development)]),
        pkg!("bar"),
        pkg!("bat"),
    ]);

    let res = resolve(
        &pkg_id("root"),
        vec![dep("foo"), dep_kind("baz", Development)],
        &reg,
    ).unwrap();

    assert_that(&res, contains(names(&["root", "foo", "bar", "baz"])));
}

#[test]
fn resolving_with_many_versions() {
    let reg = registry(vec![pkg!(("foo", "1.0.1")), pkg!(("foo", "1.0.2"))]);

    let res = resolve(&pkg_id("root"), vec![dep("foo")], &reg).unwrap();

    assert_that(
        &res,
        contains(names(&[("root", "1.0.0"), ("foo", "1.0.2")])),
    );
}

#[test]
fn resolving_with_specific_version() {
    let reg = registry(vec![pkg!(("foo", "1.0.1")), pkg!(("foo", "1.0.2"))]);

    let res = resolve(&pkg_id("root"), vec![dep_req("foo", "=1.0.1")], &reg).unwrap();

    assert_that(
        &res,
        contains(names(&[("root", "1.0.0"), ("foo", "1.0.1")])),
    );
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

    let res = resolve(
        &pkg_id("root"),
        vec![dep_req("foo", "1.0.0"), dep_req("bar", "1.0.0")],
        &reg,
    ).unwrap();

    assert_that(
        &res,
        contains(names(&[
            ("root", "1.0.0"),
            ("foo", "1.0.0"),
            ("bar", "1.0.0"),
            ("util", "1.2.2"),
        ])),
    );
    assert_that(&res, is_not(contains(names(&[("util", "1.0.1")]))));
    assert_that(&res, is_not(contains(names(&[("util", "1.1.1")]))));
}

#[test]
fn test_resolving_minimum_version_with_transitive_deps() {
    enable_nightly_features(); // -Z minimal-versions
    // When the minimal-versions config option is specified then the lowest
    // possible version of a package should be selected. "util 1.0.0" can't be
    // selected because of the requirements of "bar", so the minimum version
    // must be 1.1.1.
    let reg = registry(vec![
        pkg!(("util", "1.2.2")),
        pkg!(("util", "1.0.0")),
        pkg!(("util", "1.1.1")),
        pkg!("foo" => [dep_req("util", "1.0.0")]),
        pkg!("bar" => [dep_req("util", ">=1.0.1")]),
    ]);

    let mut config = Config::default().unwrap();
    config
        .configure(
            1,
            None,
            &None,
            false,
            false,
            &None,
            &["minimal-versions".to_string()],
        )
        .unwrap();

    let res = resolve_with_config(
        &pkg_id("root"),
        vec![dep_req("foo", "1.0.0"), dep_req("bar", "1.0.0")],
        &reg,
        Some(&config),
    ).unwrap();

    assert_that(
        &res,
        contains(names(&[
            ("root", "1.0.0"),
            ("foo", "1.0.0"),
            ("bar", "1.0.0"),
            ("util", "1.1.1"),
        ])),
    );
    assert_that(&res, is_not(contains(names(&[("util", "1.2.2")]))));
    assert_that(&res, is_not(contains(names(&[("util", "1.0.0")]))));
}

// Ensure that the "-Z minimal-versions" CLI option works and the minimal
// version of a dependency ends up in the lock file.
#[test]
fn minimal_version_cli() {
    Package::new("dep", "1.0.0").publish();
    Package::new("dep", "1.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.1"

            [dependencies]
            dep = "1.0"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    assert_that(
        p.cargo("generate-lockfile -Zminimal-versions")
            .masquerade_as_nightly_cargo(),
        execs(),
    );

    let lock = p.read_lockfile();

    assert!(lock.contains("dep 1.0.0"));
}

#[test]
fn resolving_incompat_versions() {
    let reg = registry(vec![
        pkg!(("foo", "1.0.1")),
        pkg!(("foo", "1.0.2")),
        pkg!("bar" => [dep_req("foo", "=1.0.2")]),
    ]);

    assert!(
        resolve(
            &pkg_id("root"),
            vec![dep_req("foo", "=1.0.1"), dep("bar")],
            &reg
        ).is_err()
    );
}

#[test]
fn resolving_wrong_case_from_registry() {
    // In the future we may #5678 allow this to happen.
    // For back compatibility reasons, we probably won't.
    // But we may want to future prove ourselves by understanding it.
    // This test documents the current behavior.
    let reg = registry(vec![
        pkg!(("foo", "1.0.0")),
        pkg!("bar" => ["Foo"]),
    ]);

    assert!(
        resolve(
            &pkg_id("root"),
            vec![dep("bar")],
            &reg
        ).is_err()
    );
}

#[test]
fn resolving_mis_hyphenated_from_registry() {
    // In the future we may #2775 allow this to happen.
    // For back compatibility reasons, we probably won't.
    // But we may want to future prove ourselves by understanding it.
    // This test documents the current behavior.
    let reg = registry(vec![
        pkg!(("fo-o", "1.0.0")),
        pkg!("bar" => ["fo_o"]),
    ]);

    assert!(
        resolve(
            &pkg_id("root"),
            vec![dep("bar")],
            &reg
        ).is_err()
    );
}

#[test]
fn resolving_backtrack() {
    let reg = registry(vec![
        pkg!(("foo", "1.0.2") => [dep("bar")]),
        pkg!(("foo", "1.0.1") => [dep("baz")]),
        pkg!("bar" => [dep_req("foo", "=2.0.2")]),
        pkg!("baz"),
    ]);

    let res = resolve(&pkg_id("root"), vec![dep_req("foo", "^1")], &reg).unwrap();

    assert_that(
        &res,
        contains(names(&[
            ("root", "1.0.0"),
            ("foo", "1.0.1"),
            ("baz", "1.0.0"),
        ])),
    );
}

#[test]
fn resolving_backtrack_features() {
    // test for cargo/issues/4347
    let mut bad = dep("bar");
    bad.set_features(vec!["bad"]);

    let reg = registry(vec![
        pkg!(("foo", "1.0.2") => [bad]),
        pkg!(("foo", "1.0.1") => [dep("bar")]),
        pkg!("bar"),
    ]);

    let res = resolve(&pkg_id("root"), vec![dep_req("foo", "^1")], &reg).unwrap();

    assert_that(
        &res,
        contains(names(&[
            ("root", "1.0.0"),
            ("foo", "1.0.1"),
            ("bar", "1.0.0"),
        ])),
    );
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

    let res = resolve(&pkg_id("root"), vec![dep("bar")], &reg).unwrap();

    assert_that(
        &res,
        contains(names(&[
            ("root", "1.0.0"),
            ("foo", "1.0.0"),
            ("foo", "2.0.0"),
            ("foo", "0.1.0"),
            ("foo", "0.2.0"),
            ("d1", "1.0.0"),
            ("d2", "1.0.0"),
            ("d3", "1.0.0"),
            ("d4", "1.0.0"),
            ("bar", "1.0.0"),
        ])),
    );
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

    let res = resolve(&pkg_id("root"), vec![dep_req("foo", "1")], &reg).unwrap();

    assert_that(
        &res,
        contains(names(&[
            ("root", "1.0.0"),
            ("foo", "1.0.0"),
            ("bar", "2.0.0"),
            ("baz", "1.0.1"),
        ])),
    );
}

#[test]
fn resolving_with_sys_crates() {
    // This is based on issues/4902
    // With `l` a normal library we get 2copies so everyone gets the newest compatible.
    // But `l-sys` a library with a links attribute we make sure there is only one.
    let reg = registry(vec![
        pkg!(("l-sys", "0.9.1")),
        pkg!(("l-sys", "0.10.0")),
        pkg!(("l", "0.9.1")),
        pkg!(("l", "0.10.0")),
        pkg!(("d", "1.0.0") => [dep_req("l-sys", ">=0.8.0, <=0.10.0"), dep_req("l", ">=0.8.0, <=0.10.0")]),
        pkg!(("r", "1.0.0") => [dep_req("l-sys", "0.9"), dep_req("l", "0.9")]),
    ]);

    let res = resolve(
        &pkg_id("root"),
        vec![dep_req("d", "1"), dep_req("r", "1")],
        &reg,
    ).unwrap();

    assert_that(
        &res,
        contains(names(&[
            ("root", "1.0.0"),
            ("d", "1.0.0"),
            ("r", "1.0.0"),
            ("l-sys", "0.9.1"),
            ("l", "0.9.1"),
            ("l", "0.10.0"),
        ])),
    );
}

#[test]
fn resolving_with_constrained_sibling_backtrack_parent() {
    // There is no point in considering all of the backtrack_trap{1,2}
    // candidates since they can't change the result of failing to
    // resolve 'constrained'. Cargo should (ideally) skip past them and resume
    // resolution once the activation of the parent, 'bar', is rolled back.
    // Note that the traps are slightly more constrained to make sure they
    // get picked first.
    let mut reglist = vec![
        pkg!(("foo", "1.0.0") => [dep_req("bar", "1.0"),
                                  dep_req("constrained", "=1.0.0")]),
        pkg!(("bar", "1.0.0") => [dep_req("backtrack_trap1", "1.0.2"),
                                  dep_req("backtrack_trap2", "1.0.2"),
                                  dep_req("constrained", "1.0.0")]),
        pkg!(("constrained", "1.0.0")),
        pkg!(("backtrack_trap1", "1.0.0")),
        pkg!(("backtrack_trap2", "1.0.0")),
    ];
    // Bump this to make the test harder - it adds more versions of bar that will
    // fail to resolve, and more versions of the traps to consider.
    const NUM_BARS_AND_TRAPS: usize = 50; // minimum 2
    for i in 1..NUM_BARS_AND_TRAPS {
        let vsn = format!("1.0.{}", i);
        reglist.push(
            pkg!(("bar", vsn.clone()) => [dep_req("backtrack_trap1", "1.0.2"),
                                                   dep_req("backtrack_trap2", "1.0.2"),
                                                   dep_req("constrained", "1.0.1")]),
        );
        reglist.push(pkg!(("backtrack_trap1", vsn.clone())));
        reglist.push(pkg!(("backtrack_trap2", vsn.clone())));
        reglist.push(pkg!(("constrained", vsn.clone())));
    }
    let reg = registry(reglist);

    let res = resolve(&pkg_id("root"), vec![dep_req("foo", "1")], &reg).unwrap();

    assert_that(
        &res,
        contains(names(&[
            ("root", "1.0.0"),
            ("foo", "1.0.0"),
            ("bar", "1.0.0"),
            ("constrained", "1.0.0"),
        ])),
    );
}

#[test]
fn resolving_with_many_equivalent_backtracking() {
    let mut reglist = Vec::new();

    const DEPTH: usize = 200;
    const BRANCHING_FACTOR: usize = 100;

    // Each level depends on the next but the last level does not exist.
    // Without cashing we need to test every path to the last level O(BRANCHING_FACTOR ^ DEPTH)
    // and this test will time out. With cashing we need to discover that none of these
    // can be activated O(BRANCHING_FACTOR * DEPTH)
    for l in 0..DEPTH {
        let name = format!("level{}", l);
        let next = format!("level{}", l + 1);
        for i in 1..BRANCHING_FACTOR {
            let vsn = format!("1.0.{}", i);
            reglist.push(pkg!((name.as_str(), vsn.as_str()) => [dep_req(next.as_str(), "*")]));
        }
    }

    let reg = registry(reglist.clone());

    let res = resolve(&pkg_id("root"), vec![dep_req("level0", "*")], &reg);

    assert!(res.is_err());

    // It is easy to write code that quickly returns an error.
    // Lets make sure we can find a good answer if it is there.
    reglist.push(pkg!(("level0", "1.0.0")));

    let reg = registry(reglist.clone());

    let res = resolve(&pkg_id("root"), vec![dep_req("level0", "*")], &reg).unwrap();

    assert_that(
        &res,
        contains(names(&[("root", "1.0.0"), ("level0", "1.0.0")])),
    );

    // Make sure we have not special case no candidates.
    reglist.push(pkg!(("constrained", "1.1.0")));
    reglist.push(pkg!(("constrained", "1.0.0")));
    reglist.push(
        pkg!((format!("level{}", DEPTH).as_str(), "1.0.0") => [dep_req("constrained", "=1.0.0")]),
    );

    let reg = registry(reglist.clone());

    let res = resolve(
        &pkg_id("root"),
        vec![dep_req("level0", "*"), dep_req("constrained", "*")],
        &reg,
    ).unwrap();

    assert_that(
        &res,
        contains(names(&[
            ("root", "1.0.0"),
            ("level0", "1.0.0"),
            ("constrained", "1.1.0"),
        ])),
    );

    let reg = registry(reglist.clone());

    let res = resolve(
        &pkg_id("root"),
        vec![dep_req("level0", "1.0.1"), dep_req("constrained", "*")],
        &reg,
    ).unwrap();

    assert_that(
        &res,
        contains(names(&[
            ("root", "1.0.0"),
            (format!("level{}", DEPTH).as_str(), "1.0.0"),
            ("constrained", "1.0.0"),
        ])),
    );

    let reg = registry(reglist.clone());

    let res = resolve(
        &pkg_id("root"),
        vec![dep_req("level0", "1.0.1"), dep_req("constrained", "1.1.0")],
        &reg,
    );

    assert!(res.is_err());
}

#[test]
fn resolving_with_deep_traps() {
    let mut reglist = Vec::new();

    const DEPTH: usize = 200;
    const BRANCHING_FACTOR: usize = 100;

    // Each backtrack_trap depends on the next, and adds a backtrack frame.
    // None of witch is going to help with `bad`.
    for l in 0..DEPTH {
        let name = format!("backtrack_trap{}", l);
        let next = format!("backtrack_trap{}", l + 1);
        for i in 1..BRANCHING_FACTOR {
            let vsn = format!("1.0.{}", i);
            reglist.push(pkg!((name.as_str(), vsn.as_str()) => [dep_req(next.as_str(), "*")]));
        }
    }
    {
        let name = format!("backtrack_trap{}", DEPTH);
        for i in 1..BRANCHING_FACTOR {
            let vsn = format!("1.0.{}", i);
            reglist.push(pkg!((name.as_str(), vsn.as_str())));
        }
    }
    {
        // slightly less constrained to make sure `cloaking` gets picked last.
        for i in 1..(BRANCHING_FACTOR + 10) {
            let vsn = format!("1.0.{}", i);
            reglist.push(pkg!(("cloaking", vsn.as_str()) => [dep_req("bad", "1.0.1")]));
        }
    }

    let reg = registry(reglist.clone());

    let res = resolve(
        &pkg_id("root"),
        vec![dep_req("backtrack_trap0", "*"), dep_req("cloaking", "*")],
        &reg,
    );

    assert!(res.is_err());
}

#[test]
fn resolving_with_constrained_cousins_backtrack() {
    let mut reglist = Vec::new();

    const DEPTH: usize = 100;
    const BRANCHING_FACTOR: usize = 50;

    // Each backtrack_trap depends on the next.
    // The last depends on a specific ver of constrained.
    for l in 0..DEPTH {
        let name = format!("backtrack_trap{}", l);
        let next = format!("backtrack_trap{}", l + 1);
        for i in 1..BRANCHING_FACTOR {
            let vsn = format!("1.0.{}", i);
            reglist.push(pkg!((name.as_str(), vsn.as_str()) => [dep_req(next.as_str(), "*")]));
        }
    }
    {
        let name = format!("backtrack_trap{}", DEPTH);
        for i in 1..BRANCHING_FACTOR {
            let vsn = format!("1.0.{}", i);
            reglist.push(
                pkg!((name.as_str(), vsn.as_str()) => [dep_req("constrained", ">=1.1.0, <=2.0.0")]),
            );
        }
    }
    {
        // slightly less constrained to make sure `constrained` gets picked last.
        for i in 0..(BRANCHING_FACTOR + 10) {
            let vsn = format!("1.0.{}", i);
            reglist.push(pkg!(("constrained", vsn.as_str())));
        }
        reglist.push(pkg!(("constrained", "1.1.0")));
        reglist.push(pkg!(("constrained", "2.0.0")));
        reglist.push(pkg!(("constrained", "2.0.1")));
    }
    reglist.push(pkg!(("cloaking", "1.0.0") => [dep_req("constrained", "~1.0.0")]));

    let reg = registry(reglist.clone());

    // `backtrack_trap0 = "*"` is a lot of ways of saying `constrained = ">=1.1.0, <=2.0.0"`
    // but `constrained= "2.0.1"` is already picked.
    // Only then to try and solve `constrained= "~1.0.0"` which is incompatible.
    let res = resolve(
        &pkg_id("root"),
        vec![
            dep_req("backtrack_trap0", "*"),
            dep_req("constrained", "2.0.1"),
            dep_req("cloaking", "*"),
        ],
        &reg,
    );

    assert!(res.is_err());

    // Each level depends on the next but the last depends on incompatible deps.
    // Let's make sure that we can cache that a dep has incompatible deps.
    for l in 0..DEPTH {
        let name = format!("level{}", l);
        let next = format!("level{}", l + 1);
        for i in 1..BRANCHING_FACTOR {
            let vsn = format!("1.0.{}", i);
            reglist.push(pkg!((name.as_str(), vsn.as_str()) => [dep_req(next.as_str(), "*")]));
        }
    }
    reglist.push(
        pkg!((format!("level{}", DEPTH).as_str(), "1.0.0") => [dep_req("backtrack_trap0", "*"),
            dep_req("cloaking", "*")
        ]),
    );

    let reg = registry(reglist.clone());

    let res = resolve(
        &pkg_id("root"),
        vec![dep_req("level0", "*"), dep_req("constrained", "2.0.1")],
        &reg,
    );

    assert!(res.is_err());

    let res = resolve(
        &pkg_id("root"),
        vec![dep_req("level0", "*"), dep_req("constrained", "2.0.0")],
        &reg,
    ).unwrap();

    assert_that(
        &res,
        contains(names(&[("constrained", "2.0.0"), ("cloaking", "1.0.0")])),
    );
}

#[test]
fn resolving_with_constrained_sibling_backtrack_activation() {
    // It makes sense to resolve most-constrained deps first, but
    // with that logic the backtrack traps here come between the two
    // attempted resolutions of 'constrained'. When backtracking,
    // cargo should skip past them and resume resolution once the
    // number of activations for 'constrained' changes.
    let mut reglist = vec![
        pkg!(("foo", "1.0.0") => [dep_req("bar", "=1.0.0"),
                                  dep_req("backtrack_trap1", "1.0"),
                                  dep_req("backtrack_trap2", "1.0"),
                                  dep_req("constrained", "<=1.0.60")]),
        pkg!(("bar", "1.0.0") => [dep_req("constrained", ">=1.0.60")]),
    ];
    // Bump these to make the test harder, but you'll also need to
    // change the version constraints on `constrained` above. To correctly
    // exercise Cargo, the relationship between the values is:
    // NUM_CONSTRAINED - vsn < NUM_TRAPS < vsn
    // to make sure the traps are resolved between `constrained`.
    const NUM_TRAPS: usize = 45; // min 1
    const NUM_CONSTRAINED: usize = 100; // min 1
    for i in 0..NUM_TRAPS {
        let vsn = format!("1.0.{}", i);
        reglist.push(pkg!(("backtrack_trap1", vsn.clone())));
        reglist.push(pkg!(("backtrack_trap2", vsn.clone())));
    }
    for i in 0..NUM_CONSTRAINED {
        let vsn = format!("1.0.{}", i);
        reglist.push(pkg!(("constrained", vsn.clone())));
    }
    let reg = registry(reglist);

    let res = resolve(&pkg_id("root"), vec![dep_req("foo", "1")], &reg).unwrap();

    assert_that(
        &res,
        contains(names(&[
            ("root", "1.0.0"),
            ("foo", "1.0.0"),
            ("bar", "1.0.0"),
            ("constrained", "1.0.60"),
        ])),
    );
}

#[test]
fn resolving_with_constrained_sibling_transitive_dep_effects() {
    // When backtracking due to a failed dependency, if Cargo is
    // trying to be clever and skip irrelevant dependencies, care must
    // be taken to not miss the transitive effects of alternatives. E.g.
    // in the right-to-left resolution of the graph below, B may
    // affect whether D is successfully resolved.
    //
    //    A
    //  / | \
    // B  C  D
    // |  |
    // C  D
    let reg = registry(vec![
        pkg!(("A", "1.0.0") => [dep_req("B", "1.0"),
                                dep_req("C", "1.0"),
                                dep_req("D", "1.0.100")]),
        pkg!(("B", "1.0.0") => [dep_req("C", ">=1.0.0")]),
        pkg!(("B", "1.0.1") => [dep_req("C", ">=1.0.1")]),
        pkg!(("C", "1.0.0") => [dep_req("D", "1.0.0")]),
        pkg!(("C", "1.0.1") => [dep_req("D", ">=1.0.1,<1.0.100")]),
        pkg!(("C", "1.0.2") => [dep_req("D", ">=1.0.2,<1.0.100")]),
        pkg!(("D", "1.0.0")),
        pkg!(("D", "1.0.1")),
        pkg!(("D", "1.0.2")),
        pkg!(("D", "1.0.100")),
        pkg!(("D", "1.0.101")),
        pkg!(("D", "1.0.102")),
        pkg!(("D", "1.0.103")),
        pkg!(("D", "1.0.104")),
        pkg!(("D", "1.0.105")),
    ]);

    let res = resolve(&pkg_id("root"), vec![dep_req("A", "1")], &reg).unwrap();

    assert_that(
        &res,
        contains(names(&[
            ("A", "1.0.0"),
            ("B", "1.0.0"),
            ("C", "1.0.0"),
            ("D", "1.0.105"),
        ])),
    );
}

#[test]
fn resolving_but_no_exists() {
    let reg = registry(vec![]);

    let res = resolve(&pkg_id("root"), vec![dep_req("foo", "1")], &reg);
    assert!(res.is_err());

    assert_eq!(
        res.err().unwrap().to_string(),
        "\
         no matching package named `foo` found\n\
         location searched: registry `http://example.com/`\n\
         required by package `root v1.0.0 (registry `http://example.com/`)`\
         "
    );
}

#[test]
fn resolving_cycle() {
    let reg = registry(vec![pkg!("foo" => ["foo"])]);

    let _ = resolve(&pkg_id("root"), vec![dep_req("foo", "1")], &reg);
}

#[test]
fn hard_equality() {
    let reg = registry(vec![
        pkg!(("foo", "1.0.1")),
        pkg!(("foo", "1.0.0")),
        pkg!(("bar", "1.0.0") => [dep_req("foo", "1.0.0")]),
    ]);

    let res = resolve(
        &pkg_id("root"),
        vec![dep_req("bar", "1"), dep_req("foo", "=1.0.0")],
        &reg,
    ).unwrap();

    assert_that(
        &res,
        contains(names(&[
            ("root", "1.0.0"),
            ("foo", "1.0.0"),
            ("bar", "1.0.0"),
        ])),
    );
}
