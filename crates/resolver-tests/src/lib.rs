//! > This crate is maintained by the Cargo team, primarily for use by Cargo
//! > and not intended for external use (except as a transitive dependency). This
//! > crate may make major changes to its APIs or be deprecated without warning.

#![allow(clippy::print_stderr)]

pub mod helpers;
pub mod sat;

use std::cmp::{max, min};
use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::task::Poll;
use std::time::Instant;

use cargo::core::Resolve;
use cargo::core::ResolveVersion;
use cargo::core::SourceId;
use cargo::core::dependency::DepKind;
use cargo::core::resolver::{self, ResolveOpts, VersionOrdering, VersionPreferences};
use cargo::core::{Dependency, PackageId, Registry, Summary};
use cargo::sources::IndexSummary;
use cargo::sources::source::QueryKind;
use cargo::util::interning::InternedString;
use cargo::util::{CargoResult, GlobalContext};

use crate::helpers::{ToPkgId, dep_req, dep_req_kind, pkg_dep, pkg_id};
use crate::sat::SatResolver;

use proptest::collection::{btree_map, vec};
use proptest::prelude::*;
use proptest::sample::Index;
use proptest::string::string_regex;

pub fn resolve(deps: Vec<Dependency>, registry: &[Summary]) -> CargoResult<Vec<PackageId>> {
    Ok(
        resolve_with_global_context(deps, registry, &GlobalContext::default().unwrap())?
            .into_iter()
            .map(|(pkg, _)| pkg)
            .collect(),
    )
}

pub fn resolve_and_validated(
    deps: Vec<Dependency>,
    registry: &[Summary],
    sat_resolver: &mut SatResolver,
) -> CargoResult<Vec<(PackageId, Vec<InternedString>)>> {
    resolve_and_validated_raw(deps, registry, pkg_id("root"), sat_resolver)
}

// Verify that the resolution of cargo resolver can pass the verification of SAT
pub fn resolve_and_validated_raw(
    deps: Vec<Dependency>,
    registry: &[Summary],
    root_pkg_id: PackageId,
    sat_resolver: &mut SatResolver,
) -> CargoResult<Vec<(PackageId, Vec<InternedString>)>> {
    let resolve = resolve_with_global_context_raw(
        deps.clone(),
        registry,
        root_pkg_id,
        &GlobalContext::default().unwrap(),
    );

    match resolve {
        Err(e) => {
            if sat_resolver.sat_resolve(&deps) {
                panic!(
                    "`resolve()` returned an error but the sat resolver thinks this will work:\n{}",
                    sat_resolver.used_packages().unwrap()
                );
            }
            Err(e)
        }
        Ok(resolve) => {
            let mut stack = vec![root_pkg_id];
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
            let out = collect_features(&resolve);
            assert_eq!(out.len(), used.len());

            if !sat_resolver.sat_is_valid_solution(&out) {
                panic!(
                    "`resolve()` thinks this will work, but the solution is \
                     invalid according to the sat resolver:\n{resolve:?}",
                );
            }
            Ok(out)
        }
    }
}

fn collect_features(resolve: &Resolve) -> Vec<(PackageId, Vec<InternedString>)> {
    resolve
        .sort()
        .iter()
        .map(|&pkg| (pkg, resolve.features(pkg).to_vec()))
        .collect()
}

pub fn resolve_with_global_context(
    deps: Vec<Dependency>,
    registry: &[Summary],
    gctx: &GlobalContext,
) -> CargoResult<Vec<(PackageId, Vec<InternedString>)>> {
    let resolve = resolve_with_global_context_raw(deps, registry, pkg_id("root"), gctx)?;
    Ok(collect_features(&resolve))
}

