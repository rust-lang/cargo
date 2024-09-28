use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write;

use cargo::core::{Dependency, FeatureMap, FeatureValue, PackageId, Summary};
use cargo::util::interning::{InternedString, INTERNED_DEFAULT};
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
