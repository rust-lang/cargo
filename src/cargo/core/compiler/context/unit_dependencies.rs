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

use crate::core::compiler::Unit;
use crate::core::compiler::{BuildContext, CompileMode, Context, Kind};
use crate::core::dependency::Kind as DepKind;
use crate::core::package::Downloads;
use crate::core::profiles::UnitFor;
use crate::core::{Package, PackageId, Target};
use crate::CargoResult;
use log::trace;
use std::collections::{HashMap, HashSet};

struct State<'a, 'cfg, 'tmp> {
    cx: &'tmp mut Context<'a, 'cfg>,
    waiting_on_download: HashSet<PackageId>,
    downloads: Downloads<'a, 'cfg>,
}

pub fn build_unit_dependencies<'a, 'cfg>(
    cx: &mut Context<'a, 'cfg>,
    roots: &[Unit<'a>],
) -> CargoResult<()> {
    assert!(
        cx.unit_dependencies.is_empty(),
        "can only build unit deps once"
    );

    let mut state = State {
        downloads: cx.bcx.packages.enable_download()?,
        cx,
        waiting_on_download: HashSet::new(),
    };

    loop {
        for unit in roots.iter() {
            state.get(unit.pkg.package_id())?;

            // Dependencies of tests/benches should not have `panic` set.
            // We check the global test mode to see if we are running in `cargo
            // test` in which case we ensure all dependencies have `panic`
            // cleared, and avoid building the lib thrice (once with `panic`, once
            // without, once for `--test`). In particular, the lib included for
            // Doc tests and examples are `Build` mode here.
            let unit_for = if unit.mode.is_any_test() || state.cx.bcx.build_config.test() {
                UnitFor::new_test()
            } else if unit.target.is_custom_build() {
                // This normally doesn't happen, except `clean` aggressively
                // generates all units.
                UnitFor::new_build()
            } else if unit.target.for_host() {
                // Proc macro / plugin should never have panic set.
                UnitFor::new_compiler()
            } else {
                UnitFor::new_normal()
            };
            deps_of(unit, &mut state, unit_for)?;
        }

        if !state.waiting_on_download.is_empty() {
            state.finish_some_downloads()?;
            state.cx.unit_dependencies.clear();
        } else {
            break;
        }
    }

    connect_run_custom_build_deps(&mut state);

    trace!("ALL UNIT DEPENDENCIES {:#?}", state.cx.unit_dependencies);

    record_units_requiring_metadata(state.cx);

    // Dependencies are used in tons of places throughout the backend, many of
    // which affect the determinism of the build itself. As a result be sure
    // that dependency lists are always sorted to ensure we've always got a
    // deterministic output.
    for list in state.cx.unit_dependencies.values_mut() {
        list.sort();
    }

    Ok(())
}

fn deps_of<'a, 'cfg, 'tmp>(
    unit: &Unit<'a>,
    state: &mut State<'a, 'cfg, 'tmp>,
    unit_for: UnitFor,
) -> CargoResult<()> {
    // Currently the `unit_dependencies` map does not include `unit_for`. This should
    // be safe for now. `TestDependency` only exists to clear the `panic`
    // flag, and you'll never ask for a `unit` with `panic` set as a
    // `TestDependency`. `CustomBuild` should also be fine since if the
    // requested unit's settings are the same as `Any`, `CustomBuild` can't
    // affect anything else in the hierarchy.
    if !state.cx.unit_dependencies.contains_key(unit) {
        let unit_deps = compute_deps(unit, state, unit_for)?;
        let to_insert: Vec<_> = unit_deps.iter().map(|&(unit, _)| unit).collect();
        state.cx.unit_dependencies.insert(*unit, to_insert);
        for (unit, unit_for) in unit_deps {
            deps_of(&unit, state, unit_for)?;
        }
    }
    Ok(())
}