pub fn resolve_with_global_context_raw(
    deps: Vec<Dependency>,
    registry: &[Summary],
    root_pkg_id: PackageId,
    gctx: &GlobalContext,
) -> CargoResult<Resolve> {
    struct MyRegistry<'a> {
        list: &'a [Summary],
        used: HashSet<PackageId>,
    }
    impl<'a> Registry for MyRegistry<'a> {
        fn query(
            &mut self,
            dep: &Dependency,
            kind: QueryKind,
            f: &mut dyn FnMut(IndexSummary),
        ) -> Poll<CargoResult<()>> {
            for summary in self.list.iter() {
                let matched = match kind {
                    QueryKind::Exact => dep.matches(summary),
                    QueryKind::RejectedVersions => dep.matches(summary),
                    QueryKind::AlternativeNames => true,
                    QueryKind::Normalized => true,
                };
                if matched {
                    self.used.insert(summary.package_id());
                    f(IndexSummary::Candidate(summary.clone()));
                }
            }
            Poll::Ready(Ok(()))
        }

        fn describe_source(&self, _src: SourceId) -> String {
            String::new()
        }

        fn is_replaced(&self, _src: SourceId) -> bool {
            false
        }

        fn block_until_ready(&mut self) -> CargoResult<()> {
            Ok(())
        }
    }
    impl<'a> Drop for MyRegistry<'a> {
        fn drop(&mut self) {
            if std::thread::panicking() && self.list.len() != self.used.len() {
                // we found a case that causes a panic and did not use all of the input.
                // lets print the part of the input that was used for minimization.
                eprintln!(
                    "Part used before drop: {:?}",
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

    let root_summary =
        Summary::new(root_pkg_id, deps, &BTreeMap::new(), None::<&String>, None).unwrap();

    let opts = ResolveOpts::everything();

    let start = Instant::now();
    let mut version_prefs = VersionPreferences::default();
    if gctx.cli_unstable().minimal_versions {
        version_prefs.version_ordering(VersionOrdering::MinimumVersionsFirst)
    }

    let resolve = resolver::resolve(
        &[(root_summary, opts)],
        &[],
        &mut registry,
        &version_prefs,
        ResolveVersion::with_rust_version(None),
        Some(gctx),
    );

    // The largest test in our suite takes less then 30 secs.
    // So let's fail the test if we have been running for more than 60 secs.
    assert!(start.elapsed().as_secs() < 60);
    resolve
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
                    if d.kind() == DepKind::Normal
                        && &d.version_req().to_string() == "*"
                        && !d.is_public()
                    {
                        write!(f, "dep(\"{}\"),", d.name_in_toml())?;
                    } else if d.kind() == DepKind::Normal && !d.is_public() {
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
                                DepKind::Development => "DepKind::Development",
                                DepKind::Build => "DepKind::Build",
                                DepKind::Normal => "DepKind::Normal",
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

/// This generates a random registry index.
/// Unlike `vec((Name, Ver, vec((Name, VerRq), ..), ..)`,
/// this strategy has a high probability of having valid dependencies.
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

    // If this is false then the crate will depend on the nonexistent "bad"
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

    // each version of each crate can depend on each crate smaller than it.
    // In theory shrinkage should be 2, but in practice we get better trees with a larger value.
    let max_deps = max_versions * (max_crates * (max_crates - 1)) / shrinkage;

    let raw_version_range = (any::<Index>(), any::<Index>());
    let raw_dependency = (any::<Index>(), any::<Index>(), raw_version_range, 0..=1);

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
                for (a, b, (c, d), k) in raw_dependencies {
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
                            0 => DepKind::Normal,
                            1 => DepKind::Build,
                            // => DepKind::Development, // Development has no impact so don't gen
                            _ => panic!("bad index for DepKind"),
                        },
                    ))
                }

                let mut out: Vec<Summary> = list_of_pkgid
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
                    .collect();

                if reverse_alphabetical {
                    // make sure the complicated cases are at the end
                    out.reverse();
                }

                PrettyPrintRegistry(out)
            },
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helpers::registry;

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
                    pkg!(("cat", "1.0.2") => [dep_req_kind("other", "2", DepKind::Build)]),
                    pkg!(("cat", "1.0.3") => [dep_req_kind("other", "2", DepKind::Development)]),
                    pkg!(("dep_req", "1.0.0")),
                    pkg!(("dep_req", "2.0.0")),
                ])
            ),
            "vec![pkg!((\"foo\", \"1.0.1\") => [dep_req(\"bar\", \"^1\"),]),\
         pkg!((\"foo\", \"1.0.0\") => [dep_req(\"bar\", \"^2\"),]),\
         pkg!((\"foo\", \"2.0.0\") => [dep(\"bar\"),]),\
         pkg!((\"bar\", \"1.0.0\") => [dep_req(\"baz\", \"=1.0.2\"),dep_req(\"other\", \"^1\"),]),\
         pkg!((\"bar\", \"2.0.0\") => [dep_req(\"baz\", \"=1.0.1\"),]),\
         pkg!((\"baz\", \"1.0.2\") => [dep_req(\"other\", \"^2\"),]),\
         pkg!((\"baz\", \"1.0.1\")),\
         pkg!((\"cat\", \"1.0.2\") => [dep_req_kind(\"other\", \"^2\", DepKind::Build, false),]),\
         pkg!((\"cat\", \"1.0.3\") => [dep_req_kind(\"other\", \"^2\", DepKind::Development, false),]),\
         pkg!((\"dep_req\", \"1.0.0\")),\
         pkg!((\"dep_req\", \"2.0.0\")),]"
        )
    }

    /// This test is to test the generator to ensure
    /// that it makes registries with large dependency trees
    #[test]
    fn meta_test_deep_trees_from_strategy() {
        use proptest::strategy::ValueTree;
        use proptest::test_runner::TestRunner;

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
            "In 1280 tries we did not see a wide enough distribution \
             of dependency trees! dis: {dis:?}"
        );
    }

    /// This test is to test the generator to ensure
    /// that it makes registries that include multiple versions of the same library
    #[test]
    fn meta_test_multiple_versions_strategy() {
        use proptest::strategy::ValueTree;
        use proptest::test_runner::TestRunner;

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
            "In 1280 tries we did not see a wide enough distribution \
             of multiple versions of the same library! dis: {dis:?}"
        );
    }
}
