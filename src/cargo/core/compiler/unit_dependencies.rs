//! Constructs the dependency graph for compilation.
//!
//! Rust code is typically organized as a set of Cargo packages. The
//! dependencies between the packages themselves are stored in the
//! `Resolve` struct. However, we can't use that information as is for
//! compilation! A package typically contains several targets, or crates,
//! and these targets has inter-dependencies. For example, you need to
//! compile the `lib` target before the `bin` one, and you need to compile
//! `build.rs` before either of those.
//!
//! So, we need to lower the `Resolve`, which specifies dependencies between
//! *packages*, to a graph of dependencies between their *targets*, and this
//! is exactly what this module is doing! Well, almost exactly: another
//! complication is that we might want to compile the same target several times
//! (for example, with and without tests), so we actually build a dependency
//! graph of `Unit`s, which capture these properties.

use crate::core::compiler::unit_graph::{UnitDep, UnitGraph};
use crate::core::compiler::UnitInterner;
use crate::core::compiler::{CompileKind, CompileMode, RustcTargetData, Unit};
use crate::core::dependency::DepKind;
use crate::core::profiles::{Profile, Profiles, UnitFor};
use crate::core::resolver::features::{FeaturesFor, ResolvedFeatures};
use crate::core::resolver::Resolve;
use crate::core::{Dependency, Package, PackageId, PackageSet, Target, Workspace};
use crate::ops::resolve_all_features;
use crate::util::interning::InternedString;
use crate::util::Config;
use crate::CargoResult;
use log::trace;
use std::collections::{HashMap, HashSet};

/// Collection of stuff used while creating the `UnitGraph`.
struct State<'a, 'cfg> {
    ws: &'a Workspace<'cfg>,
    config: &'cfg Config,
    unit_dependencies: UnitGraph,
    package_set: &'a PackageSet<'cfg>,
    usr_resolve: &'a Resolve,
    usr_features: &'a ResolvedFeatures,
    std_resolve: Option<&'a Resolve>,
    std_features: Option<&'a ResolvedFeatures>,
    /// This flag is `true` while generating the dependencies for the standard
    /// library.
    is_std: bool,
    global_mode: CompileMode,
    target_data: &'a RustcTargetData,
    profiles: &'a Profiles,
    interner: &'a UnitInterner,
}

pub fn build_unit_dependencies<'a, 'cfg>(
    ws: &'a Workspace<'cfg>,
    package_set: &'a PackageSet<'cfg>,
    resolve: &'a Resolve,
    features: &'a ResolvedFeatures,
    std_resolve: Option<&'a (Resolve, ResolvedFeatures)>,
    roots: &[Unit],
    std_roots: &HashMap<CompileKind, Vec<Unit>>,
    global_mode: CompileMode,
    target_data: &'a RustcTargetData,
    profiles: &'a Profiles,
    interner: &'a UnitInterner,
) -> CargoResult<UnitGraph> {
    let (std_resolve, std_features) = match std_resolve {
        Some((r, f)) => (Some(r), Some(f)),
        None => (None, None),
    };
    let mut state = State {
        ws,
        config: ws.config(),
        unit_dependencies: HashMap::new(),
        package_set,
        usr_resolve: resolve,
        usr_features: features,
        std_resolve,
        std_features,
        is_std: false,
        global_mode,
        target_data,
        profiles,
        interner,
    };

    let std_unit_deps = calc_deps_of_std(&mut state, std_roots)?;

    deps_of_roots(roots, &mut state)?;
    super::links::validate_links(state.resolve(), &state.unit_dependencies)?;
    // Hopefully there aren't any links conflicts with the standard library?

    if let Some(std_unit_deps) = std_unit_deps {
        attach_std_deps(&mut state, std_roots, std_unit_deps);
    }

    connect_run_custom_build_deps(&mut state.unit_dependencies);

    // Dependencies are used in tons of places throughout the backend, many of
    // which affect the determinism of the build itself. As a result be sure
    // that dependency lists are always sorted to ensure we've always got a
    // deterministic output.
    for list in state.unit_dependencies.values_mut() {
        list.sort();
    }
    trace!("ALL UNIT DEPENDENCIES {:#?}", state.unit_dependencies);

    Ok(state.unit_dependencies)
}