/// For a package, returns all targets that are registered as dependencies
/// for that package.
/// This returns a `Vec` of `(Unit, UnitFor)` pairs. The `UnitFor`
/// is the profile type that should be used for dependencies of the unit.
fn compute_deps<'a, 'cfg, 'tmp>(
    unit: &Unit<'a>,
    state: &mut State<'a, 'cfg, 'tmp>,
    unit_for: UnitFor,
) -> CargoResult<Vec<(Unit<'a>, UnitFor)>> {
    if unit.mode.is_run_custom_build() {
        return compute_deps_custom_build(unit, state.cx.bcx);
    } else if unit.mode.is_doc() {
        // Note: this does not include doc test.
        return compute_deps_doc(unit, state);
    }

    let bcx = state.cx.bcx;
    let id = unit.pkg.package_id();
    let deps = bcx.resolve.deps(id).filter(|&(_id, deps)| {
        assert!(!deps.is_empty());
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
    });

    let mut ret = Vec::new();
    for (id, _) in deps {
        let pkg = match state.get(id)? {
            Some(pkg) => pkg,
            None => continue,
        };
        let lib = match pkg.targets().iter().find(|t| t.is_lib()) {
            Some(t) => t,
            None => continue,
        };
        let mode = check_or_build_mode(unit.mode, lib);
        let dep_unit_for = unit_for.with_for_host(lib.for_host());

        if bcx.config.cli_unstable().dual_proc_macros
            && lib.proc_macro()
            && unit.kind == Kind::Target
        {
            let unit = new_unit(bcx, pkg, lib, dep_unit_for, Kind::Target, mode);
            ret.push((unit, dep_unit_for));
            let unit = new_unit(bcx, pkg, lib, dep_unit_for, Kind::Host, mode);
            ret.push((unit, dep_unit_for));
        } else {
            let unit = new_unit(bcx, pkg, lib, dep_unit_for, unit.kind.for_target(lib), mode);
            ret.push((unit, dep_unit_for));
        }
    }

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
    ret.extend(maybe_lib(unit, bcx, unit_for));

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
                            UnitFor::new_normal(),
                            unit.kind.for_target(t),
                            CompileMode::Build,
                        ),
                        UnitFor::new_normal(),
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
) -> CargoResult<Vec<(Unit<'a>, UnitFor)>> {
    // When not overridden, then the dependencies to run a build script are:
    //
    // 1. Compiling the build script itself.
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
        UnitFor::new_build(),
        // Build scripts always compiled for the host.
        Kind::Host,
        CompileMode::Build,
    );
    // All dependencies of this unit should use profiles for custom
    // builds.
    Ok(vec![(unit, UnitFor::new_build())])
}

/// Returns the dependencies necessary to document a package.
fn compute_deps_doc<'a, 'cfg, 'tmp>(
    unit: &Unit<'a>,
    state: &mut State<'a, 'cfg, 'tmp>,
) -> CargoResult<Vec<(Unit<'a>, UnitFor)>> {
    let bcx = state.cx.bcx;
    let deps = bcx
        .resolve
        .deps(unit.pkg.package_id())
        .filter(|&(_id, deps)| {
            deps.iter().any(|dep| match dep.kind() {
                DepKind::Normal => bcx.dep_platform_activated(dep, unit.kind),
                _ => false,
            })
        });

    // To document a library, we depend on dependencies actually being
    // built. If we're documenting *all* libraries, then we also depend on
    // the documentation of the library being built.
    let mut ret = Vec::new();
    for (id, _deps) in deps {
        let dep = match state.get(id)? {
            Some(dep) => dep,
            None => continue,
        };
        let lib = match dep.targets().iter().find(|t| t.is_lib()) {
            Some(lib) => lib,
            None => continue,
        };
        // Rustdoc only needs rmeta files for regular dependencies.
        // However, for plugins/proc macros, deps should be built like normal.
        let mode = check_or_build_mode(unit.mode, lib);
        let dep_unit_for = UnitFor::new_normal().with_for_host(lib.for_host());
        let lib_unit = new_unit(bcx, dep, lib, dep_unit_for, unit.kind.for_target(lib), mode);
        ret.push((lib_unit, dep_unit_for));
        if let CompileMode::Doc { deps: true } = unit.mode {
            // Document this lib as well.
            let doc_unit = new_unit(
                bcx,
                dep,
                lib,
                dep_unit_for,
                unit.kind.for_target(lib),
                unit.mode,
            );
            ret.push((doc_unit, dep_unit_for));
        }
    }

    // Be sure to build/run the build script for documented libraries.
    ret.extend(dep_build_script(unit, bcx));

    // If we document a binary/example, we need the library available.
    if unit.target.is_bin() || unit.target.is_example() {
        ret.extend(maybe_lib(unit, bcx, UnitFor::new_normal()));
    }
    Ok(ret)
}

fn maybe_lib<'a>(
    unit: &Unit<'a>,
    bcx: &BuildContext<'a, '_>,
    unit_for: UnitFor,
) -> Option<(Unit<'a>, UnitFor)> {
    unit.pkg.targets().iter().find(|t| t.linkable()).map(|t| {
        let mode = check_or_build_mode(unit.mode, t);
        let unit = new_unit(bcx, unit.pkg, t, unit_for, unit.kind.for_target(t), mode);
        (unit, unit_for)
    })
}

