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

use ops::Unit;
use std::collections::HashMap;
use CargoResult;
use core::dependency::Kind as DepKind;
use ops::{Context, Kind};
use core::Target;
use core::Profile;

pub fn build_unit_dependencies<'a, 'cfg>(
    roots: &[Unit<'a>],
    cx: &Context<'a, 'cfg>,
) -> CargoResult<HashMap<Unit<'a>, Vec<Unit<'a>>>> {
    let mut deps = HashMap::new();
    for unit in roots.iter() {
        deps_of(unit, cx, &mut deps)?;
    }

    Ok(deps)
}

fn deps_of<'a, 'b, 'cfg>(
    unit: &Unit<'a>,
    cx: &Context<'a, 'cfg>,
    deps: &'b mut HashMap<Unit<'a>, Vec<Unit<'a>>>,
) -> CargoResult<&'b [Unit<'a>]> {
    if !deps.contains_key(unit) {
        let unit_deps = compute_deps(unit, cx, deps)?;
        deps.insert(*unit, unit_deps.clone());
        for unit in unit_deps {
            deps_of(&unit, cx, deps)?;
        }
    }
    Ok(deps[unit].as_ref())
}

/// For a package, return all targets which are registered as dependencies
/// for that package.
fn compute_deps<'a, 'b, 'cfg>(
    unit: &Unit<'a>,
    cx: &Context<'a, 'cfg>,
    deps: &'b mut HashMap<Unit<'a>, Vec<Unit<'a>>>,
) -> CargoResult<Vec<Unit<'a>>> {
    if unit.profile.run_custom_build {
        return compute_deps_custom_build(unit, cx, deps);
    } else if unit.profile.doc && !unit.profile.test {
        return compute_deps_doc(unit, cx);
    }

    let id = unit.pkg.package_id();
    let deps = cx.resolve.deps(id);
    let mut ret = deps.filter(|dep| {
        unit.pkg
            .dependencies()
            .iter()
            .filter(|d| d.name() == dep.name() && d.version_req().matches(dep.version()))
            .any(|d| {
                // If this target is a build command, then we only want build
                // dependencies, otherwise we want everything *other than* build
                // dependencies.
                if unit.target.is_custom_build() != d.is_build() {
                    return false;
                }

                // If this dependency is *not* a transitive dependency, then it
                // only applies to test/example targets
                if !d.is_transitive() && !unit.target.is_test() && !unit.target.is_example()
                    && !unit.profile.test
                {
                    return false;
                }

                // If this dependency is only available for certain platforms,
                // make sure we're only enabling it for that platform.
                if !cx.dep_platform_activated(d, unit.kind) {
                    return false;
                }

                // If the dependency is optional, then we're only activating it
                // if the corresponding feature was activated
                if d.is_optional() && !cx.resolve.features(id).contains(&*d.name()) {
                    return false;
                }

                // If we've gotten past all that, then this dependency is
                // actually used!
                true
            })
    }).filter_map(|id| match cx.get_package(id) {
            Ok(pkg) => pkg.targets().iter().find(|t| t.is_lib()).map(|t| {
                let unit = Unit {
                    pkg,
                    target: t,
                    profile: lib_or_check_profile(unit, t, cx),
                    kind: unit.kind.for_target(t),
                };
                Ok(unit)
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
    ret.extend(dep_build_script(unit, cx));

    // If this target is a binary, test, example, etc, then it depends on
    // the library of the same package. The call to `resolve.deps` above
    // didn't include `pkg` in the return values, so we need to special case
    // it here and see if we need to push `(pkg, pkg_lib_target)`.
    if unit.target.is_lib() && !unit.profile.doc {
        return Ok(ret);
    }
    ret.extend(maybe_lib(unit, cx));

    // Integration tests/benchmarks require binaries to be built
    if unit.profile.test && (unit.target.is_test() || unit.target.is_bench()) {
        ret.extend(
            unit.pkg
                .targets()
                .iter()
                .filter(|t| {
                    let no_required_features = Vec::new();

                    t.is_bin() &&
                        // Skip binaries with required features that have not been selected.
                        t.required_features().unwrap_or(&no_required_features).iter().all(|f| {
                            cx.resolve.features(id).contains(f)
                        })
                })
                .map(|t| Unit {
                    pkg: unit.pkg,
                    target: t,
                    profile: lib_or_check_profile(unit, t, cx),
                    kind: unit.kind.for_target(t),
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
    cx: &Context<'a, 'cfg>,
    deps: &mut HashMap<Unit<'a>, Vec<Unit<'a>>>,
) -> CargoResult<Vec<Unit<'a>>> {
    // When not overridden, then the dependencies to run a build script are:
    //
    // 1. Compiling the build script itcx
    // 2. For each immediate dependency of our package which has a `links`
    //    key, the execution of that build script.
    let not_custom_build = unit.pkg
        .targets()
        .iter()
        .find(|t| !t.is_custom_build())
        .unwrap();
    let tmp = Unit {
        target: not_custom_build,
        profile: &cx.profiles.dev,
        ..*unit
    };
    let deps = deps_of(&tmp, cx, deps)?;
    Ok(deps.iter()
        .filter_map(|unit| {
            if !unit.target.linkable() || unit.pkg.manifest().links().is_none() {
                return None;
            }
            dep_build_script(unit, cx)
        })
        .chain(Some(Unit {
            profile: cx.build_script_profile(unit.pkg.package_id()),
            kind: Kind::Host, // build scripts always compiled for the host
            ..*unit
        }))
        .collect())
}

/// Returns the dependencies necessary to document a package
fn compute_deps_doc<'a, 'cfg>(
    unit: &Unit<'a>,
    cx: &Context<'a, 'cfg>,
) -> CargoResult<Vec<Unit<'a>>> {
    let deps = cx.resolve
        .deps(unit.pkg.package_id())
        .filter(|dep| {
            unit.pkg
                .dependencies()
                .iter()
                .filter(|d| d.name() == dep.name())
                .any(|dep| match dep.kind() {
                    DepKind::Normal => cx.dep_platform_activated(dep, unit.kind),
                    _ => false,
                })
        })
        .map(|dep| cx.get_package(dep));

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
        ret.push(Unit {
            pkg: dep,
            target: lib,
            profile: lib_or_check_profile(unit, lib, cx),
            kind: unit.kind.for_target(lib),
        });
        if cx.build_config.doc_all {
            ret.push(Unit {
                pkg: dep,
                target: lib,
                profile: &cx.profiles.doc,
                kind: unit.kind.for_target(lib),
            });
        }
    }

    // Be sure to build/run the build script for documented libraries as
    ret.extend(dep_build_script(unit, cx));

    // If we document a binary, we need the library available
    if unit.target.is_bin() {
        ret.extend(maybe_lib(unit, cx));
    }
    Ok(ret)
}

fn maybe_lib<'a, 'cfg>(unit: &Unit<'a>, cx: &Context<'a, 'cfg>) -> Option<Unit<'a>> {
    unit.pkg
        .targets()
        .iter()
        .find(|t| t.linkable())
        .map(|t| Unit {
            pkg: unit.pkg,
            target: t,
            profile: lib_or_check_profile(unit, t, cx),
            kind: unit.kind.for_target(t),
        })
}

/// If a build script is scheduled to be run for the package specified by
/// `unit`, this function will return the unit to run that build script.
///
/// Overriding a build script simply means that the running of the build
/// script itself doesn't have any dependencies, so even in that case a unit
/// of work is still returned. `None` is only returned if the package has no
/// build script.
fn dep_build_script<'a, 'cfg>(unit: &Unit<'a>, cx: &Context<'a, 'cfg>) -> Option<Unit<'a>> {
    unit.pkg
        .targets()
        .iter()
        .find(|t| t.is_custom_build())
        .map(|t| Unit {
            pkg: unit.pkg,
            target: t,
            profile: &cx.profiles.custom_build,
            kind: unit.kind,
        })
}

fn lib_or_check_profile<'a, 'cfg>(
    unit: &Unit,
    target: &Target,
    cx: &Context<'a, 'cfg>,
) -> &'a Profile {
    if !target.is_custom_build() && !target.for_host()
        && (unit.profile.check || (unit.profile.doc && !unit.profile.test))
    {
        return &cx.profiles.check;
    }
    cx.lib_profile()
}
