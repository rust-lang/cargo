use std::cmp::PartialEq;
use std::cmp::{max, min};
use std::collections::{BTreeMap, HashSet};
use std::env;
use std::fmt;
use std::time::{Duration, Instant};

use cargo::core::dependency::Kind::{self, Development};
use cargo::core::resolver::{self, Method};
use cargo::core::source::{GitReference, SourceId};
use cargo::core::{enable_nightly_features, Dependency, PackageId, Registry, Summary};
use cargo::util::{CargoResult, Config, ToUrl};

use support::project;
use support::registry::Package;

use proptest::collection::{btree_map, btree_set, vec};
use proptest::prelude::*;
use proptest::sample::Index;
use proptest::strategy::ValueTree;
use proptest::string::string_regex;
use proptest::test_runner::TestRunner;

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
        fn query(
            &mut self,
            dep: &Dependency,
            f: &mut FnMut(Summary),
            fuzzy: bool,
        ) -> CargoResult<()> {
            for summary in self.0.iter() {
                if fuzzy || dep.matches(summary) {
                    f(summary.clone());
                }
            }
            Ok(())
        }
    }
    let mut registry = MyRegistry(registry);
    let summary = Summary::new(
        pkg.clone(),
        deps,
        &BTreeMap::<String, Vec<String>>::new(),
        None::<String>,
        false,
    ).unwrap();
    let method = Method::Everything;
    let start = Instant::now();
    let resolve = resolver::resolve(
        &[(summary, method)],
        &[],
        &mut registry,
        &HashSet::new(),
        config,
        false,
    )?;

    // The largest test in our sweet takes less then 30 sec.
    // So lets fail the test if we have ben running for two long.
    assert!(start.elapsed() < Duration::from_secs(60));
    let res = resolve.iter().cloned().collect();
    Ok(res)
}

trait ToDep {
    fn to_dep(self) -> Dependency;
}

impl ToDep for &'static str {
    fn to_dep(self) -> Dependency {
        Dependency::parse_no_deprecated(self, Some("1.0.0"), &registry_loc()).unwrap()
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

impl ToPkgId for PackageId {
    fn to_pkgid(&self) -> PackageId {
        self.clone()
    }
}

impl<'a> ToPkgId for &'a str {
    fn to_pkgid(&self) -> PackageId {
        PackageId::new(*self, "1.0.0", &registry_loc()).unwrap()
    }
}

impl<T: AsRef<str>, U: AsRef<str>> ToPkgId for (T, U) {
    fn to_pkgid(&self) -> PackageId {
        let (name, vers) = self;
        PackageId::new(name.as_ref(), vers.as_ref(), &registry_loc()).unwrap()
    }
}

macro_rules! pkg {
    ($pkgid:expr => [$($deps:expr),+ $(,)* ]) => ({
        let d: Vec<Dependency> = vec![$($deps.to_dep()),+];
        pkg_dep($pkgid, d)
    });

    ($pkgid:expr) => ({
        pkg($pkgid)
    })
}

fn registry_loc() -> SourceId {
    lazy_static! {
        static ref EXAMPLE_DOT_COM: SourceId =
            SourceId::for_registry(&"http://example.com".to_url().unwrap()).unwrap();
    }
    EXAMPLE_DOT_COM.clone()
}

fn pkg<T: ToPkgId>(name: T) -> Summary {
    pkg_dep(name, Vec::new())
}

fn pkg_dep<T: ToPkgId>(name: T, dep: Vec<Dependency>) -> Summary {
    let pkgid = name.to_pkgid();
    let link = if pkgid.name().ends_with("-sys") {
        Some(pkgid.name().as_str())
    } else {
        None
    };
    Summary::new(
        name.to_pkgid(),
        dep,
        &BTreeMap::<String, Vec<String>>::new(),
        link,
        false,
    ).unwrap()
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
    Dependency::parse_no_deprecated(name, Some(req), &registry_loc()).unwrap()
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

/// By default `Summary` and `Dependency` have a very verbose `Debug` representation.
/// This replaces with a representation that uses constructors from this file.
///
/// If `registry_strategy` is improved to modify more fields
/// then this needs to update to display the corresponding constructor.
struct PrettyPrintRegistry(Vec<Summary>);

impl fmt::Debug for PrettyPrintRegistry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "vec![")?;
        for s in &self.0 {
            if s.dependencies().is_empty() {
                write!(f, "pkg!((\"{}\", \"{}\")),", s.name(), s.version())?;
            } else {
                write!(f, "pkg!((\"{}\", \"{}\") => [", s.name(), s.version())?;
                for d in s.dependencies() {
                    write!(
                        f,
                        "dep_req(\"{}\", \"{}\"),",
                        d.name_in_toml(),
                        d.version_req()
                    )?;
                }
                write!(f, "]),")?;
            }
        }
        write!(f, "]")
    }
}