/// Compute all the dependencies for the standard library.
fn calc_deps_of_std(
    mut state: &mut State<'_, '_>,
    std_roots: &HashMap<CompileKind, Vec<Unit>>,
) -> CargoResult<Option<UnitGraph>> {
    if std_roots.is_empty() {
        return Ok(None);
    }
    // Compute dependencies for the standard library.
    state.is_std = true;
    for roots in std_roots.values() {
        deps_of_roots(roots, &mut state)?;
    }
    state.is_std = false;
    Ok(Some(std::mem::take(&mut state.unit_dependencies)))
}

/// Add the standard library units to the `unit_dependencies`.
fn attach_std_deps(
    state: &mut State<'_, '_>,
    std_roots: &HashMap<CompileKind, Vec<Unit>>,
    std_unit_deps: UnitGraph,
) {
    // Attach the standard library as a dependency of every target unit.
    for (unit, deps) in state.unit_dependencies.iter_mut() {
        if !unit.kind.is_host() && !unit.mode.is_run_custom_build() {
            deps.extend(std_roots[&unit.kind].iter().map(|unit| UnitDep {
                unit: unit.clone(),
                unit_for: UnitFor::new_normal(),
                extern_crate_name: unit.pkg.name(),
                // TODO: Does this `public` make sense?
                public: true,
                noprelude: true,
            }));
        }
    }
    // And also include the dependencies of the standard library itself.
    for (unit, deps) in std_unit_deps.into_iter() {
        if let Some(other_unit) = state.unit_dependencies.insert(unit, deps) {
            panic!("std unit collision with existing unit: {:?}", other_unit);
        }
    }
}

/// Compute all the dependencies of the given root units.
/// The result is stored in state.unit_dependencies.
fn deps_of_roots(roots: &[Unit], mut state: &mut State<'_, '_>) -> CargoResult<()> {
    for unit in roots.iter() {
        // Dependencies of tests/benches should not have `panic` set.
        // We check the global test mode to see if we are running in `cargo
        // test` in which case we ensure all dependencies have `panic`
        // cleared, and avoid building the lib thrice (once with `panic`, once
        // without, once for `--test`). In particular, the lib included for
        // Doc tests and examples are `Build` mode here.
        let unit_for = if unit.mode.is_any_test() || state.global_mode.is_rustc_test() {
            if unit.target.proc_macro() {
                // Special-case for proc-macros, which are forced to for-host
                // since they need to link with the proc_macro crate.
                UnitFor::new_host_test(state.config)
            } else {
                UnitFor::new_test(state.config)
            }
        } else if unit.target.is_custom_build() {
            // This normally doesn't happen, except `clean` aggressively
            // generates all units.
            UnitFor::new_host(false)
        } else if unit.target.proc_macro() {
            UnitFor::new_host(true)
        } else if unit.target.for_host() {
            // Plugin should never have panic set.
            UnitFor::new_compiler()
        } else {
            UnitFor::new_normal()
        };
        deps_of(unit, &mut state, unit_for)?;
    }

    Ok(())
}

/// Compute the dependencies of a single unit.
fn deps_of(unit: &Unit, state: &mut State<'_, '_>, unit_for: UnitFor) -> CargoResult<()> {
    // Currently the `unit_dependencies` map does not include `unit_for`. This should
    // be safe for now. `TestDependency` only exists to clear the `panic`
    // flag, and you'll never ask for a `unit` with `panic` set as a
    // `TestDependency`. `CustomBuild` should also be fine since if the
    // requested unit's settings are the same as `Any`, `CustomBuild` can't
    // affect anything else in the hierarchy.
    if !state.unit_dependencies.contains_key(unit) {
        let unit_deps = compute_deps(unit, state, unit_for)?;
        state
            .unit_dependencies
            .insert(unit.clone(), unit_deps.clone());
        for unit_dep in unit_deps {
            deps_of(&unit_dep.unit, state, unit_dep.unit_for)?;
        }
    }
    Ok(())
}

