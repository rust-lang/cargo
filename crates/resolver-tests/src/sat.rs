use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write;

use cargo::core::dependency::DepKind;
use cargo::core::{Dependency, FeatureMap, FeatureValue, PackageId, Summary};
use cargo::util::interning::{InternedString, INTERNED_DEFAULT};
use cargo_platform::Platform;
use varisat::ExtendFormula;

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
    // Use the "Binary Encoding" from
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
    // No two packages with the same keys set
    let mut by_keys: HashMap<K, Vec<varisat::Var>> = HashMap::new();
    for (p, v) in data {
        by_keys.entry(p).or_default().push(v)
    }
    for key in by_keys.values() {
        sat_at_most_one(solver, key);
    }
    by_keys
}

type DependencyVarMap<'a> =
    HashMap<InternedString, HashMap<(DepKind, Option<&'a Platform>), varisat::Var>>;

type DependencyFeatureVarMap<'a> = HashMap<
    InternedString,
    HashMap<(DepKind, Option<&'a Platform>), HashMap<InternedString, varisat::Var>>,
>;

fn create_dependencies_vars<'a>(
    solver: &mut varisat::Solver<'_>,
    pkg_var: varisat::Var,
    pkg_dependencies: &'a [Dependency],
    pkg_features: &FeatureMap,
) -> (DependencyVarMap<'a>, DependencyFeatureVarMap<'a>) {
    let mut var_for_is_dependencies_used = DependencyVarMap::new();
    let mut var_for_is_dependencies_features_used = DependencyFeatureVarMap::new();

    for dep in pkg_dependencies {
        let (name, kind, platform) = (dep.name_in_toml(), dep.kind(), dep.platform());

        var_for_is_dependencies_used
            .entry(name)
            .or_default()
            .insert((kind, platform), solver.new_var());

        let dep_feature_var_map = dep
            .features()
            .iter()
            .map(|&f| (f, solver.new_var()))
            .collect();

        var_for_is_dependencies_features_used
            .entry(name)
            .or_default()
            .insert((kind, platform), dep_feature_var_map);
    }

    for feature_values in pkg_features.values() {
        for feature_value in feature_values {
            let FeatureValue::DepFeature {
                dep_name,
                dep_feature,
                weak: _,
            } = *feature_value
            else {
                continue;
            };

            for dep_features_vars in var_for_is_dependencies_features_used
                .get_mut(&dep_name)
                .expect("feature dep name exists")
                .values_mut()
            {
                dep_features_vars.insert(dep_feature, solver.new_var());
            }
        }
    }

    // If a package dependency is used, then the package is used
    for dep_var_map in var_for_is_dependencies_used.values() {
        for dep_var in dep_var_map.values() {
            solver.add_clause(&[dep_var.negative(), pkg_var.positive()]);
        }
    }

    // If a dependency feature is used, then the dependency is used
    for (&dep_name, map) in &mut var_for_is_dependencies_features_used {
        for (&(dep_kind, dep_platform), dep_feature_var_map) in map {
            for dep_feature_var in dep_feature_var_map.values() {
                let dep_var_map = &var_for_is_dependencies_used[&dep_name];
                let dep_var = dep_var_map[&(dep_kind, dep_platform)];
                solver.add_clause(&[dep_feature_var.negative(), dep_var.positive()]);
            }
        }
    }

    (
        var_for_is_dependencies_used,
        var_for_is_dependencies_features_used,
    )
}

fn process_pkg_dependencies(
    solver: &mut varisat::Solver<'_>,
    var_for_is_dependencies_used: &DependencyVarMap<'_>,
    var_for_is_dependencies_features_used: &DependencyFeatureVarMap<'_>,
    pkg_var: varisat::Var,
    pkg_dependencies: &[Dependency],
) {
    // Add clauses for package dependencies
    for dep in pkg_dependencies {
        let (name, kind, platform) = (dep.name_in_toml(), dep.kind(), dep.platform());
        let dep_var_map = &var_for_is_dependencies_used[&name];
        let dep_var = dep_var_map[&(kind, platform)];

        if !dep.is_optional() {
            solver.add_clause(&[pkg_var.negative(), dep_var.positive()]);
        }

        for &feature_name in dep.features() {
            let dep_feature_var =
                &var_for_is_dependencies_features_used[&name][&(kind, platform)][&feature_name];

            solver.add_clause(&[dep_var.negative(), dep_feature_var.positive()]);
        }
    }
}

fn process_pkg_features(
    solver: &mut varisat::Solver<'_>,
    var_for_is_dependencies_used: &DependencyVarMap<'_>,
    var_for_is_dependencies_features_used: &DependencyFeatureVarMap<'_>,
    pkg_feature_var_map: &HashMap<InternedString, varisat::Var>,
    pkg_dependencies: &[Dependency],
    pkg_features: &FeatureMap,
    check_dev_dependencies: bool,
) {
    let optional_dependencies = pkg_dependencies
        .iter()
        .filter(|dep| dep.is_optional())
        .map(|dep| (dep.kind(), dep.platform(), dep.name_in_toml()))
        .collect::<HashSet<_>>();

    // Add clauses for package features
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
                    // Add a clause for each dependency with the provided name (normal/build/dev with target)
                    for (&(dep_kind, _), &dep_var) in &var_for_is_dependencies_used[&dep_name] {
                        if dep_kind == DepKind::Development && !check_dev_dependencies {
                            continue;
                        }
                        solver.add_clause(&[pkg_feature_var.negative(), dep_var.positive()]);
                    }
                }
                FeatureValue::DepFeature {
                    dep_name,
                    dep_feature: dep_feature_name,
                    weak,
                } => {
                    // Behavior of the feature:
                    // * if dependency `dep_name` is not optional, its feature `"dep_feature_name"` is activated.
                    // * if dependency `dep_name` is optional:
                    //     - if this is a weak dependency feature:
                    //         - feature `"dep_feature_name"` of dependency `dep_name` is activated if `dep_name` has been activated via another feature.
                    //     - if this is not a weak dependency feature:
                    //         - feature `dep_name` is activated if it exists.
                    //         - dependency `dep_name` is activated.
                    //         - feature `"dep_feature_name"` of dependency `dep_name` is activated.

                    // Add clauses for each dependency with the provided name (normal/build/dev with target)
                    let dep_var_map = &var_for_is_dependencies_used[&dep_name];
                    for (&(dep_kind, dep_platform), &dep_var) in dep_var_map {
                        if dep_kind == DepKind::Development && !check_dev_dependencies {
                            continue;
                        }

                        let dep_feature_var = &var_for_is_dependencies_features_used[&dep_name]
                            [&(dep_kind, dep_platform)][&dep_feature_name];

                        solver.add_clause(&[
                            pkg_feature_var.negative(),
                            dep_var.negative(),
                            dep_feature_var.positive(),
                        ]);

                        let key = (dep_kind, dep_platform, dep_name);
                        if !weak && optional_dependencies.contains(&key) {
                            solver.add_clause(&[pkg_feature_var.negative(), dep_var.positive()]);

                            if let Some(other_feature_var) = pkg_feature_var_map.get(&dep_name) {
                                solver.add_clause(&[
                                    pkg_feature_var.negative(),
                                    other_feature_var.positive(),
                                ]);
                            }
                        }
                    }
                }
            }
        }
    }
}