#[test]
fn meta_test_deep_pretty_print_registry() {
    assert_eq!(
        &format!(
            "{:?}",
            PrettyPrintRegistry(vec![
                pkg!(("foo", "1.0.1") => [dep_req("bar", "1")]),
                pkg!(("foo", "1.0.0") => [dep_req("bar", "2")]),
                pkg!(("bar", "1.0.0") => [dep_req("baz", "=1.0.2"),
                                  dep_req("other", "1")]),
                pkg!(("bar", "2.0.0") => [dep_req("baz", "=1.0.1")]),
                pkg!(("baz", "1.0.2") => [dep_req("other", "2")]),
                pkg!(("baz", "1.0.1")),
                pkg!(("dep_req", "1.0.0")),
                pkg!(("dep_req", "2.0.0")),
            ])
        ),
        "vec![pkg!((\"foo\", \"1.0.1\") => [dep_req(\"bar\", \"^1\"),]),\
         pkg!((\"foo\", \"1.0.0\") => [dep_req(\"bar\", \"^2\"),]),\
         pkg!((\"bar\", \"1.0.0\") => [dep_req(\"baz\", \"= 1.0.2\"),dep_req(\"other\", \"^1\"),]),\
         pkg!((\"bar\", \"2.0.0\") => [dep_req(\"baz\", \"= 1.0.1\"),]),\
         pkg!((\"baz\", \"1.0.2\") => [dep_req(\"other\", \"^2\"),]),\
         pkg!((\"baz\", \"1.0.1\")),pkg!((\"dep_req\", \"1.0.0\")),\
         pkg!((\"dep_req\", \"2.0.0\")),]"
    )
}