/// For a package, returns all targets that are registered as dependencies
/// for that package.
/// This returns a `Vec` of `(Unit, UnitFor)` pairs. The `UnitFor`
/// is the profile type that should be used for dependencies of the unit.
fn compute_deps(
    unit: &Unit,
    state: &mut State<'_, '_>,
    unit_for: UnitFor,
) -> CargoResult<Vec<UnitDep>> {
    if unit.mode.is_run_custom_build() {
        return compute_deps_custom_build(unit, unit_for, state);
    } else if unit.mode.is_doc() {
        // Note: this does not include doc test.
        return compute_deps_doc(unit, state);
    }

    let id = unit.pkg.package_id();
    let filtered_deps = state
        .deps(unit, unit_for)
        .into_iter()
        .filter(|&(_id, deps)| {
            deps.iter().any(|dep| {
                // If this target is a build command, then we only want build
                // dependencies, otherwise we want everything *other than* build
                // dependencies.
                if unit.target.is_custom_build() != dep.is_build() {
                    return false;
                }

                // If this dependency is **not** a transitive dependency, then it
                // only applies to test/example targets.
                if !dep.is_transitive()
                    && !unit.target.is_test()
                    && !unit.target.is_example()
                    && !unit.mode.is_any_test()
                {
                    return false;
                }

                // If we've gotten past all that, then this dependency is
                // actually used!
                true
            })
        });

    let mut ret = Vec::new();
    for (id, _) in filtered_deps {
        let pkg = state.get(id);
        let lib = match pkg.targets().iter().find(|t| t.is_lib()) {
            Some(t) => t,
            None => continue,
        };
        let mode = check_or_build_mode(unit.mode, lib);
        let dep_unit_for = unit_for
            .with_for_host(lib.for_host())
            // If it is a custom build script, then it *only* has build dependencies.
            .with_host_features(unit.target.is_custom_build() || lib.proc_macro());

        if state.config.cli_unstable().dual_proc_macros && lib.proc_macro() && !unit.kind.is_host()
        {
            let unit_dep = new_unit_dep(state, unit, pkg, lib, dep_unit_for, unit.kind, mode)?;
            ret.push(unit_dep);
            let unit_dep =
                new_unit_dep(state, unit, pkg, lib, dep_unit_for, CompileKind::Host, mode)?;
            ret.push(unit_dep);
        } else {
            let unit_dep = new_unit_dep(
                state,
                unit,
                pkg,
                lib,
                dep_unit_for,
                unit.kind.for_target(lib),
                mode,
            )?;
            ret.push(unit_dep);
        }
    }

    // If this target is a build script, then what we've collected so far is
    // all we need. If this isn't a build script, then it depends on the
    // build script if there is one.
    if unit.target.is_custom_build() {
        return Ok(ret);
    }
    ret.extend(dep_build_script(unit, unit_for, state)?);

    // If this target is a binary, test, example, etc, then it depends on
    // the library of the same package. The call to `resolve.deps` above
    // didn't include `pkg` in the return values, so we need to special case
    // it here and see if we need to push `(pkg, pkg_lib_target)`.
    if unit.target.is_lib() && unit.mode != CompileMode::Doctest {
        return Ok(ret);
    }
    ret.extend(maybe_lib(unit, state, unit_for)?);

    // If any integration tests/benches are being run, make sure that
    // binaries are built as well.
    if !unit.mode.is_check()
        && unit.mode.is_any_test()
        && (unit.target.is_test() || unit.target.is_bench())
    {
        ret.extend(
            unit.pkg
                .targets()
                .iter()
                .filter(|t| {
                    // Skip binaries with required features that have not been selected.
                    match t.required_features() {
                        Some(rf) if t.is_bin() => {
                            let features = resolve_all_features(
                                state.resolve(),
                                state.features(),
                                state.package_set,
                                id,
                            );
                            rf.iter().all(|f| features.contains(f))
                        }
                        None if t.is_bin() => true,
                        _ => false,
                    }
                })
                .map(|t| {
                    new_unit_dep(
                        state,
                        unit,
                        &unit.pkg,
                        t,
                        UnitFor::new_normal(),
                        unit.kind.for_target(t),
                        CompileMode::Build,
                    )
                })
                .collect::<CargoResult<Vec<UnitDep>>>()?,
        );
    }

    Ok(ret)
}

