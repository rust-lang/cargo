use std::cmp::PartialEq;
use std::cmp::{max, min};
use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::time::Instant;

use crate::support::slow_cpu_multiplier;

use cargo::core::dependency::Kind;
use cargo::core::resolver::{self, Method};
use cargo::core::source::{GitReference, SourceId};
use cargo::core::Resolve;
use cargo::core::{Dependency, PackageId, Registry, Summary};
use cargo::util::{CargoResult, Config, ToUrl};

use proptest::collection::{btree_map, vec};
use proptest::prelude::*;
use proptest::sample::Index;
use proptest::strategy::ValueTree;
use proptest::string::string_regex;
use proptest::test_runner::TestRunner;

pub fn resolve(
    pkg: PackageId,
    deps: Vec<Dependency>,
    registry: &[Summary],
) -> CargoResult<Vec<PackageId>> {
    resolve_with_config(pkg, deps, registry, None)
}

pub fn resolve_and_validated(
    pkg: PackageId,
    deps: Vec<Dependency>,
    registry: &[Summary],
) -> CargoResult<Vec<PackageId>> {
    let resolve = resolve_with_config_raw(pkg, deps, registry, None)?;
    let mut stack = vec![pkg];
    let mut used = HashSet::new();
    let mut links = HashSet::new();
    while let Some(p) = stack.pop() {
        assert!(resolve.contains(&p));
        if used.insert(p) {
            // in the tests all `links` crates end in `-sys`
            if p.name().ends_with("-sys") {
                assert!(links.insert(p.name()));
            }
            stack.extend(resolve.deps(p).map(|(dp, deps)| {
                for d in deps {
                    assert!(d.matches_id(dp));
                }
                dp
            }));
        }
    }
    let out = resolve.sort();
    assert_eq!(out.len(), used.len());
    Ok(out)
}

pub fn resolve_with_config(
    pkg: PackageId,
    deps: Vec<Dependency>,
    registry: &[Summary],
    config: Option<&Config>,
) -> CargoResult<Vec<PackageId>> {
    let resolve = resolve_with_config_raw(pkg, deps, registry, config)?;
    Ok(resolve.sort())
}

pub fn resolve_with_config_raw(
    pkg: PackageId,
    deps: Vec<Dependency>,
    registry: &[Summary],
    config: Option<&Config>,
) -> CargoResult<Resolve> {
    struct MyRegistry<'a> {
        list: &'a [Summary],
        used: HashSet<PackageId>,
    };
    impl<'a> Registry for MyRegistry<'a> {
        fn query(
            &mut self,
            dep: &Dependency,
            f: &mut dyn FnMut(Summary),
            fuzzy: bool,
        ) -> CargoResult<()> {
            for summary in self.list.iter() {
                if fuzzy || dep.matches(summary) {
                    self.used.insert(summary.package_id());
                    f(summary.clone());
                }
            }
            Ok(())
        }

        fn describe_source(&self, _src: SourceId) -> String {
            String::new()
        }

        fn is_replaced(&self, _src: SourceId) -> bool {
            false
        }
    }
    impl<'a> Drop for MyRegistry<'a> {
        fn drop(&mut self) {
            if std::thread::panicking() && self.list.len() != self.used.len() {
                // we found a case that causes a panic and did not use all of the input.
                // lets print the part of the input that was used for minimization.
                println!(
                    "{:?}",
                    PrettyPrintRegistry(
                        self.list
                            .iter()
                            .filter(|s| { self.used.contains(&s.package_id()) })
                            .cloned()
                            .collect()
                    )
                );
            }
        }
    }
    let mut registry = MyRegistry {
        list: registry,
        used: HashSet::new(),
    };
    let summary = Summary::new(
        pkg,
        deps,
        &BTreeMap::<String, Vec<String>>::new(),
        None::<String>,
        false,
    )
    .unwrap();
    let method = Method::Everything;
    let start = Instant::now();
    let resolve = resolver::resolve(
        &[(summary, method)],
        &[],
        &mut registry,
        &HashSet::new(),
        config,
        true,
    );

    // The largest test in our suite takes less then 30 sec.
    // So lets fail the test if we have ben running for two long.
    assert!(start.elapsed() < slow_cpu_multiplier(60));
    resolve
}

pub trait ToDep {
    fn to_dep(self) -> Dependency;
}

impl ToDep for &'static str {
    fn to_dep(self) -> Dependency {
        Dependency::parse_no_deprecated(self, Some("1.0.0"), registry_loc()).unwrap()
    }
}