/// This generates a random registry index.
/// Unlike vec((Name, Ver, vec((Name, VerRq), ..), ..)
/// This strategy has a high probability of having valid dependencies
fn registry_strategy(
    max_crates: usize,
    max_versions: usize,
) -> impl Strategy<Value = PrettyPrintRegistry> {
    let name = string_regex("[A-Za-z_-][A-Za-z0-9_-]*(-sys)?").unwrap();

    let raw_version = [..max_versions; 3];
    let version_from_raw = |v: &[usize; 3]| format!("{}.{}.{}", v[0], v[1], v[2]);

    // If this is false than the crate will depend on the nonexistent "bad"
    // instead of the complex set we generated for it.
    let allow_deps = prop::bool::weighted(0.95);

    let list_of_versions =
        btree_set((raw_version, allow_deps), 1..=max_versions).prop_map(move |ver| {
            ver.iter()
                .map(|a| (version_from_raw(&a.0), a.1))
                .collect::<Vec<_>>()
        });

    let list_of_crates_with_versions =
        btree_map(name, list_of_versions, 1..=max_crates).prop_map(|mut vers| {
            // root is the name of the thing being compiled
            // so it would be confusing to have it in the index
            vers.remove("root");
            // bad is a name reserved for a dep that won't work
            vers.remove("bad");
            vers
        });

    // each version of each crate can depend on each crate smaller then it
    let max_deps = 1 + max_versions * (max_crates * (max_crates - 1)) / 2;

    let raw_version_range = (any::<Index>(), any::<Index>());
    let raw_dependency = (any::<Index>(), any::<Index>(), raw_version_range);

    fn order_index(a: Index, b: Index, size: usize) -> (usize, usize) {
        let (a, b) = (a.index(size), b.index(size));
        (min(a, b), max(a, b))
    }

    let list_of_raw_dependency = vec(raw_dependency, ..=max_deps);

    (list_of_crates_with_versions, list_of_raw_dependency).prop_map(
        |(crate_vers_by_name, raw_dependencies)| {
            let list_of_pkgid: Vec<_> = crate_vers_by_name
                .iter()
                .flat_map(|(name, vers)| vers.iter().map(move |x| ((name.as_str(), &x.0), x.1)))
                .collect();
            let len_all_pkgid = list_of_pkgid.len();
            let mut dependency_by_pkgid = vec![vec![]; len_all_pkgid];
            for (a, b, (c, d)) in raw_dependencies {
                let (a, b) = order_index(a, b, len_all_pkgid);
                let ((dep_name, _), _) = list_of_pkgid[a];
                if (list_of_pkgid[b].0).0 == dep_name {
                    continue;
                }
                let s = &crate_vers_by_name[dep_name];
                let (c, d) = order_index(c, d, s.len());

                dependency_by_pkgid[b].push(dep_req(
                    &dep_name,
                    &if c == d {
                        format!("={}", s[c].0)
                    } else {
                        format!(">={}, <={}", s[c].0, s[d].0)
                    },
                ))
            }

            PrettyPrintRegistry(
                list_of_pkgid
                    .into_iter()
                    .zip(dependency_by_pkgid.into_iter())
                    .map(|(((name, ver), allow_deps), deps)| {
                        pkg_dep(
                            (name, ver).to_pkgid(),
                            if !allow_deps {
                                vec![dep_req("bad", "*")]
                            } else {
                                let mut deps = deps;
                                deps.sort_by_key(|d| d.name_in_toml());
                                deps.dedup_by_key(|d| d.name_in_toml());
                                deps
                            },
                        )
                    }).collect(),
            )
        },
    )
}