/// Returns the dependencies needed to run a build script.
///
/// The `unit` provided must represent an execution of a build script, and
/// the returned set of units must all be run before `unit` is run.
fn compute_deps_custom_build(
    unit: &Unit,
    unit_for: UnitFor,
    state: &mut State<'_, '_>,
) -> CargoResult<Vec<UnitDep>> {
    if let Some(links) = unit.pkg.manifest().links() {
        if state
            .target_data
            .script_override(links, unit.kind)
            .is_some()
        {
            // Overridden build scripts don't have any dependencies.
            return Ok(Vec::new());
        }
    }
    // All dependencies of this unit should use profiles for custom builds.
    // If this is a build script of a proc macro, make sure it uses host
    // features.
    let script_unit_for = UnitFor::new_host(unit_for.is_for_host_features());
    // When not overridden, then the dependencies to run a build script are:
    //
    // 1. Compiling the build script itself.
    // 2. For each immediate dependency of our package which has a `links`
    //    key, the execution of that build script.
    //
    // We don't have a great way of handling (2) here right now so this is
    // deferred until after the graph of all unit dependencies has been
    // constructed.
    let unit_dep = new_unit_dep(
        state,
        unit,
        &unit.pkg,
        &unit.target,
        script_unit_for,
        // Build scripts always compiled for the host.
        CompileKind::Host,
        CompileMode::Build,
    )?;
    Ok(vec![unit_dep])
}

/// Returns the dependencies necessary to document a package.
fn compute_deps_doc(unit: &Unit, state: &mut State<'_, '_>) -> CargoResult<Vec<UnitDep>> {
    let deps = state
        .deps(unit, UnitFor::new_normal())
        .into_iter()
        .filter(|&(_id, deps)| deps.iter().any(|dep| dep.kind() == DepKind::Normal));

    // To document a library, we depend on dependencies actually being
    // built. If we're documenting *all* libraries, then we also depend on
    // the documentation of the library being built.
    let mut ret = Vec::new();
    for (id, _deps) in deps {
        let dep = state.get(id);
        let lib = match dep.targets().iter().find(|t| t.is_lib()) {
            Some(lib) => lib,
            None => continue,
        };
        // Rustdoc only needs rmeta files for regular dependencies.
        // However, for plugins/proc macros, deps should be built like normal.
        let mode = check_or_build_mode(unit.mode, lib);
        let dep_unit_for = UnitFor::new_normal()
            .with_for_host(lib.for_host())
            .with_host_features(lib.proc_macro());
        let lib_unit_dep = new_unit_dep(
            state,
            unit,
            dep,
            lib,
            dep_unit_for,
            unit.kind.for_target(lib),
            mode,
        )?;
        ret.push(lib_unit_dep);
        if let CompileMode::Doc { deps: true } = unit.mode {
            // Document this lib as well.
            let doc_unit_dep = new_unit_dep(
                state,
                unit,
                dep,
                lib,
                dep_unit_for,
                unit.kind.for_target(lib),
                unit.mode,
            )?;
            ret.push(doc_unit_dep);
        }
    }

    // Be sure to build/run the build script for documented libraries.
    ret.extend(dep_build_script(unit, UnitFor::new_normal(), state)?);

    // If we document a binary/example, we need the library available.
    if unit.target.is_bin() || unit.target.is_example() {
        ret.extend(maybe_lib(unit, state, UnitFor::new_normal())?);
    }
    Ok(ret)
}

fn maybe_lib(
    unit: &Unit,
    state: &mut State<'_, '_>,
    unit_for: UnitFor,
) -> CargoResult<Option<UnitDep>> {
    unit.pkg
        .targets()
        .iter()
        .find(|t| t.is_linkable())
        .map(|t| {
            let mode = check_or_build_mode(unit.mode, t);
            new_unit_dep(
                state,
                unit,
                &unit.pkg,
                t,
                unit_for,
                unit.kind.for_target(t),
                mode,
            )
        })
        .transpose()
}