/// If a build script is scheduled to be run for the package specified by
/// `unit`, this function will return the unit to run that build script.
///
/// Overriding a build script simply means that the running of the build
/// script itself doesn't have any dependencies, so even in that case a unit
/// of work is still returned. `None` is only returned if the package has no
/// build script.
fn dep_build_script<'a>(
    unit: &Unit<'a>,
    bcx: &BuildContext<'a, '_>,
) -> Option<(Unit<'a>, UnitFor)> {
    unit.pkg
        .targets()
        .iter()
        .find(|t| t.is_custom_build())
        .map(|t| {
            // The profile stored in the Unit is the profile for the thing
            // the custom build script is running for.
            let unit = bcx.units.intern(
                unit.pkg,
                t,
                bcx.profiles.get_profile_run_custom_build(&unit.profile),
                unit.kind,
                CompileMode::RunCustomBuild,
            );

            (unit, UnitFor::new_build())
        })
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

fn new_unit<'a>(
    bcx: &BuildContext<'a, '_>,
    pkg: &'a Package,
    target: &'a Target,
    unit_for: UnitFor,
    kind: Kind,
    mode: CompileMode,
) -> Unit<'a> {
    let profile = bcx.profiles.get_profile(
        pkg.package_id(),
        bcx.ws.is_member(pkg),
        unit_for,
        mode,
        bcx.build_config.release,
    );

    bcx.units.intern(pkg, target, profile, kind, mode)
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
fn connect_run_custom_build_deps(state: &mut State<'_, '_, '_>) {
    let mut new_deps = Vec::new();

    {
        // First up build a reverse dependency map. This is a mapping of all
        // `RunCustomBuild` known steps to the unit which depends on them. For
        // example a library might depend on a build script, so this map will
        // have the build script as the key and the library would be in the
        // value's set.
        let mut reverse_deps = HashMap::new();
        for (unit, deps) in state.cx.unit_dependencies.iter() {
            for dep in deps {
                if dep.mode == CompileMode::RunCustomBuild {
                    reverse_deps
                        .entry(dep)
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
        for unit in state
            .cx
            .unit_dependencies
            .keys()
            .filter(|k| k.mode == CompileMode::RunCustomBuild)
        {
            let reverse_deps = match reverse_deps.get(unit) {
                Some(set) => set,
                None => continue,
            };

            let to_add = reverse_deps
                .iter()
                .flat_map(|reverse_dep| state.cx.unit_dependencies[reverse_dep].iter())
                .filter(|other| {
                    other.pkg != unit.pkg
                        && other.target.linkable()
                        && other.pkg.manifest().links().is_some()
                })
                .filter_map(|other| dep_build_script(other, state.cx.bcx).map(|p| p.0))
                .collect::<HashSet<_>>();

            if !to_add.is_empty() {
                new_deps.push((*unit, to_add));
            }
        }
    }

    // And finally, add in all the missing dependencies!
    for (unit, new_deps) in new_deps {
        state
            .cx
            .unit_dependencies
            .get_mut(&unit)
            .unwrap()
            .extend(new_deps);
    }
}

/// Records the list of units which are required to emit metadata.
///
/// Units which depend only on the metadata of others requires the others to
/// actually produce metadata, so we'll record that here.
fn record_units_requiring_metadata(cx: &mut Context<'_, '_>) {
    for (key, deps) in cx.unit_dependencies.iter() {
        for dep in deps {
            if cx.only_requires_rmeta(key, dep) {
                cx.rmeta_required.insert(*dep);
            }
        }
    }
}

impl<'a, 'cfg, 'tmp> State<'a, 'cfg, 'tmp> {
    fn get(&mut self, id: PackageId) -> CargoResult<Option<&'a Package>> {
        if let Some(pkg) = self.cx.package_cache.get(&id) {
            return Ok(Some(pkg));
        }
        if !self.waiting_on_download.insert(id) {
            return Ok(None);
        }
        if let Some(pkg) = self.downloads.start(id)? {
            self.cx.package_cache.insert(id, pkg);
            self.waiting_on_download.remove(&id);
            return Ok(Some(pkg));
        }
        Ok(None)
    }

    /// Completes at least one downloading, maybe waiting for more to complete.
    ///
    /// This function will block the current thread waiting for at least one
    /// crate to finish downloading. The function may continue to download more
    /// crates if it looks like there's a long enough queue of crates to keep
    /// downloading. When only a handful of packages remain this function
    /// returns, and it's hoped that by returning we'll be able to push more
    /// packages to download into the queue.
    fn finish_some_downloads(&mut self) -> CargoResult<()> {
        assert!(self.downloads.remaining() > 0);
        loop {
            let pkg = self.downloads.wait()?;
            self.waiting_on_download.remove(&pkg.package_id());
            self.cx.package_cache.insert(pkg.package_id(), pkg);

            // Arbitrarily choose that 5 or more packages concurrently download
            // is a good enough number to "fill the network pipe". If we have
            // less than this let's recompute the whole unit dependency graph
            // again and try to find some more packages to download.
            if self.downloads.remaining() < 5 {
                break;
            }
        }
        Ok(())
    }
}