/// This test is to test the generator to ensure
/// that it makes registries with large dependency trees
#[test]
fn meta_test_deep_trees_from_strategy() {
    let mut seen_an_error = false;
    let mut seen_a_deep_tree = false;

    let strategy = registry_strategy(50, 10);
    for _ in 0..256 {
        let PrettyPrintRegistry(input) = strategy
            .new_tree(&mut TestRunner::default())
            .unwrap()
            .current();
        let reg = registry(input.clone());
        for this in input.iter().rev().take(10) {
            let res = resolve(
                &pkg_id("root"),
                vec![dep_req(&this.name(), &format!("={}", this.version()))],
                &reg,
            );
            match res {
                Ok(r) => {
                    if r.len() >= 7 {
                        seen_a_deep_tree = true;
                    }
                }
                Err(_) => {
                    seen_an_error = true;
                }
            }
            if seen_a_deep_tree && seen_an_error {
                return;
            }
        }
    }

    assert!(
        seen_an_error,
        "In 2560 tries we did not see any crates that could not be built!"
    );
    assert!(
        seen_a_deep_tree,
        "In 2560 tries we did not see any crates that had more then 7 pkg in the dependency tree!"
    );
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases:
            if env::var("CI").is_ok() {
                256 // we have a lot of builds in CI so one or another of them will find problems
            } else {
                1024 // but locally try and find it in the one build
            },
        max_shrink_iters:
            if env::var("CI").is_ok() {
                // This attempts to make sure that CI will fail fast,
                0
            } else {
                // but that local builds will give a small clear test case.
                ProptestConfig::default().max_shrink_iters
            },
        .. ProptestConfig::default()
    })]
    #[test]
    fn limited_independence_of_irrelevant_alternatives(
        PrettyPrintRegistry(input) in registry_strategy(50, 10),
        indexs_to_unpublish in vec(any::<prop::sample::Index>(), 10)
    )  {
        let reg = registry(input.clone());
        // there is only a small chance that eny one
        // crate will be interesting.
        // So we try some of the most complicated.
        for this in input.iter().rev().take(10) {
            let res = resolve(
                &pkg_id("root"),
                vec![dep_req(&this.name(), &format!("={}", this.version()))],
                &reg,
            );

            match res {
                Ok(r) => {
                    // If resolution was successful, then unpublishing a version of a crate
                    // that was not selected should not change that.
                    let not_selected: Vec<_> = input
                        .iter()
                        .cloned()
                        .filter(|x| !r.contains(x.package_id()))
                        .collect();
                    if !not_selected.is_empty() {
                        for index_to_unpublish in &indexs_to_unpublish {
                            let summary_to_unpublish = index_to_unpublish.get(&not_selected);
                            let new_reg = registry(
                                input
                                    .iter()
                                    .cloned()
                                    .filter(|x| summary_to_unpublish != x)
                                    .collect(),
                            );

                            let res = resolve(
                                &pkg_id("root"),
                                vec![dep_req(&this.name(), &format!("={}", this.version()))],
                                &new_reg,
                            );

                            // Note: that we can not assert that the two `res` are identical
                            // as the resolver does depend on irrelevant alternatives.
                            // It uses how constrained a dependency requirement is
                            // to determine what order to evaluate requirements.

                            prop_assert!(
                                res.is_ok(),
                                "unpublishing {:?} stopped `{} = \"={}\"` from working",
                                summary_to_unpublish.package_id(),
                                this.name(),
                                this.version()
                            )
                        }
                    }
                }

                Err(_) => {
                    // If resolution was unsuccessful, then it should stay unsuccessful
                    // even if any version of a crate is unpublished.
                    for index_to_unpublish in &indexs_to_unpublish {
                        let summary_to_unpublish = index_to_unpublish.get(&input);
                        let new_reg = registry(
                            input
                                .iter()
                                .cloned()
                                .filter(|x| summary_to_unpublish != x)
                                .collect(),
                        );

                        let res = resolve(
                            &pkg_id("root"),
                            vec![dep_req(&this.name(), &format!("={}", this.version()))],
                            &new_reg,
                        );

                        prop_assert!(
                            res.is_err(),
                            "full index did not work for `{} = \"={}\"` but unpublishing {:?} fixed it!",
                            this.name(),
                            this.version(),
                            summary_to_unpublish.package_id()
                        )
                    }
                }
            }
        }
    }
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

/// Assert `xs` contains `elems`
fn assert_contains<A: PartialEq>(xs: &[A], elems: &[A]) {
    for elem in elems {
        assert!(xs.contains(elem));
    }
}

fn assert_same<A: PartialEq>(a: &[A], b: &[A]) {
    assert_eq!(a.len(), b.len());
    assert_contains(b, a);
}

#[test]
fn test_resolving_only_package() {
    let reg = registry(vec![pkg!("foo")]);
    let res = resolve(&pkg_id("root"), vec![dep("foo")], &reg).unwrap();
    assert_same(&res, &names(&["root", "foo"]));
}

#[test]
fn test_resolving_one_dep() {
    let reg = registry(vec![pkg!("foo"), pkg!("bar")]);
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

    assert_contains(&res, &names(&["root", "foo", "bar"]));
}

#[test]
fn test_resolving_common_transitive_deps() {
    let reg = registry(vec![pkg!("foo" => ["bar"]), pkg!("bar")]);
    let res = resolve(&pkg_id("root"), vec![dep("foo"), dep("bar")], &reg).unwrap();

    assert_contains(&res, &names(&["root", "foo", "bar"]));
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

    assert_contains(&res, &names(&["root", "foo", "bar", "baz"]));
}

