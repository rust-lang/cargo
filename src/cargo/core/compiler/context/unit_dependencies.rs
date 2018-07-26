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

use std::collections::{HashMap, HashSet};

use CargoResult;
use core::dependency::Kind as DepKind;
use core::profiles::ProfileFor;
use core::{Package, Target};
use super::{BuildContext, CompileMode, Kind, Unit};

pub fn build_unit_dependencies<'a, 'cfg>(
    roots: &[Unit<'a>],
    bcx: &BuildContext<'a, 'cfg>,
    deps: &mut HashMap<Unit<'a>, Vec<Unit<'a>>>,
) -> CargoResult<()> {
    assert!(deps.is_empty(), "can only build unit deps once");

    for unit in roots.iter() {
        // Dependencies of tests/benches should not have `panic` set.
        // We check the global test mode to see if we are running in `cargo
        // test` in which case we ensure all dependencies have `panic`
        // cleared, and avoid building the lib thrice (once with `panic`, once
        // without, once for --test).  In particular, the lib included for
        // doctests and examples are `Build` mode here.
        let profile_for = if unit.mode.is_any_test() || bcx.build_config.test() {
            ProfileFor::TestDependency
        } else {
            ProfileFor::Any
        };
        deps_of(unit, bcx, deps, profile_for)?;
    }
    trace!("ALL UNIT DEPENDENCIES {:#?}", deps);

    connect_run_custom_build_deps(bcx, deps);

    Ok(())
}

fn deps_of<'a, 'cfg>(
    unit: &Unit<'a>,
    bcx: &BuildContext<'a, 'cfg>,
    deps: &mut HashMap<Unit<'a>, Vec<Unit<'a>>>,
    profile_for: ProfileFor,
) -> CargoResult<()> {
    // Currently the `deps` map does not include `profile_for`.  This should
    // be safe for now.  `TestDependency` only exists to clear the `panic`
    // flag, and you'll never ask for a `unit` with `panic` set as a
    // `TestDependency`.  `CustomBuild` should also be fine since if the
    // requested unit's settings are the same as `Any`, `CustomBuild` can't
    // affect anything else in the hierarchy.
    if !deps.contains_key(unit) {
        let unit_deps = compute_deps(unit, bcx, profile_for)?;
        let to_insert: Vec<_> = unit_deps.iter().map(|&(unit, _)| unit).collect();
        deps.insert(*unit, to_insert);
        for (unit, profile_for) in unit_deps {
            deps_of(&unit, bcx, deps, profile_for)?;
        }
    }
    Ok(())
}

/// For a package, return all targets which are registered as dependencies
/// for that package.
/// This returns a vec of `(Unit, ProfileFor)` pairs.  The `ProfileFor`
/// is the profile type that should be used for dependencies of the unit.
fn compute_deps<'a, 'cfg>(
    unit: &Unit<'a>,
    bcx: &BuildContext<'a, 'cfg>,
    profile_for: ProfileFor,
) -> CargoResult<Vec<(Unit<'a>, ProfileFor)>> {
    if unit.mode.is_run_custom_build() {
        return compute_deps_custom_build(unit, bcx);
    } else if unit.mode.is_doc() && !unit.mode.is_any_test() {
        // Note: This does not include Doctest.
        return compute_deps_doc(unit, bcx);
    }

    let id = unit.pkg.package_id();
    let deps = bcx.resolve.deps(id);
    let mut ret = deps.filter(|&(_id, deps)| {
        assert!(!deps.is_empty());
        deps.iter().any(|dep| {
            // If this target is a build command, then we only want build
            // dependencies, otherwise we want everything *other than* build
            // dependencies.
            if unit.target.is_custom_build() != dep.is_build() {
                return false;
            }

            // If this dependency is *not* a transitive dependency, then it
            // only applies to test/example targets
            if !dep.is_transitive() && !unit.target.is_test() && !unit.target.is_example()
                && !unit.mode.is_any_test()
            {
                return false;
            }

            // If this dependency is only available for certain platforms,
            // make sure we're only enabling it for that platform.
            if !bcx.dep_platform_activated(dep, unit.kind) {
                return false;
            }

            // If the dependency is optional, then we're only activating it
            // if the corresponding feature was activated
            if dep.is_optional() && !bcx.resolve.features(id).contains(&*dep.name_in_toml()) {
                return false;
            }

            // If we've gotten past all that, then this dependency is
            // actually used!
            true
        })
    }).filter_map(|(id, _)| match bcx.get_package(id) {
            Ok(pkg) => pkg.targets().iter().find(|t| t.is_lib()).map(|t| {
                let mode = check_or_build_mode(unit.mode, t);
                let unit = new_unit(bcx, pkg, t, profile_for, unit.kind.for_target(t), mode);
                Ok((unit, profile_for))
            }),
            Err(e) => Some(Err(e)),
        })
        .collect::<CargoResult<Vec<_>>>()?;

    // If this target is a build script, then what we've collected so far is
    // all we need. If this isn't a build script, then it depends on the
    // build script if there is one.
    if unit.target.is_custom_build() {
        return Ok(ret);
    }
    ret.extend(dep_build_script(unit, bcx));

    // If this target is a binary, test, example, etc, then it depends on
    // the library of the same package. The call to `resolve.deps` above
    // didn't include `pkg` in the return values, so we need to special case
    // it here and see if we need to push `(pkg, pkg_lib_target)`.
    if unit.target.is_lib() && unit.mode != CompileMode::Doctest {
        return Ok(ret);
    }
    ret.extend(maybe_lib(unit, bcx, profile_for));

    // If any integration tests/benches are being run, make sure that
    // binaries are built as well.
    if !unit.mode.is_check() && unit.mode.is_any_test()
        && (unit.target.is_test() || unit.target.is_bench())
    {
        ret.extend(
            unit.pkg
                .targets()
                .iter()
                .filter(|t| {
                    let no_required_features = Vec::new();

                    t.is_bin() &&
                        // Skip binaries with required features that have not been selected.
                        t.required_features().unwrap_or(&no_required_features).iter().all(|f| {
                            bcx.resolve.features(id).contains(f)
                        })
                })
                .map(|t| {
                    (
                        new_unit(
                            bcx,
                            unit.pkg,
                            t,
                            ProfileFor::Any,
                            unit.kind.for_target(t),
                            CompileMode::Build,
                        ),
                        ProfileFor::Any,
                    )
                }),
        );
    }

    Ok(ret)
}

/// Returns the dependencies needed to run a build script.
///
/// The `unit` provided must represent an execution of a build script, and
/// the returned set of units must all be run before `unit` is run.
fn compute_deps_custom_build<'a, 'cfg>(
    unit: &Unit<'a>,
    bcx: &BuildContext<'a, 'cfg>,
) -> CargoResult<Vec<(Unit<'a>, ProfileFor)>> {
    // When not overridden, then the dependencies to run a build script are:
    //
    // 1. Compiling the build script itself
    // 2. For each immediate dependency of our package which has a `links`
    //    key, the execution of that build script.
    //
    // We don't have a great way of handling (2) here right now so this is
    // deferred until after the graph of all unit dependencies has been
    // constructed.
    let unit = new_unit(
        bcx,
        unit.pkg,
        unit.target,
        ProfileFor::CustomBuild,
        Kind::Host, // build scripts always compiled for the host
        CompileMode::Build,
    );
    // All dependencies of this unit should use profiles for custom
    // builds.
    Ok(vec![(unit, ProfileFor::CustomBuild)])
}

/// Returns the dependencies necessary to document a package
fn compute_deps_doc<'a, 'cfg>(
    unit: &Unit<'a>,
    bcx: &BuildContext<'a, 'cfg>,
) -> CargoResult<Vec<(Unit<'a>, ProfileFor)>> {
    let deps = bcx.resolve
        .deps(unit.pkg.package_id())
        .filter(|&(_id, deps)| {
            deps.iter().any(|dep| match dep.kind() {
                DepKind::Normal => bcx.dep_platform_activated(dep, unit.kind),
                _ => false,
            })
        })
        .map(|(id, _deps)| bcx.get_package(id));

    // To document a library, we depend on dependencies actually being
    // built. If we're documenting *all* libraries, then we also depend on
    // the documentation of the library being built.
    let mut ret = Vec::new();
    for dep in deps {
        let dep = dep?;
        let lib = match dep.targets().iter().find(|t| t.is_lib()) {
            Some(lib) => lib,
            None => continue,
        };
        // rustdoc only needs rmeta files for regular dependencies.
        // However, for plugins/proc-macros, deps should be built like normal.
        let mode = check_or_build_mode(unit.mode, lib);
        let lib_unit = new_unit(
            bcx,
            dep,
            lib,
            ProfileFor::Any,
            unit.kind.for_target(lib),
            mode,
        );
        ret.push((lib_unit, ProfileFor::Any));
        if let CompileMode::Doc { deps: true } = unit.mode {
            // Document this lib as well.
            let doc_unit = new_unit(
                bcx,
                dep,
                lib,
                ProfileFor::Any,
                unit.kind.for_target(lib),
                unit.mode,
            );
            ret.push((doc_unit, ProfileFor::Any));
        }
    }

    // Be sure to build/run the build script for documented libraries as
    ret.extend(dep_build_script(unit, bcx));

    // If we document a binary, we need the library available
    if unit.target.is_bin() {
        ret.extend(maybe_lib(unit, bcx, ProfileFor::Any));
    }
    Ok(ret)
}

fn maybe_lib<'a>(
    unit: &Unit<'a>,
    bcx: &BuildContext,
    profile_for: ProfileFor,
) -> Option<(Unit<'a>, ProfileFor)> {
    unit.pkg.targets().iter().find(|t| t.linkable()).map(|t| {
        let mode = check_or_build_mode(unit.mode, t);
        let unit = new_unit(bcx, unit.pkg, t, profile_for, unit.kind.for_target(t), mode);
        (unit, profile_for)
    })
}