fn process_compatible_dep_summaries(
    solver: &mut varisat::Solver<'_>,
    var_for_is_dependencies_used: &DependencyVarMap<'_>,
    var_for_is_dependencies_features_used: &DependencyFeatureVarMap<'_>,
    var_for_is_packages_used: &HashMap<PackageId, varisat::Var>,
    var_for_is_packages_features_used: &HashMap<PackageId, HashMap<InternedString, varisat::Var>>,
    by_name: &HashMap<InternedString, Vec<Summary>>,
    pkg_dependencies: &[Dependency],
    check_dev_dependencies: bool,
) {
    for dep in pkg_dependencies {
        if dep.kind() == DepKind::Development && !check_dev_dependencies {
            continue;
        }

        let (name, kind, platform) = (dep.name_in_toml(), dep.kind(), dep.platform());
        let dep_var_map = &var_for_is_dependencies_used[&name];
        let dep_var = dep_var_map[&(kind, platform)];

        let dep_feature_var_map = &var_for_is_dependencies_features_used[&name][&(kind, platform)];

        let compatible_summaries = by_name
            .get(&dep.package_name())
            .into_iter()
            .flatten()
            .filter(|s| dep.matches(s))
            .filter(|s| dep.features().iter().all(|f| s.features().contains_key(f)))
            .cloned()
            .collect::<Vec<_>>();

        // At least one compatible package should be activated
        let dep_clause = compatible_summaries
            .iter()
            .map(|s| var_for_is_packages_used[&s.package_id()].positive())
            .chain([dep_var.negative()])
            .collect::<Vec<_>>();

        solver.add_clause(&dep_clause);

        for (&feature_name, &dep_feature_var) in dep_feature_var_map {
            // At least one compatible package with the additional feature should be activated
            let dep_feature_clause = compatible_summaries
                .iter()
                .filter_map(|s| {
                    var_for_is_packages_features_used[&s.package_id()].get(&feature_name)
                })
                .map(|var| var.positive())
                .chain([dep_feature_var.negative()])
                .collect::<Vec<_>>();

            solver.add_clause(&dep_feature_clause);
        }

        if dep.uses_default_features() {
            // For the selected package for this dependency, the `"default"` feature should be activated if it exists
            let mut dep_default_clause = vec![dep_var.negative()];

            for s in &compatible_summaries {
                let s_pkg_id = s.package_id();
                let s_var = var_for_is_packages_used[&s_pkg_id];
                let s_feature_var_map = &var_for_is_packages_features_used[&s_pkg_id];

                if let Some(s_default_feature_var) = s_feature_var_map.get(&INTERNED_DEFAULT) {
                    dep_default_clause
                        .extend_from_slice(&[s_var.negative(), s_default_feature_var.positive()]);
                } else {
                    dep_default_clause.push(s_var.positive());
                }
            }

            solver.add_clause(&dep_default_clause);
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
    pub fn new<'a>(registry: impl IntoIterator<Item = &'a Summary>) -> Self {
        let check_dev_dependencies = false;

        let mut by_name: HashMap<InternedString, Vec<Summary>> = HashMap::new();
        for pkg in registry {
            by_name.entry(pkg.name()).or_default().push(pkg.clone());
        }

        let mut solver = varisat::Solver::new();

        // Create boolean variables for packages and packages features
        let mut var_for_is_packages_used = HashMap::new();
        let mut var_for_is_packages_features_used = HashMap::<_, HashMap<_, _>>::new();

        for pkg in by_name.values().flatten() {
            let pkg_id = pkg.package_id();

            var_for_is_packages_used.insert(pkg_id, solver.new_var());

            var_for_is_packages_features_used.insert(
                pkg_id,
                (pkg.features().keys().map(|&f| (f, solver.new_var()))).collect(),
            );
        }

        // If a package feature is used, then the package is used
        for (&pkg_id, pkg_feature_var_map) in &var_for_is_packages_features_used {
            for pkg_feature_var in pkg_feature_var_map.values() {
                let pkg_var = var_for_is_packages_used[&pkg_id];
                solver.add_clause(&[pkg_feature_var.negative(), pkg_var.positive()]);
            }
        }

        // No two packages with the same links set
        sat_at_most_one_by_key(
            &mut solver,
            by_name
                .values()
                .flatten()
                .map(|s| (s.links(), var_for_is_packages_used[&s.package_id()]))
                .filter(|(l, _)| l.is_some()),
        );

        // No two semver compatible versions of the same package
        sat_at_most_one_by_key(
            &mut solver,
            var_for_is_packages_used
                .iter()
                .map(|(p, &v)| (p.activation_key(), v)),
        );

        for pkg in by_name.values().flatten() {
            let pkg_id = pkg.package_id();
            let pkg_dependencies = pkg.dependencies();
            let pkg_features = pkg.features();
            let pkg_var = var_for_is_packages_used[&pkg_id];

            // Create boolean variables for dependencies and dependencies features
            let (var_for_is_dependencies_used, var_for_is_dependencies_features_used) =
                create_dependencies_vars(&mut solver, pkg_var, pkg_dependencies, pkg_features);

            process_pkg_dependencies(
                &mut solver,
                &var_for_is_dependencies_used,
                &var_for_is_dependencies_features_used,
                pkg_var,
                pkg_dependencies,
            );

            process_pkg_features(
                &mut solver,
                &var_for_is_dependencies_used,
                &var_for_is_dependencies_features_used,
                &var_for_is_packages_features_used[&pkg_id],
                pkg_dependencies,
                pkg_features,
                check_dev_dependencies,
            );

            process_compatible_dep_summaries(
                &mut solver,
                &var_for_is_dependencies_used,
                &var_for_is_dependencies_features_used,
                &var_for_is_packages_used,
                &var_for_is_packages_features_used,
                &by_name,
                pkg_dependencies,
                check_dev_dependencies,
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

        // Create boolean variables for dependencies and dependencies features
        let (var_for_is_dependencies_used, var_for_is_dependencies_features_used) =
            create_dependencies_vars(solver, root_var, root_dependencies, &FeatureMap::new());

        process_pkg_dependencies(
            solver,
            &var_for_is_dependencies_used,
            &var_for_is_dependencies_features_used,
            root_var,
            root_dependencies,
        );

        process_compatible_dep_summaries(
            solver,
            &var_for_is_dependencies_used,
            &var_for_is_dependencies_features_used,
            var_for_is_packages_used,
            var_for_is_packages_features_used,
            by_name,
            root_dependencies,
            true,
        );

        // Root package is always used.
        // Root vars from previous runs are deactivated.
        let assumption = old_root_vars
            .iter()
            .map(|v| v.negative())
            .chain([root_var.positive()])
            .collect::<Vec<_>>();

        old_root_vars.push(root_var);

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

        // Root vars from previous runs are deactivated
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

    pub fn used_packages(&self) -> Option<String> {
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