#[test]
fn resolving_with_many_versions() {
    let reg = registry(vec![pkg!(("foo", "1.0.1")), pkg!(("foo", "1.0.2"))]);

    let res = resolve(&pkg_id("root"), vec![dep("foo")], &reg).unwrap();

    assert_contains(&res, &names(&[("root", "1.0.0"), ("foo", "1.0.2")]));
}

#[test]
fn resolving_with_specific_version() {
    let reg = registry(vec![pkg!(("foo", "1.0.1")), pkg!(("foo", "1.0.2"))]);

    let res = resolve(&pkg_id("root"), vec![dep_req("foo", "=1.0.1")], &reg).unwrap();

    assert_contains(&res, &names(&[("root", "1.0.0"), ("foo", "1.0.1")]));
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

    assert_contains(
        &res,
        &names(&[
            ("root", "1.0.0"),
            ("foo", "1.0.0"),
            ("bar", "1.0.0"),
            ("util", "1.2.2"),
        ]),
    );
    assert!(!res.contains(&("util", "1.0.1").to_pkgid()));
    assert!(!res.contains(&("util", "1.1.1").to_pkgid()));
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
        ).unwrap();

    let res = resolve_with_config(
        &pkg_id("root"),
        vec![dep_req("foo", "1.0.0"), dep_req("bar", "1.0.0")],
        &reg,
        Some(&config),
    ).unwrap();

    assert_contains(
        &res,
        &names(&[
            ("root", "1.0.0"),
            ("foo", "1.0.0"),
            ("bar", "1.0.0"),
            ("util", "1.1.1"),
        ]),
    );
    assert!(!res.contains(&("util", "1.2.2").to_pkgid()));
    assert!(!res.contains(&("util", "1.0.0").to_pkgid()));
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
        ).file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("generate-lockfile -Zminimal-versions")
        .masquerade_as_nightly_cargo()
        .run();

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
    let reg = registry(vec![pkg!(("foo", "1.0.0")), pkg!("bar" => ["Foo"])]);

    assert!(resolve(&pkg_id("root"), vec![dep("bar")], &reg).is_err());
}