/// If a build script is scheduled to be run for the package specified by
/// `unit`, this function will return the unit to run that build script.
///
/// Overriding a build script simply means that the running of the build
/// script itself doesn't have any dependencies, so even in that case a unit
/// of work is still returned. `None` is only returned if the package has no
/// build script.
fn dep_build_script<'a>(unit: &Unit<'a>, bcx: &BuildContext) -> Option<(Unit<'a>, ProfileFor)> {
    unit.pkg
        .targets()
        .iter()
        .find(|t| t.is_custom_build())
        .map(|t| {
            // The profile stored in the Unit is the profile for the thing
            // the custom build script is running for.
            (
                Unit {
                    pkg: unit.pkg,
                    target: t,
                    profile: bcx.profiles.get_profile_run_custom_build(&unit.profile),
                    kind: unit.kind,
                    mode: CompileMode::RunCustomBuild,
                },
                ProfileFor::CustomBuild,
            )
        })
}

/// Choose the correct mode for dependencies.
fn check_or_build_mode(mode: CompileMode, target: &Target) -> CompileMode {
    match mode {
        CompileMode::Check { .. } | CompileMode::Doc { .. } => {
            if target.for_host() {
                // Plugin and proc-macro targets should be compiled like
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

fn new_unit<'a>(
    bcx: &BuildContext,
    pkg: &'a Package,
    target: &'a Target,
    profile_for: ProfileFor,
    kind: Kind,
    mode: CompileMode,
) -> Unit<'a> {
    let profile = bcx.profiles.get_profile(
        &pkg.package_id(),
        bcx.ws.is_member(pkg),
        profile_for,
        mode,
        bcx.build_config.release,
    );
    Unit {
        pkg,
        target,
        profile,
        kind,
        mode,
    }
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
fn connect_run_custom_build_deps<'a>(
    bcx: &BuildContext,
    deps: &mut HashMap<Unit<'a>, Vec<Unit<'a>>>,
) {
    let mut new_deps = Vec::new();

    {
        // First up build a reverse dependency map. This is a mapping of all
        // `RunCustomBuild` known steps to the unit which depends on them. For
        // example a library might depend on a build script, so this map will
        // have the build script as the key and the library would be in the
        // value's set.
        let mut reverse_deps = HashMap::new();
        for (unit, deps) in deps.iter() {
            for dep in deps {
                if dep.mode == CompileMode::RunCustomBuild {
                    reverse_deps.entry(dep)
                        .or_insert_with(HashSet::new)
                        .insert(unit);
                }
            }
        }

        // And next we take a look at all build scripts executions listed in the
        // dependency map. Our job here is to take everything that depends on
        // this build script (from our reverse map above) and look at the other
        // package dependencies of these parents.
        //
        // If we depend on a linkable target and the build script mentions
        // `links`, then we depend on that package's build script! Here we use
        // `dep_build_script` to manufacture an appropriate build script unit to
        // depend on.
        for unit in deps.keys().filter(|k| k.mode == CompileMode::RunCustomBuild) {
            let reverse_deps = match reverse_deps.get(unit) {
                Some(set) => set,
                None => continue,
            };

            let to_add = reverse_deps
                .iter()
                .flat_map(|reverse_dep| deps[reverse_dep].iter())
                .filter(|other| {
                    other.pkg != unit.pkg &&
                        other.target.linkable() &&
                        other.pkg.manifest().links().is_some()
                })
                .filter_map(|other| dep_build_script(other, bcx).map(|p| p.0))
                .collect::<HashSet<_>>();

            if !to_add.is_empty() {
                new_deps.push((*unit, to_add));
            }
        }
    }

    // And finally, add in all the missing dependencies!
    for (unit, new_deps) in new_deps {
        deps.get_mut(&unit).unwrap().extend(new_deps);
    }
}