impl ToDep for Dependency {
    fn to_dep(self) -> Dependency {
        self
    }
}

pub trait ToPkgId {
    fn to_pkgid(&self) -> PackageId;
}

impl ToPkgId for PackageId {
    fn to_pkgid(&self) -> PackageId {
        *self
    }
}

impl<'a> ToPkgId for &'a str {
    fn to_pkgid(&self) -> PackageId {
        PackageId::new(*self, "1.0.0", registry_loc()).unwrap()
    }
}

impl<T: AsRef<str>, U: AsRef<str>> ToPkgId for (T, U) {
    fn to_pkgid(&self) -> PackageId {
        let (name, vers) = self;
        PackageId::new(name.as_ref(), vers.as_ref(), registry_loc()).unwrap()
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
    lazy_static::lazy_static! {
        static ref EXAMPLE_DOT_COM: SourceId =
            SourceId::for_registry(&"https://example.com".to_url().unwrap()).unwrap();
    }
    *EXAMPLE_DOT_COM
}

pub fn pkg<T: ToPkgId>(name: T) -> Summary {
    pkg_dep(name, Vec::new())
}

pub fn pkg_dep<T: ToPkgId>(name: T, dep: Vec<Dependency>) -> Summary {
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
    )
    .unwrap()
}

pub fn pkg_id(name: &str) -> PackageId {
    PackageId::new(name, "1.0.0", registry_loc()).unwrap()
}

fn pkg_id_loc(name: &str, loc: &str) -> PackageId {
    let remote = loc.to_url();
    let master = GitReference::Branch("master".to_string());
    let source_id = SourceId::for_git(&remote.unwrap(), master).unwrap();

    PackageId::new(name, "1.0.0", source_id).unwrap()
}

pub fn pkg_loc(name: &str, loc: &str) -> Summary {
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
    )
    .unwrap()
}

pub fn remove_dep(sum: &Summary, ind: usize) -> Summary {
    let mut deps = sum.dependencies().to_vec();
    deps.remove(ind);
    // note: more things will need to be copied over in the future, but it works for now.
    Summary::new(
        sum.package_id(),
        deps,
        &BTreeMap::<String, Vec<String>>::new(),
        sum.links().map(|a| a.as_str()),
        sum.namespaced_features(),
    )
    .unwrap()
}

pub fn dep(name: &str) -> Dependency {
    dep_req(name, "*")
}
pub fn dep_req(name: &str, req: &str) -> Dependency {
    Dependency::parse_no_deprecated(name, Some(req), registry_loc()).unwrap()
}
pub fn dep_req_kind(name: &str, req: &str, kind: Kind, public: bool) -> Dependency {
    let mut dep = dep_req(name, req);
    dep.set_kind(kind);
    dep.set_public(public);
    dep
}

pub fn dep_loc(name: &str, location: &str) -> Dependency {
    let url = location.to_url().unwrap();
    let master = GitReference::Branch("master".to_string());
    let source_id = SourceId::for_git(&url, master).unwrap();
    Dependency::parse_no_deprecated(name, Some("1.0.0"), source_id).unwrap()
}
pub fn dep_kind(name: &str, kind: Kind) -> Dependency {
    dep(name).set_kind(kind).clone()
}

pub fn registry(pkgs: Vec<Summary>) -> Vec<Summary> {
    pkgs
}

pub fn names<P: ToPkgId>(names: &[P]) -> Vec<PackageId> {
    names.iter().map(|name| name.to_pkgid()).collect()
}