#[test]
fn resolving_mis_hyphenated_from_registry() {
    // In the future we may #2775 allow this to happen.
    // For back compatibility reasons, we probably won't.
    // But we may want to future prove ourselves by understanding it.
    // This test documents the current behavior.
    let reg = registry(vec![pkg!(("fo-o", "1.0.0")), pkg!("bar" => ["fo_o"])]);

    assert!(resolve(&pkg_id("root"), vec![dep("bar")], &reg).is_err());
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

    assert_contains(
        &res,
        &names(&[("root", "1.0.0"), ("foo", "1.0.1"), ("baz", "1.0.0")]),
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

    assert_contains(
        &res,
        &names(&[("root", "1.0.0"), ("foo", "1.0.1"), ("bar", "1.0.0")]),
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

    assert_contains(
        &res,
        &names(&[
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
        ]),
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

    assert_contains(
        &res,
        &names(&[
            ("root", "1.0.0"),
            ("foo", "1.0.0"),
            ("bar", "2.0.0"),
            ("baz", "1.0.1"),
        ]),
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

    assert_contains(
        &res,
        &names(&[
            ("root", "1.0.0"),
            ("d", "1.0.0"),
            ("r", "1.0.0"),
            ("l-sys", "0.9.1"),
            ("l", "0.9.1"),
            ("l", "0.10.0"),
        ]),
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

    assert_contains(
        &res,
        &names(&[
            ("root", "1.0.0"),
            ("foo", "1.0.0"),
            ("bar", "1.0.0"),
            ("constrained", "1.0.0"),
        ]),
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

    assert_contains(&res, &names(&[("root", "1.0.0"), ("level0", "1.0.0")]));

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

    assert_contains(
        &res,
        &names(&[
            ("root", "1.0.0"),
            ("level0", "1.0.0"),
            ("constrained", "1.1.0"),
        ]),
    );

    let reg = registry(reglist.clone());

    let res = resolve(
        &pkg_id("root"),
        vec![dep_req("level0", "1.0.1"), dep_req("constrained", "*")],
        &reg,
    ).unwrap();

    assert_contains(
        &res,
        &names(&[
            ("root", "1.0.0"),
            (format!("level{}", DEPTH).as_str(), "1.0.0"),
            ("constrained", "1.0.0"),
        ]),
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

    assert_contains(
        &res,
        &names(&[("constrained", "2.0.0"), ("cloaking", "1.0.0")]),
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

    assert_contains(
        &res,
        &names(&[
            ("root", "1.0.0"),
            ("foo", "1.0.0"),
            ("bar", "1.0.0"),
            ("constrained", "1.0.60"),
        ]),
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

    assert_contains(
        &res,
        &names(&[
            ("A", "1.0.0"),
            ("B", "1.0.0"),
            ("C", "1.0.0"),
            ("D", "1.0.105"),
        ]),
    );
}

#[test]
fn incomplete_information_skiping() {
    // When backtracking due to a failed dependency, if Cargo is
    // trying to be clever and skip irrelevant dependencies, care must
    // be taken to not miss the transitive effects of alternatives.
    // Fuzzing discovered that for some reason cargo was skiping based
    // on incomplete information in the following case:
    // minimized bug found in:
    // https://github.com/rust-lang/cargo/commit/003c29b0c71e5ea28fbe8e72c148c755c9f3f8d9
    let input = vec![
        pkg!(("a", "1.0.0")),
        pkg!(("a", "1.1.0")),
        pkg!("b" => [dep("a")]),
        pkg!(("c", "1.0.0")),
        pkg!(("c", "1.1.0")),
        pkg!("d" => [dep_req("c", "=1.0")]),
        pkg!(("e", "1.0.0")),
        pkg!(("e", "1.1.0") => [dep_req("c", "1.1")]),
        pkg!("to_yank"),
        pkg!(("f", "1.0.0") => [
            dep("to_yank"),
            dep("d"),
        ]),
        pkg!(("f", "1.1.0") => [dep("d")]),
        pkg!("g" => [
            dep("b"),
            dep("e"),
            dep("f"),
        ]),
    ];
    let reg = registry(input.clone());

    let res = resolve(&pkg_id("root"), vec![dep("g")], &reg).unwrap();
    let package_to_yank = "to_yank".to_pkgid();
    // this package is not used in the resolution.
    assert!(!res.contains(&package_to_yank));
    // so when we yank it
    let new_reg = registry(
        input
            .iter()
            .cloned()
            .filter(|x| &package_to_yank != x.package_id())
            .collect(),
    );
    assert_eq!(input.len(), new_reg.len() + 1);
    // it should still build
    assert!(resolve(&pkg_id("root"), vec![dep("g")], &new_reg).is_ok());
}

#[test]
fn incomplete_information_skiping_2() {
    // When backtracking due to a failed dependency, if Cargo is
    // trying to be clever and skip irrelevant dependencies, care must
    // be taken to not miss the transitive effects of alternatives.
    // Fuzzing discovered that for some reason cargo was skiping based
    // on incomplete information in the following case:
    // https://github.com/rust-lang/cargo/commit/003c29b0c71e5ea28fbe8e72c148c755c9f3f8d9
    let input = vec![
        pkg!(("b", "3.8.10")),
        pkg!(("b", "8.7.4")),
        pkg!(("b", "9.4.6")),
        pkg!(("c", "1.8.8")),
        pkg!(("c", "10.2.5")),
        pkg!(("d", "4.1.2") => [
            dep_req("bad", "=6.10.9"),
        ]),
        pkg!(("d", "5.5.6")),
        pkg!(("d", "5.6.10")),
        pkg!(("to_yank", "8.0.1")),
        pkg!(("to_yank", "8.8.1")),
        pkg!(("e", "4.7.8") => [
            dep_req("d", ">=5.5.6, <=5.6.10"),
            dep_req("to_yank", "=8.0.1"),
        ]),
        pkg!(("e", "7.4.9") => [
            dep_req("bad", "=4.7.5"),
        ]),
        pkg!("f" => [
            dep_req("d", ">=4.1.2, <=5.5.6"),
        ]),
        pkg!("g" => [
            dep("bad"),
        ]),
        pkg!(("h", "3.8.3") => [
            dep_req("g", "*"),
        ]),
        pkg!(("h", "6.8.3") => [
            dep("f"),
        ]),
        pkg!(("h", "8.1.9") => [
            dep_req("to_yank", "=8.8.1"),
        ]),
        pkg!("i" => [
            dep_req("b", "*"),
            dep_req("c", "*"),
            dep_req("e", "*"),
            dep_req("h", "*"),
        ]),
    ];
    let reg = registry(input.clone());

    let res = resolve(&pkg_id("root"), vec![dep("i")], &reg).unwrap();
    let package_to_yank = ("to_yank", "8.8.1").to_pkgid();
    // this package is not used in the resolution.
    assert!(!res.contains(&package_to_yank));
    // so when we yank it
    let new_reg = registry(
        input
            .iter()
            .cloned()
            .filter(|x| &package_to_yank != x.package_id())
            .collect(),
    );
    assert_eq!(input.len(), new_reg.len() + 1);
    // it should still build
    assert!(resolve(&pkg_id("root"), vec![dep("i")], &new_reg).is_ok());
}

#[test]
fn incomplete_information_skiping_3() {
    // When backtracking due to a failed dependency, if Cargo is
    // trying to be clever and skip irrelevant dependencies, care must
    // be taken to not miss the transitive effects of alternatives.
    // Fuzzing discovered that for some reason cargo was skiping based
    // on incomplete information in the following case:
    // minimized bug found in:
    // https://github.com/rust-lang/cargo/commit/003c29b0c71e5ea28fbe8e72c148c755c9f3f8d9
    let input = vec![
        pkg!{("to_yank", "3.0.3")},
        pkg!{("to_yank", "3.3.0")},
        pkg!{("to_yank", "3.3.1")},
        pkg!{("a", "3.3.0") => [
            dep_req("to_yank", "=3.0.3"),
        ] },
        pkg!{("a", "3.3.2") => [
            dep_req("to_yank", "<=3.3.0"),
        ] },
        pkg!{("b", "0.1.3") => [
            dep_req("a", "=3.3.0"),
        ] },
        pkg!{("b", "2.0.2") => [
            dep_req("to_yank", "3.3.0"),
            dep_req("a", "*"),
        ] },
        pkg!{("b", "2.3.3") => [
            dep_req("to_yank", "3.3.0"),
            dep_req("a", "=3.3.0"),
        ] },
    ];
    let reg = registry(input.clone());

    let res = resolve(&pkg_id("root"), vec![dep_req("b", "*")], &reg).unwrap();
    let package_to_yank = ("to_yank", "3.0.3").to_pkgid();
    // this package is not used in the resolution.
    assert!(!res.contains(&package_to_yank));
    // so when we yank it
    let new_reg = registry(
        input
            .iter()
            .cloned()
            .filter(|x| &package_to_yank != x.package_id())
            .collect(),
    );
    assert_eq!(input.len(), new_reg.len() + 1);
    // it should still build
    assert!(resolve(&pkg_id("root"), vec![dep_req("b", "*")], &new_reg).is_ok());
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

    assert_contains(
        &res,
        &names(&[("root", "1.0.0"), ("foo", "1.0.0"), ("bar", "1.0.0")]),
    );
}
