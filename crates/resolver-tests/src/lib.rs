#![allow(clippy::print_stderr)]

use std::cmp::{max, min};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt;
use std::fmt::Write;
use std::sync::OnceLock;
use std::task::Poll;
use std::time::Instant;

use cargo::core::dependency::DepKind;
use cargo::core::resolver::{self, ResolveOpts, VersionOrdering, VersionPreferences};
use cargo::core::FeatureMap;
use cargo::core::ResolveVersion;
use cargo::core::{Dependency, PackageId, Registry, Summary};
use cargo::core::{FeatureValue, Resolve};
use cargo::core::{GitReference, SourceId};
use cargo::sources::source::QueryKind;
use cargo::sources::IndexSummary;
use cargo::util::interning::{InternedString, INTERNED_DEFAULT};
use cargo::util::{CargoResult, GlobalContext, IntoUrl};

use proptest::collection::{btree_map, vec};
use proptest::prelude::*;
use proptest::sample::Index;
use proptest::string::string_regex;
use varisat::ExtendFormula;

pub fn resolve(deps: Vec<Dependency>, registry: &[Summary]) -> CargoResult<Vec<PackageId>> {
    Ok(
        resolve_with_global_context(deps, registry, &GlobalContext::default().unwrap())?
            .into_iter()
            .map(|(pkg, _)| pkg)
            .collect(),
    )
}

// Verify that the resolution of cargo resolver can pass the verification of SAT
pub fn resolve_and_validated(
    deps: Vec<Dependency>,
    registry: &[Summary],
    sat_resolver: &mut SatResolver,
) -> CargoResult<Vec<(PackageId, Vec<InternedString>)>> {
    let resolve =
        resolve_with_global_context_raw(deps.clone(), registry, &GlobalContext::default().unwrap());

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
            let mut stack = vec![pkg_id("root")];
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
    let resolve = resolve_with_global_context_raw(deps, registry, gctx)?;
    Ok(collect_features(&resolve))
}

