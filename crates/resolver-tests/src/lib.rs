#![allow(clippy::all)]

use std::cell::RefCell;
use std::cmp::PartialEq;
use std::cmp::{max, min};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt;
use std::fmt::Write;
use std::rc::Rc;
use std::sync::OnceLock;
use std::task::Poll;
use std::time::Instant;

use cargo::core::dependency::DepKind;
use cargo::core::resolver::{self, ResolveOpts, VersionPreferences};
use cargo::core::Resolve;
use cargo::core::{Dependency, PackageId, Registry, Summary};
use cargo::core::{GitReference, SourceId};
use cargo::sources::source::QueryKind;
use cargo::util::{CargoResult, Config, Graph, IntoUrl, RustVersion};

use proptest::collection::{btree_map, vec};
use proptest::prelude::*;
use proptest::sample::Index;
use proptest::string::string_regex;
use varisat::{self, ExtendFormula};

pub fn resolve(deps: Vec<Dependency>, registry: &[Summary]) -> CargoResult<Vec<PackageId>> {
    resolve_with_config(deps, registry, &Config::default().unwrap())
}

pub fn resolve_and_validated(
    deps: Vec<Dependency>,
    registry: &[Summary],
    sat_resolve: Option<SatResolve>,
) -> CargoResult<Vec<PackageId>> {
    let resolve = resolve_with_config_raw(deps.clone(), registry, &Config::default().unwrap());

    match resolve {
        Err(e) => {
            let sat_resolve = sat_resolve.unwrap_or_else(|| SatResolve::new(registry));
            if sat_resolve.sat_resolve(&deps) {
                panic!(
                    "the resolve err but the sat_resolve thinks this will work:\n{}",
                    sat_resolve.use_packages().unwrap()
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
            let out = resolve.sort();
            assert_eq!(out.len(), used.len());

            let mut pub_deps: HashMap<PackageId, HashSet<_>> = HashMap::new();
            for &p in out.iter() {
                // make the list of `p` public dependencies
                let mut self_pub_dep = HashSet::new();
                self_pub_dep.insert(p);
                for (dp, deps) in resolve.deps(p) {
                    if deps.iter().any(|d| d.is_public()) {
                        self_pub_dep.extend(pub_deps[&dp].iter().cloned())
                    }
                }
                pub_deps.insert(p, self_pub_dep);

                // check if `p` has a public dependencies conflicts
                let seen_dep: BTreeSet<_> = resolve
                    .deps(p)
                    .flat_map(|(dp, _)| pub_deps[&dp].iter().cloned())
                    .collect();
                let seen_dep: Vec<_> = seen_dep.iter().collect();
                for a in seen_dep.windows(2) {
                    if a[0].name() == a[1].name() {
                        panic!(
                            "the package {:?} can publicly see {:?} and {:?}",
                            p, a[0], a[1]
                        )
                    }
                }
            }
            let sat_resolve = sat_resolve.unwrap_or_else(|| SatResolve::new(registry));
            if !sat_resolve.sat_is_valid_solution(&out) {
                panic!(
                    "the sat_resolve err but the resolve thinks this will work:\n{:?}",
                    resolve
                );
            }
            Ok(out)
        }
    }
}

pub fn resolve_with_config(
    deps: Vec<Dependency>,
    registry: &[Summary],
    config: &Config,
) -> CargoResult<Vec<PackageId>> {
    let resolve = resolve_with_config_raw(deps, registry, config)?;
    Ok(resolve.sort())
}

pub fn resolve_with_config_raw(
    deps: Vec<Dependency>,
    registry: &[Summary],
    config: &Config,
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
            f: &mut dyn FnMut(Summary),
        ) -> Poll<CargoResult<()>> {
            for summary in self.list.iter() {
                let matched = match kind {
                    QueryKind::Exact => dep.matches(summary),
                    QueryKind::Fuzzy => true,
                };
                if matched {
                    self.used.insert(summary.package_id());
                    f(summary.clone());
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
    let summary = Summary::new(
        pkg_id("root"),
        deps,
        &BTreeMap::new(),
        None::<&String>,
        None::<RustVersion>,
    )
    .unwrap();
    let opts = ResolveOpts::everything();
    let start = Instant::now();
    let max_rust_version = None;
    let resolve = resolver::resolve(
        &[(summary, opts)],
        &[],
        &mut registry,
        &VersionPreferences::default(),
        Some(config),
        true,
        max_rust_version,
    );

    // The largest test in our suite takes less then 30 sec.
    // So lets fail the test if we have ben running for two long.
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

fn sat_at_most_one(solver: &mut impl varisat::ExtendFormula, vars: &[varisat::Var]) {
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
    cnf: &mut impl varisat::ExtendFormula,
    data: impl Iterator<Item = (K, varisat::Var)>,
) -> HashMap<K, Vec<varisat::Var>> {
    // no two packages with the same links set
    let mut by_keys: HashMap<K, Vec<varisat::Var>> = HashMap::new();
    for (p, v) in data {
        by_keys.entry(p).or_default().push(v)
    }
    for key in by_keys.values() {
        sat_at_most_one(cnf, key);
    }
    by_keys
}

/// Resolution can be reduced to the SAT problem. So this is an alternative implementation
/// of the resolver that uses a SAT library for the hard work. This is intended to be easy to read,
/// as compared to the real resolver.
///
/// For the subset of functionality that are currently made by `registry_strategy` this will,
/// find a valid resolution if one exists. The big thing that the real resolver does,
/// that this one does not do is work with features and optional dependencies.
///
/// The SAT library dose not optimize for the newer version,
/// so the selected packages may not match the real resolver.
#[derive(Clone)]
pub struct SatResolve(Rc<RefCell<SatResolveInner>>);
struct SatResolveInner {
    solver: varisat::Solver<'static>,
    var_for_is_packages_used: HashMap<PackageId, varisat::Var>,
    by_name: HashMap<&'static str, Vec<PackageId>>,
}

impl SatResolve {
    pub fn new(registry: &[Summary]) -> Self {
        let mut cnf = varisat::CnfFormula::new();
        let var_for_is_packages_used: HashMap<PackageId, varisat::Var> = registry
            .iter()
            .map(|s| (s.package_id(), cnf.new_var()))
            .collect();

        // no two packages with the same links set
        sat_at_most_one_by_key(
            &mut cnf,
            registry
                .iter()
                .map(|s| (s.links(), var_for_is_packages_used[&s.package_id()]))
                .filter(|(l, _)| l.is_some()),
        );

        // no two semver compatible versions of the same package
        let by_activations_keys = sat_at_most_one_by_key(
            &mut cnf,
            var_for_is_packages_used
                .iter()
                .map(|(p, &v)| (p.as_activations_key(), v)),
        );

        let mut by_name: HashMap<&'static str, Vec<PackageId>> = HashMap::new();

        for p in registry.iter() {
            by_name
                .entry(p.name().as_str())
                .or_default()
                .push(p.package_id())
        }

        let empty_vec = vec![];

        let mut graph: Graph<PackageId, ()> = Graph::new();

        let mut version_selected_for: HashMap<
            PackageId,
            HashMap<Dependency, HashMap<_, varisat::Var>>,
        > = HashMap::new();
        // active packages need each of there `deps` to be satisfied
        for p in registry.iter() {
            graph.add(p.package_id());
            for dep in p.dependencies() {
                // This can more easily be written as:
                // !is_active(p) or one of the things that match dep is_active
                // All the complexity, from here to the end, is to support public and private dependencies!
                let mut by_key: HashMap<_, Vec<varisat::Lit>> = HashMap::new();
                for &m in by_name
                    .get(dep.package_name().as_str())
                    .unwrap_or(&empty_vec)
                    .iter()
                    .filter(|&p| dep.matches_id(*p))
                {
                    graph.link(p.package_id(), m);
                    by_key
                        .entry(m.as_activations_key())
                        .or_default()
                        .push(var_for_is_packages_used[&m].positive());
                }
                let keys: HashMap<_, _> = by_key.keys().map(|&k| (k, cnf.new_var())).collect();

                // if `p` is active then we need to select one of the keys
                let matches: Vec<_> = keys
                    .values()
                    .map(|v| v.positive())
                    .chain(Some(var_for_is_packages_used[&p.package_id()].negative()))
                    .collect();
                cnf.add_clause(&matches);

                // if a key is active then we need to select one of the versions
                for (key, vars) in by_key.iter() {
                    let mut matches = vars.clone();
                    matches.push(keys[key].negative());
                    cnf.add_clause(&matches);
                }

                version_selected_for
                    .entry(p.package_id())
                    .or_default()
                    .insert(dep.clone(), keys);
            }
        }

        let topological_order = graph.sort();

        // we already ensure there is only one version for each `activations_key` so we can think of
        // `publicly_exports` as being in terms of a set of `activations_key`s
        let mut publicly_exports: HashMap<_, HashMap<_, varisat::Var>> = HashMap::new();

        for &key in by_activations_keys.keys() {
            // everything publicly depends on itself
            let var = publicly_exports
                .entry(key)
                .or_default()
                .entry(key)
                .or_insert_with(|| cnf.new_var());
            cnf.add_clause(&[var.positive()]);
        }

        // if a `dep` is public then `p` `publicly_exports` all the things that the selected version `publicly_exports`
        for &p in topological_order.iter() {
            if let Some(deps) = version_selected_for.get(&p) {
                let mut p_exports = publicly_exports.remove(&p.as_activations_key()).unwrap();
                for (_, versions) in deps.iter().filter(|(d, _)| d.is_public()) {
                    for (ver, sel) in versions {
                        for (&export_pid, &export_var) in publicly_exports[ver].iter() {
                            let our_var =
                                p_exports.entry(export_pid).or_insert_with(|| cnf.new_var());
                            cnf.add_clause(&[
                                sel.negative(),
                                export_var.negative(),
                                our_var.positive(),
                            ]);
                        }
                    }
                }
                publicly_exports.insert(p.as_activations_key(), p_exports);
            }
        }

        // we already ensure there is only one version for each `activations_key` so we can think of
        // `can_see` as being in terms of a set of `activations_key`s
        // and if `p` `publicly_exports` `export` then it `can_see` `export`
        let mut can_see: HashMap<_, HashMap<_, varisat::Var>> = HashMap::new();

        // if `p` has a `dep` that selected `ver` then it `can_see` all the things that the selected version `publicly_exports`
        for (&p, deps) in version_selected_for.iter() {
            let p_can_see = can_see.entry(p).or_default();
            for (_, versions) in deps.iter() {
                for (&ver, sel) in versions {
                    for (&export_pid, &export_var) in publicly_exports[&ver].iter() {
                        let our_var = p_can_see.entry(export_pid).or_insert_with(|| cnf.new_var());
                        cnf.add_clause(&[
                            sel.negative(),
                            export_var.negative(),
                            our_var.positive(),
                        ]);
                    }
                }
            }
        }

        // a package `can_see` only one version by each name
        for (_, see) in can_see.iter() {
            sat_at_most_one_by_key(&mut cnf, see.iter().map(|((name, _, _), &v)| (name, v)));
        }
        let mut solver = varisat::Solver::new();
        solver.add_formula(&cnf);

        // We dont need to `solve` now. We know that "use nothing" will satisfy all the clauses so far.
        // But things run faster if we let it spend some time figuring out how the constraints interact before we add assumptions.
        solver
            .solve()
            .expect("docs say it can't error in default config");
        SatResolve(Rc::new(RefCell::new(SatResolveInner {
            solver,
            var_for_is_packages_used,
            by_name,
        })))
    }
    pub fn sat_resolve(&self, deps: &[Dependency]) -> bool {
        let mut s = self.0.borrow_mut();
        let mut assumption = vec![];
        let mut this_call = None;

        // the starting `deps` need to be satisfied
        for dep in deps.iter() {
            let empty_vec = vec![];
            let matches: Vec<varisat::Lit> = s
                .by_name
                .get(dep.package_name().as_str())
                .unwrap_or(&empty_vec)
                .iter()
                .filter(|&p| dep.matches_id(*p))
                .map(|p| s.var_for_is_packages_used[p].positive())
                .collect();
            if matches.is_empty() {
                return false;
            } else if matches.len() == 1 {
                assumption.extend_from_slice(&matches)
            } else {
                if this_call.is_none() {
                    let new_var = s.solver.new_var();
                    this_call = Some(new_var);
                    assumption.push(new_var.positive());
                }
                let mut matches = matches;
                matches.push(this_call.unwrap().negative());
                s.solver.add_clause(&matches);
            }
        }

        s.solver.assume(&assumption);

        s.solver
            .solve()
            .expect("docs say it can't error in default config")
    }
    pub fn sat_is_valid_solution(&self, pids: &[PackageId]) -> bool {
        let mut s = self.0.borrow_mut();
        for p in pids {
            if p.name().as_str() != "root" && !s.var_for_is_packages_used.contains_key(p) {
                return false;
            }
        }
        let assumption: Vec<_> = s
            .var_for_is_packages_used
            .iter()
            .map(|(p, v)| v.lit(pids.contains(p)))
            .collect();

        s.solver.assume(&assumption);

        s.solver
            .solve()
            .expect("docs say it can't error in default config")
    }
    fn use_packages(&self) -> Option<String> {
        self.0.borrow().solver.model().map(|lits| {
            let lits: HashSet<_> = lits
                .iter()
                .filter(|l| l.is_positive())
                .map(|l| l.var())
                .collect();
            let mut out = String::new();
            out.push_str("used:\n");
            for (p, v) in self.0.borrow().var_for_is_packages_used.iter() {
                if lits.contains(v) {
                    writeln!(&mut out, "    {}", p).unwrap();
                }
            }
            out
        })
    }
}

pub trait ToDep {
    fn to_dep(self) -> Dependency;
}

impl ToDep for &'static str {
    fn to_dep(self) -> Dependency {
        Dependency::parse(self, Some("1.0.0"), registry_loc()).unwrap()
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

#[macro_export]
macro_rules! pkg {
    ($pkgid:expr => [$($deps:expr),+ $(,)* ]) => ({
        let d: Vec<Dependency> = vec![$($deps.to_dep()),+];
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
        Some(pkgid.name().as_str())
    } else {
        None
    };
    Summary::new(
        name.to_pkgid(),
        dep,
        &BTreeMap::new(),
        link,
        None::<RustVersion>,
    )
    .unwrap()
}

pub fn pkg_id(name: &str) -> PackageId {
    PackageId::new(name, "1.0.0", registry_loc()).unwrap()
}

fn pkg_id_loc(name: &str, loc: &str) -> PackageId {
    let remote = loc.into_url();
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
        &BTreeMap::new(),
        link,
        None::<RustVersion>,
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
        &BTreeMap::new(),
        sum.links().map(|a| a.as_str()),
        None::<RustVersion>,
    )
    .unwrap()
}

pub fn dep(name: &str) -> Dependency {
    dep_req(name, "*")
}
pub fn dep_req(name: &str, req: &str) -> Dependency {
    Dependency::parse(name, Some(req), registry_loc()).unwrap()
}
pub fn dep_req_kind(name: &str, req: &str, kind: DepKind, public: bool) -> Dependency {
    let mut dep = dep_req(name, req);
    dep.set_kind(kind);
    dep.set_public(public);
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
                pkg!(("cat", "1.0.2") => [dep_req_kind("other", "2", DepKind::Build, false)]),
                pkg!(("cat", "1.0.3") => [dep_req_kind("other", "2", DepKind::Development, false)]),
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
                            0 => DepKind::Normal,
                            1 => DepKind::Build,
                            // => DepKind::Development, // Development has no impact so don't gen
                            _ => panic!("bad index for DepKind"),
                        },
                        p && k == 0,
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
pub fn assert_contains<A: PartialEq>(xs: &[A], elems: &[A]) {
    for elem in elems {
        assert!(xs.contains(elem));
    }
}

#[track_caller]
pub fn assert_same<A: PartialEq>(a: &[A], b: &[A]) {
    assert_eq!(a.len(), b.len());
    assert_contains(b, a);
}