/// If a build script is scheduled to be run for the package specified by
/// `unit`, this function will return the unit to run that build script.
///
/// Overriding a build script simply means that the running of the build
/// script itself doesn't have any dependencies, so even in that case a unit
/// of work is still returned. `None` is only returned if the package has no
/// build script.
fn dep_build_script(
    unit: &Unit,
    unit_for: UnitFor,
    state: &State<'_, '_>,
) -> CargoResult<Option<UnitDep>> {
    unit.pkg
        .targets()
        .iter()
        .find(|t| t.is_custom_build())
        .map(|t| {
            // The profile stored in the Unit is the profile for the thing
            // the custom build script is running for.
            let profile = state.profiles.get_profile_run_custom_build(&unit.profile);
            // UnitFor::new_host is used because we want the `host` flag set
            // for all of our build dependencies (so they all get
            // build-override profiles), including compiling the build.rs
            // script itself.
            //
            // If `is_for_host_features` here is `false`, that means we are a
            // build.rs script for a normal dependency and we want to set the
            // CARGO_FEATURE_* environment variables to the features as a
            // normal dep.
            //
            // If `is_for_host_features` here is `true`, that means that this
            // package is being used as a build dependency or proc-macro, and
            // so we only want to set CARGO_FEATURE_* variables for the host
            // side of the graph.
            //
            // Keep in mind that the RunCustomBuild unit and the Compile
            // build.rs unit use the same features. This is because some
            // people use `cfg!` and `#[cfg]` expressions to check for enabled
            // features instead of just checking `CARGO_FEATURE_*` at runtime.
            // In the case with `-Zfeatures=host_dep`, and a shared
            // dependency has different features enabled for normal vs. build,
            // then the build.rs script will get compiled twice. I believe it
            // is not feasible to only build it once because it would break a
            // large number of scripts (they would think they have the wrong
            // set of features enabled).
            let script_unit_for = UnitFor::new_host(unit_for.is_for_host_features());
            new_unit_dep_with_profile(
                state,
                unit,
                &unit.pkg,
                t,
                script_unit_for,
                unit.kind,
                CompileMode::RunCustomBuild,
                profile,
            )
        })
        .transpose()
}

/// Choose the correct mode for dependencies.
fn check_or_build_mode(mode: CompileMode, target: &Target) -> CompileMode {
    match mode {
        CompileMode::Check { .. } | CompileMode::Doc { .. } => {
            if target.for_host() {
                // Plugin and proc macro targets should be compiled like
                // normal.
                CompileMode::Build
            } else {
                // Regular dependencies should not be checked with --test.
                // Regular dependencies of doc targets should emit rmeta only.
                CompileMode::Check { test: false }
            }
        }
        _ => CompileMode::Build,
    }
}

/// Create a new Unit for a dependency from `parent` to `pkg` and `target`.
fn new_unit_dep(
    state: &State<'_, '_>,
    parent: &Unit,
    pkg: &Package,
    target: &Target,
    unit_for: UnitFor,
    kind: CompileKind,
    mode: CompileMode,
) -> CargoResult<UnitDep> {
    let is_local = pkg.package_id().source_id().is_path() && !state.is_std;
    let profile = state.profiles.get_profile(
        pkg.package_id(),
        state.ws.is_member(pkg),
        is_local,
        unit_for,
        mode,
    );
    new_unit_dep_with_profile(state, parent, pkg, target, unit_for, kind, mode, profile)
}

fn new_unit_dep_with_profile(
    state: &State<'_, '_>,
    parent: &Unit,
    pkg: &Package,
    target: &Target,
    unit_for: UnitFor,
    kind: CompileKind,
    mode: CompileMode,
    profile: Profile,
) -> CargoResult<UnitDep> {
    // TODO: consider making extern_crate_name return InternedString?
    let extern_crate_name = InternedString::new(&state.resolve().extern_crate_name(
        parent.pkg.package_id(),
        pkg.package_id(),
        target,
    )?);
    let public = state
        .resolve()
        .is_public_dep(parent.pkg.package_id(), pkg.package_id());
    let features_for = unit_for.map_to_features_for();
    let features = state.activated_features(pkg.package_id(), features_for);
    let unit = state
        .interner
        .intern(pkg, target, profile, kind, mode, features, state.is_std, 0);
    Ok(UnitDep {
        unit,
        unit_for,
        extern_crate_name,
        public,
        noprelude: false,
    })
}