pub fn loc_names(names: &[(&'static str, &'static str)]) -> Vec<PackageId> {
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
pub struct PrettyPrintRegistry(pub Vec<Summary>);

impl fmt::Debug for PrettyPrintRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "vec![")?;
        for s in &self.0 {
            if s.dependencies().is_empty() {
                write!(f, "pkg!((\"{}\", \"{}\")),", s.name(), s.version())?;
            } else {
                write!(f, "pkg!((\"{}\", \"{}\") => [", s.name(), s.version())?;
                for d in s.dependencies() {
                    if d.kind() == Kind::Normal
                        && &d.version_req().to_string() == "*"
                        && !d.is_public()
                    {
                        write!(f, "dep(\"{}\"),", d.name_in_toml())?;
                    } else if d.kind() == Kind::Normal && !d.is_public() {
                        write!(
                            f,
                            "dep_req(\"{}\", \"{}\"),",
                            d.name_in_toml(),
                            d.version_req()
                        )?;
                    } else {
                        write!(
                            f,
                            "dep_req_kind(\"{}\", \"{}\", {}, {}),",
                            d.name_in_toml(),
                            d.version_req(),
                            match d.kind() {
                                Kind::Development => "Kind::Development",
                                Kind::Build => "Kind::Build",
                                Kind::Normal => "Kind::Normal",
                            },
                            d.is_public()
                        )?;
                    }
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
                pkg!(("foo", "2.0.0") => [dep_req("bar", "*")]),
                pkg!(("bar", "1.0.0") => [dep_req("baz", "=1.0.2"),
                                  dep_req("other", "1")]),
                pkg!(("bar", "2.0.0") => [dep_req("baz", "=1.0.1")]),
                pkg!(("baz", "1.0.2") => [dep_req("other", "2")]),
                pkg!(("baz", "1.0.1")),
                pkg!(("cat", "1.0.2") => [dep_req_kind("other", "2", Kind::Build, false)]),
                pkg!(("cat", "1.0.3") => [dep_req_kind("other", "2", Kind::Development, false)]),
                pkg!(("dep_req", "1.0.0")),
                pkg!(("dep_req", "2.0.0")),
            ])
        ),
        "vec![pkg!((\"foo\", \"1.0.1\") => [dep_req(\"bar\", \"^1\"),]),\
         pkg!((\"foo\", \"1.0.0\") => [dep_req(\"bar\", \"^2\"),]),\
         pkg!((\"foo\", \"2.0.0\") => [dep(\"bar\"),]),\
         pkg!((\"bar\", \"1.0.0\") => [dep_req(\"baz\", \"= 1.0.2\"),dep_req(\"other\", \"^1\"),]),\
         pkg!((\"bar\", \"2.0.0\") => [dep_req(\"baz\", \"= 1.0.1\"),]),\
         pkg!((\"baz\", \"1.0.2\") => [dep_req(\"other\", \"^2\"),]),\
         pkg!((\"baz\", \"1.0.1\")),\
         pkg!((\"cat\", \"1.0.2\") => [dep_req_kind(\"other\", \"^2\", Kind::Build, false),]),\
         pkg!((\"cat\", \"1.0.3\") => [dep_req_kind(\"other\", \"^2\", Kind::Development, false),]),\
         pkg!((\"dep_req\", \"1.0.0\")),\
         pkg!((\"dep_req\", \"2.0.0\")),]"
    )
}