pub fn resolve_with_global_context_raw(
    deps: Vec<Dependency>,
    registry: &[Summary],
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
                    QueryKind::Alternatives => true,
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
                    "Part used befor drop: {:?}",
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

    let root_summary = Summary::new(
        pkg_id("root"),
        deps,
        &BTreeMap::new(),
        None::<&String>,
        None,
    )
    .unwrap();

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

const fn num_bits<T>() -> usize {
    std::mem::size_of::<T>() * 8
}

fn log_bits(x: usize) -> usize {
    if x == 0 {
        return 0;
    }
    assert!(x > 0);
    (num_bits::<usize>() as u32 - x.leading_zeros()) as usize
}

// At this point is possible to select every version of every package.
// So we need to mark certain versions as incompatible with each other.
// We could add a clause not A, not B for all A and B that are incompatible,
fn sat_at_most_one(solver: &mut varisat::Solver<'_>, vars: &[varisat::Var]) {
    if vars.len() <= 1 {
        return;
    } else if vars.len() == 2 {
        solver.add_clause(&[vars[0].negative(), vars[1].negative()]);
        return;
    } else if vars.len() == 3 {
        solver.add_clause(&[vars[0].negative(), vars[1].negative()]);
        solver.add_clause(&[vars[0].negative(), vars[2].negative()]);
        solver.add_clause(&[vars[1].negative(), vars[2].negative()]);
        return;
    }
    // There are more efficient ways to do it for large numbers of versions.
    //
    // use the "Binary Encoding" from
    // https://www.it.uu.se/research/group/astra/ModRef10/papers/Alan%20M.%20Frisch%20and%20Paul%20A.%20Giannoros.%20SAT%20Encodings%20of%20the%20At-Most-k%20Constraint%20-%20ModRef%202010.pdf
    let bits: Vec<varisat::Var> = solver.new_var_iter(log_bits(vars.len())).collect();
    for (i, p) in vars.iter().enumerate() {
        for b in 0..bits.len() {
            solver.add_clause(&[p.negative(), bits[b].lit(((1 << b) & i) > 0)]);
        }
    }
}

fn sat_at_most_one_by_key<K: std::hash::Hash + Eq>(
    solver: &mut varisat::Solver<'_>,
    data: impl Iterator<Item = (K, varisat::Var)>,
) -> HashMap<K, Vec<varisat::Var>> {
    // no two packages with the same keys set
    let mut by_keys: HashMap<K, Vec<varisat::Var>> = HashMap::new();
    for (p, v) in data {
        by_keys.entry(p).or_default().push(v)
    }
    for key in by_keys.values() {
        sat_at_most_one(solver, key);
    }
    by_keys
}

fn find_compatible_dep_summaries_by_name_in_toml(
    pkg_dependencies: &[Dependency],
    by_name: &HashMap<InternedString, Vec<Summary>>,
) -> HashMap<InternedString, Vec<Summary>> {
    let empty_vec = vec![];

    pkg_dependencies
        .iter()
        .map(|dep| {
            let name_in_toml = dep.name_in_toml();

            let compatible_summaries = by_name
                .get(&dep.package_name())
                .unwrap_or(&empty_vec)
                .iter()
                .filter(|s| dep.matches_id(s.package_id()))
                .filter(|s| dep.features().iter().all(|f| s.features().contains_key(f)))
                .cloned()
                .collect::<Vec<_>>();

            (name_in_toml, compatible_summaries)
        })
        .collect()
}

fn process_pkg_features(
    solver: &mut varisat::Solver<'_>,
    var_for_is_packages_used: &HashMap<PackageId, varisat::Var>,
    var_for_is_packages_features_used: &HashMap<PackageId, HashMap<InternedString, varisat::Var>>,
    pkg_feature_var_map: &HashMap<InternedString, varisat::Var>,
    pkg_features: &FeatureMap,
    compatible_dep_summaries_by_name_in_toml: &HashMap<InternedString, Vec<Summary>>,
) {
    // add clauses for package features
    for (&feature_name, feature_values) in pkg_features {
        for feature_value in feature_values {
            let pkg_feature_var = pkg_feature_var_map[&feature_name];

            match *feature_value {
                FeatureValue::Feature(other_feature_name) => {
                    solver.add_clause(&[
                        pkg_feature_var.negative(),
                        pkg_feature_var_map[&other_feature_name].positive(),
                    ]);
                }
                FeatureValue::Dep { dep_name } => {
                    let dep_clause = compatible_dep_summaries_by_name_in_toml[&dep_name]
                        .iter()
                        .map(|dep| var_for_is_packages_used[&dep.package_id()].positive())
                        .chain([pkg_feature_var.negative()])
                        .collect::<Vec<_>>();

                    solver.add_clause(&dep_clause);
                }
                FeatureValue::DepFeature {
                    dep_name,
                    dep_feature: dep_feature_name,
                    weak,
                } => {
                    for dep in &compatible_dep_summaries_by_name_in_toml[&dep_name] {
                        let dep_var = var_for_is_packages_used[&dep.package_id()];
                        let dep_feature_var =
                            var_for_is_packages_features_used[&dep.package_id()][&dep_feature_name];

                        solver.add_clause(&[
                            pkg_feature_var.negative(),
                            dep_var.negative(),
                            dep_feature_var.positive(),
                        ]);
                    }

                    if !weak {
                        let dep_clause = compatible_dep_summaries_by_name_in_toml[&dep_name]
                            .iter()
                            .map(|dep| var_for_is_packages_used[&dep.package_id()].positive())
                            .chain([pkg_feature_var.negative()])
                            .collect::<Vec<_>>();

                        solver.add_clause(&dep_clause);
                    }
                }
            }
        }
    }
}

fn process_pkg_dependencies(
    solver: &mut varisat::Solver<'_>,
    var_for_is_packages_used: &HashMap<PackageId, varisat::Var>,
    var_for_is_packages_features_used: &HashMap<PackageId, HashMap<InternedString, varisat::Var>>,
    pkg_var: varisat::Var,
    pkg_dependencies: &[Dependency],
    compatible_dep_summaries_by_name_in_toml: &HashMap<InternedString, Vec<Summary>>,
) {
    for dep in pkg_dependencies {
        let compatible_dep_summaries =
            &compatible_dep_summaries_by_name_in_toml[&dep.name_in_toml()];

        // add clauses for package dependency features
        for dep_summary in compatible_dep_summaries {
            let dep_package_id = dep_summary.package_id();

            let default_feature = if dep.uses_default_features()
                && dep_summary.features().contains_key(&*INTERNED_DEFAULT)
            {
                Some(&INTERNED_DEFAULT)
            } else {
                None
            };

            for &feature_name in default_feature.into_iter().chain(dep.features()) {
                solver.add_clause(&[
                    pkg_var.negative(),
                    var_for_is_packages_used[&dep_package_id].negative(),
                    var_for_is_packages_features_used[&dep_package_id][&feature_name].positive(),
                ]);
            }
        }

        // active packages need to activate each of their non-optional dependencies
        if !dep.is_optional() {
            let dep_clause = compatible_dep_summaries
                .iter()
                .map(|d| var_for_is_packages_used[&d.package_id()].positive())
                .chain([pkg_var.negative()])
                .collect::<Vec<_>>();

            solver.add_clause(&dep_clause);
        }
    }
}

/// Resolution can be reduced to the SAT problem. So this is an alternative implementation
/// of the resolver that uses a SAT library for the hard work. This is intended to be easy to read,
/// as compared to the real resolver.
///
/// For the subset of functionality that are currently made by `registry_strategy`,
/// this will find a valid resolution if one exists.
///
/// The SAT library does not optimize for the newer version,
/// so the selected packages may not match the real resolver.
pub struct SatResolver {
    solver: varisat::Solver<'static>,
    old_root_vars: Vec<varisat::Var>,
    var_for_is_packages_used: HashMap<PackageId, varisat::Var>,
    var_for_is_packages_features_used: HashMap<PackageId, HashMap<InternedString, varisat::Var>>,
    by_name: HashMap<InternedString, Vec<Summary>>,
}

impl SatResolver {
    pub fn new(registry: &[Summary]) -> Self {
        let mut solver = varisat::Solver::new();

        // That represents each package version which is set to "true" if the packages in the lock file and "false" if it is unused.
        let var_for_is_packages_used = registry
            .iter()
            .map(|s| (s.package_id(), solver.new_var()))
            .collect::<HashMap<_, _>>();

        // That represents each feature of each package version, which is set to "true" if the package feature is used.
        let var_for_is_packages_features_used = registry
            .iter()
            .map(|s| {
                (
                    s.package_id(),
                    (s.features().keys().map(|&f| (f, solver.new_var()))).collect(),
                )
            })
            .collect::<HashMap<_, HashMap<_, _>>>();

        // if a package feature is used, then the package is used
        for (package, pkg_feature_var_map) in &var_for_is_packages_features_used {
            for (_, package_feature_var) in pkg_feature_var_map {
                let package_var = var_for_is_packages_used[package];
                solver.add_clause(&[package_feature_var.negative(), package_var.positive()]);
            }
        }

        // no two packages with the same links set
        sat_at_most_one_by_key(
            &mut solver,
            registry
                .iter()
                .map(|s| (s.links(), var_for_is_packages_used[&s.package_id()]))
                .filter(|(l, _)| l.is_some()),
        );

        // no two semver compatible versions of the same package
        sat_at_most_one_by_key(
            &mut solver,
            var_for_is_packages_used
                .iter()
                .map(|(p, &v)| (p.as_activations_key(), v)),
        );

        let mut by_name: HashMap<InternedString, Vec<Summary>> = HashMap::new();

        for p in registry {
            by_name.entry(p.name()).or_default().push(p.clone())
        }

        for pkg in registry {
            let pkg_id = pkg.package_id();
            let pkg_dependencies = pkg.dependencies();
            let pkg_features = pkg.features();

            let compatible_dep_summaries_by_name_in_toml =
                find_compatible_dep_summaries_by_name_in_toml(pkg_dependencies, &by_name);

            process_pkg_features(
                &mut solver,
                &var_for_is_packages_used,
                &var_for_is_packages_features_used,
                &var_for_is_packages_features_used[&pkg_id],
                pkg_features,
                &compatible_dep_summaries_by_name_in_toml,
            );

            process_pkg_dependencies(
                &mut solver,
                &var_for_is_packages_used,
                &var_for_is_packages_features_used,
                var_for_is_packages_used[&pkg_id],
                pkg_dependencies,
                &compatible_dep_summaries_by_name_in_toml,
            );
        }

        // We don't need to `solve` now. We know that "use nothing" will satisfy all the clauses so far.
        // But things run faster if we let it spend some time figuring out how the constraints interact before we add assumptions.
        solver
            .solve()
            .expect("docs say it can't error in default config");

        SatResolver {
            solver,
            old_root_vars: Vec::new(),
            var_for_is_packages_used,
            var_for_is_packages_features_used,
            by_name,
        }
    }

    pub fn sat_resolve(&mut self, root_dependencies: &[Dependency]) -> bool {
        let SatResolver {
            solver,
            old_root_vars,
            var_for_is_packages_used,
            var_for_is_packages_features_used,
            by_name,
        } = self;

        let root_var = solver.new_var();

        // root package is always used
        // root vars from previous runs are deactivated
        let assumption = old_root_vars
            .iter()
            .map(|v| v.negative())
            .chain([root_var.positive()])
            .collect::<Vec<_>>();

        old_root_vars.push(root_var);

        let compatible_dep_summaries_by_name_in_toml =
            find_compatible_dep_summaries_by_name_in_toml(root_dependencies, &by_name);

        process_pkg_dependencies(
            solver,
            var_for_is_packages_used,
            var_for_is_packages_features_used,
            root_var,
            root_dependencies,
            &compatible_dep_summaries_by_name_in_toml,
        );

        solver.assume(&assumption);

        solver
            .solve()
            .expect("docs say it can't error in default config")
    }

    pub fn sat_is_valid_solution(&mut self, pkgs: &[(PackageId, Vec<InternedString>)]) -> bool {
        let contains_pkg = |pkg| pkgs.iter().any(|(p, _)| p == pkg);
        let contains_pkg_feature =
            |pkg, f| pkgs.iter().any(|(p, flist)| p == pkg && flist.contains(f));

        for (p, _) in pkgs {
            if p.name() != "root" && !self.var_for_is_packages_used.contains_key(p) {
                return false;
            }
        }

        // root vars from previous runs are deactivated
        let assumption = (self.old_root_vars.iter().map(|v| v.negative()))
            .chain(
                self.var_for_is_packages_used
                    .iter()
                    .map(|(p, v)| v.lit(contains_pkg(p))),
            )
            .chain(
                self.var_for_is_packages_features_used
                    .iter()
                    .flat_map(|(p, fmap)| {
                        fmap.iter()
                            .map(move |(f, v)| v.lit(contains_pkg_feature(p, f)))
                    }),
            )
            .collect::<Vec<_>>();

        self.solver.assume(&assumption);

        self.solver
            .solve()
            .expect("docs say it can't error in default config")
    }

    fn used_packages(&self) -> Option<String> {
        self.solver.model().map(|lits| {
            let lits: HashSet<_> = lits
                .iter()
                .filter(|l| l.is_positive())
                .map(|l| l.var())
                .collect();

            let mut used_packages = BTreeMap::<PackageId, BTreeSet<InternedString>>::new();
            for (&p, v) in self.var_for_is_packages_used.iter() {
                if lits.contains(v) {
                    used_packages.entry(p).or_default();
                }
            }
            for (&p, map) in &self.var_for_is_packages_features_used {
                for (&f, v) in map {
                    if lits.contains(v) {
                        used_packages
                            .get_mut(&p)
                            .expect("the feature is activated without the package being activated")
                            .insert(f);
                    }
                }
            }

            let mut out = String::from("used:\n");
            for (package, feature_names) in used_packages {
                writeln!(&mut out, "  {package}").unwrap();
                for feature_name in feature_names {
                    writeln!(&mut out, "    + {feature_name}").unwrap();
                }
            }

            out
        })
    }
}

pub trait ToDep {
    fn to_dep(self) -> Dependency;
    fn to_opt_dep(self) -> Dependency;
    fn to_dep_with(self, features: &[&'static str]) -> Dependency;
}

impl ToDep for &'static str {
    fn to_dep(self) -> Dependency {
        Dependency::parse(self, Some("1.0.0"), registry_loc()).unwrap()
    }
    fn to_opt_dep(self) -> Dependency {
        let mut dep = self.to_dep();
        dep.set_optional(true);
        dep
    }
    fn to_dep_with(self, features: &[&'static str]) -> Dependency {
        let mut dep = self.to_dep();
        dep.set_default_features(false);
        dep.set_features(features.into_iter().copied());
        dep
    }
}

impl ToDep for Dependency {
    fn to_dep(self) -> Dependency {
        self
    }
    fn to_opt_dep(mut self) -> Dependency {
        self.set_optional(true);
        self
    }
    fn to_dep_with(mut self, features: &[&'static str]) -> Dependency {
        self.set_default_features(false);
        self.set_features(features.into_iter().copied());
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
        PackageId::try_new(*self, "1.0.0", registry_loc()).unwrap()
    }
}

impl<T: AsRef<str>, U: AsRef<str>> ToPkgId for (T, U) {
    fn to_pkgid(&self) -> PackageId {
        let (name, vers) = self;
        PackageId::try_new(name.as_ref(), vers.as_ref(), registry_loc()).unwrap()
    }
}

#[macro_export]
macro_rules! pkg {
    ($pkgid:expr => [$($deps:expr),* $(,)? ]) => ({
        let d: Vec<Dependency> = vec![$($deps.to_dep()),*];
        $crate::pkg_dep($pkgid, d)
    });

    ($pkgid:expr) => ({
        $crate::pkg($pkgid)
    })
}

fn registry_loc() -> SourceId {
    static EXAMPLE_DOT_COM: OnceLock<SourceId> = OnceLock::new();
    let example_dot = EXAMPLE_DOT_COM.get_or_init(|| {
        SourceId::for_registry(&"https://example.com".into_url().unwrap()).unwrap()
    });
    *example_dot
}

pub fn pkg<T: ToPkgId>(name: T) -> Summary {
    pkg_dep(name, Vec::new())
}

pub fn pkg_dep<T: ToPkgId>(name: T, dep: Vec<Dependency>) -> Summary {
    let pkgid = name.to_pkgid();
    let link = if pkgid.name().ends_with("-sys") {
        Some(pkgid.name())
    } else {
        None
    };
    Summary::new(name.to_pkgid(), dep, &BTreeMap::new(), link, None).unwrap()
}

pub fn pkg_dep_with<T: ToPkgId>(
    name: T,
    dep: Vec<Dependency>,
    features: &[(&'static str, &[&'static str])],
) -> Summary {
    let pkgid = name.to_pkgid();
    let link = if pkgid.name().ends_with("-sys") {
        Some(pkgid.name())
    } else {
        None
    };
    let features = features
        .into_iter()
        .map(|&(name, values)| (name.into(), values.into_iter().map(|&v| v.into()).collect()))
        .collect();
    Summary::new(name.to_pkgid(), dep, &features, link, None).unwrap()
}

pub fn pkg_id(name: &str) -> PackageId {
    PackageId::try_new(name, "1.0.0", registry_loc()).unwrap()
}

fn pkg_id_loc(name: &str, loc: &str) -> PackageId {
    let remote = loc.into_url();
    let master = GitReference::Branch("master".to_string());
    let source_id = SourceId::for_git(&remote.unwrap(), master).unwrap();

    PackageId::try_new(name, "1.0.0", source_id).unwrap()
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
        &BTreeMap::new(),
        link,
        None,
    )
    .unwrap()
}

pub fn remove_dep(sum: &Summary, ind: usize) -> Summary {
    let mut deps = sum.dependencies().to_vec();
    deps.remove(ind);
    // note: more things will need to be copied over in the future, but it works for now.
    Summary::new(sum.package_id(), deps, &BTreeMap::new(), sum.links(), None).unwrap()
}

pub fn dep(name: &str) -> Dependency {
    dep_req(name, "*")
}

pub fn dep_req(name: &str, req: &str) -> Dependency {
    Dependency::parse(name, Some(req), registry_loc()).unwrap()
}

pub fn dep_req_kind(name: &str, req: &str, kind: DepKind) -> Dependency {
    let mut dep = dep_req(name, req);
    dep.set_kind(kind);
    dep
}

pub fn dep_loc(name: &str, location: &str) -> Dependency {
    let url = location.into_url().unwrap();
    let master = GitReference::Branch("master".to_string());
    let source_id = SourceId::for_git(&url, master).unwrap();
    Dependency::parse(name, Some("1.0.0"), source_id).unwrap()
}

pub fn dep_kind(name: &str, kind: DepKind) -> Dependency {
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
        "In 1280 tries we did not see a wide enough distribution of dependency trees! dis: {:?}",
        dis
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
        "In 1280 tries we did not see a wide enough distribution of multiple versions of the same library! dis: {:?}",
        dis
    );
}

/// Assert `xs` contains `elems`
#[track_caller]
pub fn assert_contains<A: PartialEq + std::fmt::Debug>(xs: &[A], elems: &[A]) {
    for elem in elems {
        assert!(
            xs.contains(elem),
            "missing element\nset: {xs:?}\nmissing: {elem:?}"
        );
    }
}

#[track_caller]
pub fn assert_same<A: PartialEq + std::fmt::Debug>(a: &[A], b: &[A]) {
    assert_eq!(a.len(), b.len(), "not equal\n{a:?}\n{b:?}");
    assert_contains(b, a);
}