/// Fill in missing dependencies for units of the `RunCustomBuild`
///
/// As mentioned above in `compute_deps_custom_build` each build script
/// execution has two dependencies. The first is compiling the build script
/// itself (already added) and the second is that all crates the package of the
/// build script depends on with `links` keys, their build script execution. (a
/// bit confusing eh?)
///
/// Here we take the entire `deps` map and add more dependencies from execution
/// of one build script to execution of another build script.
fn connect_run_custom_build_deps(unit_dependencies: &mut UnitGraph) {
    let mut new_deps = Vec::new();

    {
        // First up build a reverse dependency map. This is a mapping of all
        // `RunCustomBuild` known steps to the unit which depends on them. For
        // example a library might depend on a build script, so this map will
        // have the build script as the key and the library would be in the
        // value's set.
        let mut reverse_deps_map = HashMap::new();
        for (unit, deps) in unit_dependencies.iter() {
            for dep in deps {
                if dep.unit.mode == CompileMode::RunCustomBuild {
                    reverse_deps_map
                        .entry(dep.unit.clone())
                        .or_insert_with(HashSet::new)
                        .insert(unit);
                }
            }
        }

        // Next, we take a look at all build scripts executions listed in the
        // dependency map. Our job here is to take everything that depends on
        // this build script (from our reverse map above) and look at the other
        // package dependencies of these parents.
        //
        // If we depend on a linkable target and the build script mentions
        // `links`, then we depend on that package's build script! Here we use
        // `dep_build_script` to manufacture an appropriate build script unit to
        // depend on.
        for unit in unit_dependencies
            .keys()
            .filter(|k| k.mode == CompileMode::RunCustomBuild)
        {
            // This is the lib that runs this custom build.
            let reverse_deps = match reverse_deps_map.get(unit) {
                Some(set) => set,
                None => continue,
            };

            let to_add = reverse_deps
                .iter()
                // Get all deps for lib.
                .flat_map(|reverse_dep| unit_dependencies[reverse_dep].iter())
                // Only deps with `links`.
                .filter(|other| {
                    other.unit.pkg != unit.pkg
                        && other.unit.target.is_linkable()
                        && other.unit.pkg.manifest().links().is_some()
                })
                // Get the RunCustomBuild for other lib.
                .filter_map(|other| {
                    unit_dependencies[&other.unit]
                        .iter()
                        .find(|other_dep| other_dep.unit.mode == CompileMode::RunCustomBuild)
                        .cloned()
                })
                .collect::<HashSet<_>>();

            if !to_add.is_empty() {
                // (RunCustomBuild, set(other RunCustomBuild))
                new_deps.push((unit.clone(), to_add));
            }
        }
    }

    // And finally, add in all the missing dependencies!
    for (unit, new_deps) in new_deps {
        unit_dependencies.get_mut(&unit).unwrap().extend(new_deps);
    }
}

impl<'a, 'cfg> State<'a, 'cfg> {
    fn resolve(&self) -> &'a Resolve {
        if self.is_std {
            self.std_resolve.unwrap()
        } else {
            self.usr_resolve
        }
    }

    fn features(&self) -> &'a ResolvedFeatures {
        if self.is_std {
            self.std_features.unwrap()
        } else {
            self.usr_features
        }
    }

    fn activated_features(
        &self,
        pkg_id: PackageId,
        features_for: FeaturesFor,
    ) -> Vec<InternedString> {
        let features = self.features();
        features.activated_features(pkg_id, features_for)
    }

    fn is_dep_activated(
        &self,
        pkg_id: PackageId,
        features_for: FeaturesFor,
        dep_name: InternedString,
    ) -> bool {
        self.features()
            .is_dep_activated(pkg_id, features_for, dep_name)
    }

    fn get(&self, id: PackageId) -> &'a Package {
        self.package_set
            .get_one(id)
            .unwrap_or_else(|_| panic!("expected {} to be downloaded", id))
    }

    /// Returns a filtered set of dependencies for the given unit.
    fn deps(&self, unit: &Unit, unit_for: UnitFor) -> Vec<(PackageId, &HashSet<Dependency>)> {
        let pkg_id = unit.pkg.package_id();
        let kind = unit.kind;
        self.resolve()
            .deps(pkg_id)
            .filter(|&(_id, deps)| {
                assert!(!deps.is_empty());
                deps.iter().any(|dep| {
                    // If this dependency is only available for certain platforms,
                    // make sure we're only enabling it for that platform.
                    if !self.target_data.dep_platform_activated(dep, kind) {
                        return false;
                    }

                    // If this is an optional dependency, and the new feature resolver
                    // did not enable it, don't include it.
                    if dep.is_optional() {
                        let features_for = unit_for.map_to_features_for();
                        if !self.is_dep_activated(pkg_id, features_for, dep.name_in_toml()) {
                            return false;
                        }
                    }

                    true
                })
            })
            .collect()
    }
}