/// This generates a random registry index.
/// Unlike vec((Name, Ver, vec((Name, VerRq), ..), ..)
/// This strategy has a high probability of having valid dependencies
pub fn registry_strategy(
    max_crates: usize,
    max_versions: usize,
    shrinkage: usize,
) -> impl Strategy<Value = PrettyPrintRegistry> {
    let name = string_regex("[A-Za-z][A-Za-z0-9_-]*(-sys)?").unwrap();

    let raw_version = ..max_versions.pow(3);
    let version_from_raw = move |r: usize| {
        let major = ((r / max_versions) / max_versions) % max_versions;
        let minor = (r / max_versions) % max_versions;
        let patch = r % max_versions;
        format!("{}.{}.{}", major, minor, patch)
    };

    // If this is false than the crate will depend on the nonexistent "bad"
    // instead of the complex set we generated for it.
    let allow_deps = prop::bool::weighted(0.99);

    let list_of_versions =
        btree_map(raw_version, allow_deps, 1..=max_versions).prop_map(move |ver| {
            ver.into_iter()
                .map(|a| (version_from_raw(a.0), a.1))
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

    // each version of each crate can depend on each crate smaller then it.
    // In theory shrinkage should be 2, but in practice we get better trees with a larger value.
    let max_deps = max_versions * (max_crates * (max_crates - 1)) / shrinkage;

    let raw_version_range = (any::<Index>(), any::<Index>());
    let raw_dependency = (
        any::<Index>(),
        any::<Index>(),
        raw_version_range,
        0..=1,
        Just(false),
        // TODO: ^ this needs to be set back to `any::<bool>()` and work before public & private dependencies can stabilize
    );

    fn order_index(a: Index, b: Index, size: usize) -> (usize, usize) {
        let (a, b) = (a.index(size), b.index(size));
        (min(a, b), max(a, b))
    }

    let list_of_raw_dependency = vec(raw_dependency, ..=max_deps);

    // By default a package depends only on other packages that have a smaller name,
    // this helps make sure that all things in the resulting index are DAGs.
    // If this is true then the DAG is maintained with grater instead.
    let reverse_alphabetical = any::<bool>().no_shrink();

    (
        list_of_crates_with_versions,
        list_of_raw_dependency,
        reverse_alphabetical,
    )
        .prop_map(
            |(crate_vers_by_name, raw_dependencies, reverse_alphabetical)| {
                let list_of_pkgid: Vec<_> = crate_vers_by_name
                    .iter()
                    .flat_map(|(name, vers)| vers.iter().map(move |x| ((name.as_str(), &x.0), x.1)))
                    .collect();
                let len_all_pkgid = list_of_pkgid.len();
                let mut dependency_by_pkgid = vec![vec![]; len_all_pkgid];
                for (a, b, (c, d), k, p) in raw_dependencies {
                    let (a, b) = order_index(a, b, len_all_pkgid);
                    let (a, b) = if reverse_alphabetical { (b, a) } else { (a, b) };
                    let ((dep_name, _), _) = list_of_pkgid[a];
                    if (list_of_pkgid[b].0).0 == dep_name {
                        continue;
                    }
                    let s = &crate_vers_by_name[dep_name];
                    let s_last_index = s.len() - 1;
                    let (c, d) = order_index(c, d, s.len());

                    dependency_by_pkgid[b].push(dep_req_kind(
                        dep_name,
                        &if c == 0 && d == s_last_index {
                            "*".to_string()
                        } else if c == 0 {
                            format!("<={}", s[d].0)
                        } else if d == s_last_index {
                            format!(">={}", s[c].0)
                        } else if c == d {
                            format!("={}", s[c].0)
                        } else {
                            format!(">={}, <={}", s[c].0, s[d].0)
                        },
                        match k {
                            0 => Kind::Normal,
                            1 => Kind::Build,
                            // => Kind::Development, // Development has no impact so don't gen
                            _ => panic!("bad index for Kind"),
                        },
                        p,
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
                        })
                        .collect(),
                )
            },
        )
}

/// This test is to test the generator to ensure
/// that it makes registries with large dependency trees
#[test]
fn meta_test_deep_trees_from_strategy() {
    let mut dis = [0; 21];

    let strategy = registry_strategy(50, 20, 60);
    let mut test_runner = TestRunner::deterministic();
    for _ in 0..128 {
        let PrettyPrintRegistry(input) = strategy
            .new_tree(&mut TestRunner::new_with_rng(
                Default::default(),
                test_runner.new_rng(),
            ))
            .unwrap()
            .current();
        let reg = registry(input.clone());
        for this in input.iter().rev().take(10) {
            let res = resolve(
                pkg_id("root"),
                vec![dep_req(&this.name(), &format!("={}", this.version()))],
                &reg,
            );
            dis[res
                .as_ref()
                .map(|x| min(x.len(), dis.len()) - 1)
                .unwrap_or(0)] += 1;
            if dis.iter().all(|&x| x > 0) {
                return;
            }
        }
    }

    panic!(
        "In 1280 tries we did not see a wide enough distribution of dependency trees! dis: {:?}",
        dis
    );
}

/// This test is to test the generator to ensure
/// that it makes registries that include multiple versions of the same library
#[test]
fn meta_test_multiple_versions_strategy() {
    let mut dis = [0; 10];

    let strategy = registry_strategy(50, 20, 60);
    let mut test_runner = TestRunner::deterministic();
    for _ in 0..128 {
        let PrettyPrintRegistry(input) = strategy
            .new_tree(&mut TestRunner::new_with_rng(
                Default::default(),
                test_runner.new_rng(),
            ))
            .unwrap()
            .current();
        let reg = registry(input.clone());
        for this in input.iter().rev().take(10) {
            let res = resolve(
                pkg_id("root"),
                vec![dep_req(&this.name(), &format!("={}", this.version()))],
                &reg,
            );
            if let Ok(mut res) = res {
                let res_len = res.len();
                res.sort_by_key(|s| s.name());
                res.dedup_by_key(|s| s.name());
                dis[min(res_len - res.len(), dis.len() - 1)] += 1;
            }
            if dis.iter().all(|&x| x > 0) {
                return;
            }
        }
    }
    panic!(
        "In 1280 tries we did not see a wide enough distribution of multiple versions of the same library! dis: {:?}",
        dis
    );
}

/// Assert `xs` contains `elems`
pub fn assert_contains<A: PartialEq>(xs: &[A], elems: &[A]) {
    for elem in elems {
        assert!(xs.contains(elem));
    }
}

pub fn assert_same<A: PartialEq>(a: &[A], b: &[A]) {
    assert_eq!(a.len(), b.len());
    assert_contains(b, a);
}
