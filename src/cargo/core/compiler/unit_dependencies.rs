//! Constructs the dependency graph for compilation.
//!
//! Rust code is typically organized as a set of Cargo packages. The
//! dependencies between the packages themselves are stored in the
//! [`Resolve`] struct. However, we can't use that information as is for
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
//! graph of [`Unit`]s, which capture these properties.

use std::collections::{HashMap, HashSet};

use tracing::trace;

use crate::core::compiler::artifact::match_artifacts_kind_with_targets;
use crate::core::compiler::unit_graph::{UnitDep, UnitGraph};
use crate::core::compiler::{
    CompileKind, CompileMode, CrateType, RustcTargetData, Unit, UnitInterner,
};
use crate::core::dependency::{Artifact, ArtifactTarget, DepKind};
use crate::core::profiles::{Profile, Profiles, UnitFor};
use crate::core::resolver::features::{FeaturesFor, ResolvedFeatures};
use crate::core::resolver::Resolve;
use crate::core::{Dependency, Package, PackageId, PackageSet, Target, TargetKind, Workspace};
use crate::ops::resolve_all_features;
use crate::util::interning::InternedString;
use crate::util::Config;
use crate::CargoResult;

const IS_NO_ARTIFACT_DEP: Option<&'static Artifact> = None;

/// Collection of stuff used while creating the [`UnitGraph`].
struct State<'a, 'cfg> {
    ws: &'a Workspace<'cfg>,
    config: &'cfg Config,
    /// Stores the result of building the [`UnitGraph`].
    unit_dependencies: UnitGraph,
    package_set: &'a PackageSet<'cfg>,
    usr_resolve: &'a Resolve,
    usr_features: &'a ResolvedFeatures,
    /// Like `usr_resolve` but for building standard library (`-Zbuild-std`).
    std_resolve: Option<&'a Resolve>,
    /// Like `usr_features` but for building standard library (`-Zbuild-std`).
    std_features: Option<&'a ResolvedFeatures>,
    /// `true` while generating the dependencies for the standard library.
    is_std: bool,
    /// The mode we are compiling in. Used for preventing from building lib thrice.
    global_mode: CompileMode,
    target_data: &'a RustcTargetData<'cfg>,
    profiles: &'a Profiles,
    interner: &'a UnitInterner,
    // Units for `-Zrustdoc-scrape-examples`.
    scrape_units: &'a [Unit],

    /// A set of edges in `unit_dependencies` where (a, b) means that the
    /// dependency from a to b was added purely because it was a dev-dependency.
    /// This is used during `connect_run_custom_build_deps`.
    dev_dependency_edges: HashSet<(Unit, Unit)>,
}

/// A boolean-like to indicate if a `Unit` is an artifact or not.
#[derive(Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum IsArtifact {
    Yes,
    No,
}

impl IsArtifact {
    pub fn is_true(&self) -> bool {
        matches!(self, IsArtifact::Yes)
    }
}

/// Then entry point for building a dependency graph of compilation units.
///
/// You can find some information for arguments from doc of [`State`].
pub fn build_unit_dependencies<'a, 'cfg>(
    ws: &'a Workspace<'cfg>,
    package_set: &'a PackageSet<'cfg>,
    resolve: &'a Resolve,
    features: &'a ResolvedFeatures,
    std_resolve: Option<&'a (Resolve, ResolvedFeatures)>,
    roots: &[Unit],
    scrape_units: &[Unit],
    std_roots: &HashMap<CompileKind, Vec<Unit>>,
    global_mode: CompileMode,
    target_data: &'a RustcTargetData<'cfg>,
    profiles: &'a Profiles,
    interner: &'a UnitInterner,
) -> CargoResult<UnitGraph> {
    if roots.is_empty() {
        // If -Zbuild-std, don't attach units if there is nothing to build.
        // Otherwise, other parts of the code may be confused by seeing units
        // in the dep graph without a root.
        return Ok(HashMap::new());
    }
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
        scrape_units,
        dev_dependency_edges: HashSet::new(),
    };

    let std_unit_deps = calc_deps_of_std(&mut state, std_roots)?;

    deps_of_roots(roots, &mut state)?;
    super::links::validate_links(state.resolve(), &state.unit_dependencies)?;
    // Hopefully there aren't any links conflicts with the standard library?

    if let Some(std_unit_deps) = std_unit_deps {
        attach_std_deps(&mut state, std_roots, std_unit_deps);
    }

    connect_run_custom_build_deps(&mut state);

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
    state: &mut State<'_, '_>,
    std_roots: &HashMap<CompileKind, Vec<Unit>>,
) -> CargoResult<Option<UnitGraph>> {
    if std_roots.is_empty() {
        return Ok(None);
    }
    // Compute dependencies for the standard library.
    state.is_std = true;
    for roots in std_roots.values() {
        deps_of_roots(roots, state)?;
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
    let mut found = false;
    for (unit, deps) in state.unit_dependencies.iter_mut() {
        if !unit.kind.is_host() && !unit.mode.is_run_custom_build() {
            deps.extend(std_roots[&unit.kind].iter().map(|unit| UnitDep {
                unit: unit.clone(),
                unit_for: UnitFor::new_normal(unit.kind),
                extern_crate_name: unit.pkg.name(),
                dep_name: None,
                // TODO: Does this `public` make sense?
                public: true,
                noprelude: true,
            }));
            found = true;
        }
    }
    // And also include the dependencies of the standard library itself. Don't
    // include these if no units actually needed the standard library.
    if found {
        for (unit, deps) in std_unit_deps.into_iter() {
            if let Some(other_unit) = state.unit_dependencies.insert(unit, deps) {
                panic!("std unit collision with existing unit: {:?}", other_unit);
            }
        }
    }
}

/// Compute all the dependencies of the given root units.
/// The result is stored in state.unit_dependencies.
fn deps_of_roots(roots: &[Unit], state: &mut State<'_, '_>) -> CargoResult<()> {
    for unit in roots.iter() {
        // Dependencies of tests/benches should not have `panic` set.
        // We check the global test mode to see if we are running in `cargo
        // test` in which case we ensure all dependencies have `panic`
        // cleared, and avoid building the lib thrice (once with `panic`, once
        // without, once for `--test`). In particular, the lib included for
        // Doc tests and examples are `Build` mode here.
        let root_compile_kind = unit.kind;
        let unit_for = if unit.mode.is_any_test() || state.global_mode.is_rustc_test() {
            if unit.target.proc_macro() {
                // Special-case for proc-macros, which are forced to for-host
                // since they need to link with the proc_macro crate.
                UnitFor::new_host_test(state.config, root_compile_kind)
            } else {
                UnitFor::new_test(state.config, root_compile_kind)
            }
        } else if unit.target.is_custom_build() {
            // This normally doesn't happen, except `clean` aggressively
            // generates all units.
            UnitFor::new_host(false, root_compile_kind)
        } else if unit.target.proc_macro() {
            UnitFor::new_host(true, root_compile_kind)
        } else if unit.target.for_host() {
            // Plugin should never have panic set.
            UnitFor::new_compiler(root_compile_kind)
        } else {
            UnitFor::new_normal(root_compile_kind)
        };
        deps_of(unit, state, unit_for)?;
    }

    Ok(())
}

/// Compute the dependencies of a single unit, recursively computing all
/// transitive dependencies.
///
/// The result is stored in `state.unit_dependencies`.
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

/// Returns the direct unit dependencies for the given `Unit`.
fn compute_deps(
    unit: &Unit,
    state: &mut State<'_, '_>,
    unit_for: UnitFor,
) -> CargoResult<Vec<UnitDep>> {
    if unit.mode.is_run_custom_build() {
        return compute_deps_custom_build(unit, unit_for, state);
    } else if unit.mode.is_doc() {
        // Note: this does not include doc test.
        return compute_deps_doc(unit, state, unit_for);
    }

    let mut ret = Vec::new();
    let mut dev_deps = Vec::new();
    for (dep_pkg_id, deps) in state.deps(unit, unit_for) {
        let Some(dep_lib) = calc_artifact_deps(unit, unit_for, dep_pkg_id, &deps, state, &mut ret)?
        else {
            continue;
        };
        let dep_pkg = state.get(dep_pkg_id);
        let mode = check_or_build_mode(unit.mode, dep_lib);
        let dep_unit_for = unit_for.with_dependency(unit, dep_lib, unit_for.root_compile_kind());

        let start = ret.len();
        if state.config.cli_unstable().dual_proc_macros
            && dep_lib.proc_macro()
            && !unit.kind.is_host()
        {
            let unit_dep = new_unit_dep(
                state,
                unit,
                dep_pkg,
                dep_lib,
                dep_unit_for,
                unit.kind,
                mode,
                IS_NO_ARTIFACT_DEP,
            )?;
            ret.push(unit_dep);
            let unit_dep = new_unit_dep(
                state,
                unit,
                dep_pkg,
                dep_lib,
                dep_unit_for,
                CompileKind::Host,
                mode,
                IS_NO_ARTIFACT_DEP,
            )?;
            ret.push(unit_dep);
        } else {
            let unit_dep = new_unit_dep(
                state,
                unit,
                dep_pkg,
                dep_lib,
                dep_unit_for,
                unit.kind.for_target(dep_lib),
                mode,
                IS_NO_ARTIFACT_DEP,
            )?;
            ret.push(unit_dep);
        }

        // If the unit added was a dev-dependency unit, then record that in the
        // dev-dependencies array. We'll add this to
        // `state.dev_dependency_edges` at the end and process it later in
        // `connect_run_custom_build_deps`.
        if deps.iter().all(|d| !d.is_transitive()) {
            for dep in ret[start..].iter() {
                dev_deps.push((unit.clone(), dep.unit.clone()));
            }
        }
    }
    state.dev_dependency_edges.extend(dev_deps);

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
        let id = unit.pkg.package_id();
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
                        UnitFor::new_normal(unit_for.root_compile_kind()),
                        unit.kind.for_target(t),
                        CompileMode::Build,
                        IS_NO_ARTIFACT_DEP,
                    )
                })
                .collect::<CargoResult<Vec<UnitDep>>>()?,
        );
    }

    Ok(ret)
}

/// Find artifacts for all `deps` of `unit` and add units that build these artifacts
/// to `ret`.
fn calc_artifact_deps<'a>(
    unit: &Unit,
    unit_for: UnitFor,
    dep_id: PackageId,
    deps: &[&Dependency],
    state: &State<'a, '_>,
    ret: &mut Vec<UnitDep>,
) -> CargoResult<Option<&'a Target>> {
    let mut has_artifact_lib = false;
    let mut maybe_non_artifact_lib = false;
    let artifact_pkg = state.get(dep_id);
    for dep in deps {
        let Some(artifact) = dep.artifact() else {
            maybe_non_artifact_lib = true;
            continue;
        };
        has_artifact_lib |= artifact.is_lib();
        // Custom build scripts (build/compile) never get artifact dependencies,
        // but the run-build-script step does (where it is handled).
        if !unit.target.is_custom_build() {
            debug_assert!(
                !unit.mode.is_run_custom_build(),
                "BUG: This should be handled in a separate branch"
            );
            ret.extend(artifact_targets_to_unit_deps(
                unit,
                unit_for.with_artifact_features(artifact),
                state,
                artifact
                    .target()
                    .and_then(|t| match t {
                        ArtifactTarget::BuildDependencyAssumeTarget => None,
                        ArtifactTarget::Force(kind) => Some(CompileKind::Target(kind)),
                    })
                    .unwrap_or(unit.kind),
                artifact_pkg,
                dep,
            )?);
        }
    }
    if has_artifact_lib || maybe_non_artifact_lib {
        Ok(artifact_pkg.targets().iter().find(|t| t.is_lib()))
    } else {
        Ok(None)
    }
}

/// Returns the dependencies needed to run a build script.
///
/// The `unit` provided must represent an execution of a build script, and
/// the returned set of units must all be run before `unit` is run.
fn compute_deps_custom_build(
    unit: &Unit,
    unit_for: UnitFor,
    state: &State<'_, '_>,
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
    let script_unit_for = unit_for.for_custom_build();
    // When not overridden, then the dependencies to run a build script are:
    //
    // 1. Compiling the build script itself.
    // 2. For each immediate dependency of our package which has a `links`
    //    key, the execution of that build script.
    //
    // We don't have a great way of handling (2) here right now so this is
    // deferred until after the graph of all unit dependencies has been
    // constructed.
    let compile_script_unit = new_unit_dep(
        state,
        unit,
        &unit.pkg,
        &unit.target,
        script_unit_for,
        // Build scripts always compiled for the host.
        CompileKind::Host,
        CompileMode::Build,
        IS_NO_ARTIFACT_DEP,
    )?;

    let mut result = vec![compile_script_unit];

    // Include any artifact dependencies.
    //
    // This is essentially the same as `calc_artifact_deps`, but there are some
    // subtle differences that require this to be implemented differently.
    //
    // Produce units that build all required artifact kinds (like binaries,
    // static libraries, etc) with the correct compile target.
    //
    // Computing the compile target for artifact units is more involved as it has to handle
    // various target configurations specific to artifacts, like `target = "target"` and
    // `target = "<triple>"`, which makes knowing the root units compile target
    // `root_unit_compile_target` necessary.
    let root_unit_compile_target = unit_for.root_compile_kind();
    let unit_for = UnitFor::new_host(/*host_features*/ true, root_unit_compile_target);
    for (dep_pkg_id, deps) in state.deps(unit, script_unit_for) {
        for dep in deps {
            if dep.kind() != DepKind::Build || dep.artifact().is_none() {
                continue;
            }
            let artifact_pkg = state.get(dep_pkg_id);
            let artifact = dep.artifact().expect("artifact dep");
            let resolved_artifact_compile_kind = artifact
                .target()
                .map(|target| target.to_resolved_compile_kind(root_unit_compile_target));

            result.extend(artifact_targets_to_unit_deps(
                unit,
                unit_for.with_artifact_features_from_resolved_compile_kind(
                    resolved_artifact_compile_kind,
                ),
                state,
                resolved_artifact_compile_kind.unwrap_or(CompileKind::Host),
                artifact_pkg,
                dep,
            )?);
        }
    }

    Ok(result)
}

/// Given a `parent` unit containing a dependency `dep` whose package is `artifact_pkg`,
/// find all targets in `artifact_pkg` which refer to the `dep`s artifact declaration
/// and turn them into units.
/// Due to the nature of artifact dependencies, a single dependency in a manifest can
/// cause one or more targets to be build, for instance with
/// `artifact = ["bin:a", "bin:b", "staticlib"]`, which is very different from normal
/// dependencies which cause only a single unit to be created.
///
/// `compile_kind` is the computed kind for the future artifact unit
/// dependency, only the caller can pick the correct one.
fn artifact_targets_to_unit_deps(
    parent: &Unit,
    parent_unit_for: UnitFor,
    state: &State<'_, '_>,
    compile_kind: CompileKind,
    artifact_pkg: &Package,
    dep: &Dependency,
) -> CargoResult<Vec<UnitDep>> {
    let ret =
        match_artifacts_kind_with_targets(dep, artifact_pkg.targets(), parent.pkg.name().as_str())?
            .into_iter()
            .map(|(_artifact_kind, target)| target)
            .flat_map(|target| {
                // We split target libraries into individual units, even though rustc is able
                // to produce multiple kinds in an single invocation for the sole reason that
                // each artifact kind has its own output directory, something we can't easily
                // teach rustc for now.
                match target.kind() {
                    TargetKind::Lib(kinds) => Box::new(
                        kinds
                            .iter()
                            .filter(|tk| matches!(tk, CrateType::Cdylib | CrateType::Staticlib))
                            .map(|target_kind| {
                                new_unit_dep(
                                    state,
                                    parent,
                                    artifact_pkg,
                                    target
                                        .clone()
                                        .set_kind(TargetKind::Lib(vec![target_kind.clone()])),
                                    parent_unit_for,
                                    compile_kind,
                                    CompileMode::Build,
                                    dep.artifact(),
                                )
                            }),
                    ) as Box<dyn Iterator<Item = _>>,
                    _ => Box::new(std::iter::once(new_unit_dep(
                        state,
                        parent,
                        artifact_pkg,
                        target,
                        parent_unit_for,
                        compile_kind,
                        CompileMode::Build,
                        dep.artifact(),
                    ))),
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
    Ok(ret)
}

/// Returns the dependencies necessary to document a package.
fn compute_deps_doc(
    unit: &Unit,
    state: &mut State<'_, '_>,
    unit_for: UnitFor,
) -> CargoResult<Vec<UnitDep>> {
    // To document a library, we depend on dependencies actually being
    // built. If we're documenting *all* libraries, then we also depend on
    // the documentation of the library being built.
    let mut ret = Vec::new();
    for (id, deps) in state.deps(unit, unit_for) {
        let Some(dep_lib) = calc_artifact_deps(unit, unit_for, id, &deps, state, &mut ret)? else {
            continue;
        };
        let dep_pkg = state.get(id);
        // Rustdoc only needs rmeta files for regular dependencies.
        // However, for plugins/proc macros, deps should be built like normal.
        let mode = check_or_build_mode(unit.mode, dep_lib);
        let dep_unit_for = unit_for.with_dependency(unit, dep_lib, unit_for.root_compile_kind());
        let lib_unit_dep = new_unit_dep(
            state,
            unit,
            dep_pkg,
            dep_lib,
            dep_unit_for,
            unit.kind.for_target(dep_lib),
            mode,
            IS_NO_ARTIFACT_DEP,
        )?;
        ret.push(lib_unit_dep);
        if dep_lib.documented() {
            if let CompileMode::Doc { deps: true } = unit.mode {
                // Document this lib as well.
                let doc_unit_dep = new_unit_dep(
                    state,
                    unit,
                    dep_pkg,
                    dep_lib,
                    dep_unit_for,
                    unit.kind.for_target(dep_lib),
                    unit.mode,
                    IS_NO_ARTIFACT_DEP,
                )?;
                ret.push(doc_unit_dep);
            }
        }
    }

    // Be sure to build/run the build script for documented libraries.
    ret.extend(dep_build_script(unit, unit_for, state)?);

    // If we document a binary/example, we need the library available.
    if unit.target.is_bin() || unit.target.is_example() {
        // build the lib
        ret.extend(maybe_lib(unit, state, unit_for)?);
        // and also the lib docs for intra-doc links
        if let Some(lib) = unit
            .pkg
            .targets()
            .iter()
            .find(|t| t.is_linkable() && t.documented())
        {
            let dep_unit_for = unit_for.with_dependency(unit, lib, unit_for.root_compile_kind());
            let lib_doc_unit = new_unit_dep(
                state,
                unit,
                &unit.pkg,
                lib,
                dep_unit_for,
                unit.kind.for_target(lib),
                unit.mode,
                IS_NO_ARTIFACT_DEP,
            )?;
            ret.push(lib_doc_unit);
        }
    }

    // Add all units being scraped for examples as a dependency of top-level Doc units.
    if state.ws.unit_needs_doc_scrape(unit) {
        for scrape_unit in state.scrape_units.iter() {
            let scrape_unit_for = UnitFor::new_normal(scrape_unit.kind);
            deps_of(scrape_unit, state, scrape_unit_for)?;
            ret.push(new_unit_dep(
                state,
                scrape_unit,
                &scrape_unit.pkg,
                &scrape_unit.target,
                scrape_unit_for,
                scrape_unit.kind,
                scrape_unit.mode,
                IS_NO_ARTIFACT_DEP,
            )?);
        }
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
            let dep_unit_for = unit_for.with_dependency(unit, t, unit_for.root_compile_kind());
            new_unit_dep(
                state,
                unit,
                &unit.pkg,
                t,
                dep_unit_for,
                unit.kind.for_target(t),
                mode,
                IS_NO_ARTIFACT_DEP,
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
            // UnitFor::for_custom_build is used because we want the `host` flag set
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
            // In the case with the new feature resolver (decoupled host
            // deps), and a shared dependency has different features enabled
            // for normal vs. build, then the build.rs script will get
            // compiled twice. I believe it is not feasible to only build it
            // once because it would break a large number of scripts (they
            // would think they have the wrong set of features enabled).
            let script_unit_for = unit_for.for_custom_build();
            new_unit_dep_with_profile(
                state,
                unit,
                &unit.pkg,
                t,
                script_unit_for,
                unit.kind,
                CompileMode::RunCustomBuild,
                profile,
                IS_NO_ARTIFACT_DEP,
            )
        })
        .transpose()
}

/// Choose the correct mode for dependencies.
fn check_or_build_mode(mode: CompileMode, target: &Target) -> CompileMode {
    match mode {
        CompileMode::Check { .. } | CompileMode::Doc { .. } | CompileMode::Docscrape => {
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
    artifact: Option<&Artifact>,
) -> CargoResult<UnitDep> {
    let is_local = pkg.package_id().source_id().is_path() && !state.is_std;
    let profile = state.profiles.get_profile(
        pkg.package_id(),
        state.ws.is_member(pkg),
        is_local,
        unit_for,
        kind,
    );
    new_unit_dep_with_profile(
        state, parent, pkg, target, unit_for, kind, mode, profile, artifact,
    )
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
    artifact: Option<&Artifact>,
) -> CargoResult<UnitDep> {
    let (extern_crate_name, dep_name) = state.resolve().extern_crate_name_and_dep_name(
        parent.pkg.package_id(),
        pkg.package_id(),
        target,
    )?;
    let public = state
        .resolve()
        .is_public_dep(parent.pkg.package_id(), pkg.package_id());
    let features_for = unit_for.map_to_features_for(artifact);
    let artifact_target = match features_for {
        FeaturesFor::ArtifactDep(target) => Some(target),
        _ => None,
    };
    let features = state.activated_features(pkg.package_id(), features_for);
    let unit = state.interner.intern(
        pkg,
        target,
        profile,
        kind,
        mode,
        features,
        state.is_std,
        /*dep_hash*/ 0,
        artifact.map_or(IsArtifact::No, |_| IsArtifact::Yes),
        artifact_target,
    );
    Ok(UnitDep {
        unit,
        unit_for,
        extern_crate_name,
        dep_name,
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
fn connect_run_custom_build_deps(state: &mut State<'_, '_>) {
    let mut new_deps = Vec::new();

    {
        let state = &*state;
        // First up build a reverse dependency map. This is a mapping of all
        // `RunCustomBuild` known steps to the unit which depends on them. For
        // example a library might depend on a build script, so this map will
        // have the build script as the key and the library would be in the
        // value's set.
        let mut reverse_deps_map = HashMap::new();
        for (unit, deps) in state.unit_dependencies.iter() {
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
        for unit in state
            .unit_dependencies
            .keys()
            .filter(|k| k.mode == CompileMode::RunCustomBuild)
        {
            // This list of dependencies all depend on `unit`, an execution of
            // the build script.
            let Some(reverse_deps) = reverse_deps_map.get(unit) else {
                continue;
            };

            let to_add = reverse_deps
                .iter()
                // Get all sibling dependencies of `unit`
                .flat_map(|reverse_dep| {
                    state.unit_dependencies[reverse_dep]
                        .iter()
                        .map(move |a| (reverse_dep, a))
                })
                // Only deps with `links`.
                .filter(|(_parent, other)| {
                    other.unit.pkg != unit.pkg
                        && other.unit.target.is_linkable()
                        && other.unit.pkg.manifest().links().is_some()
                })
                // Avoid cycles when using the doc --scrape-examples feature:
                // Say a workspace has crates A and B where A has a build-dependency on B.
                // The Doc units for A and B will have a dependency on the Docscrape for both A and B.
                // So this would add a dependency from B-build to A-build, causing a cycle:
                //   B (build) -> A (build) -> B(build)
                // See the test scrape_examples_avoid_build_script_cycle for a concrete example.
                // To avoid this cycle, we filter out the B -> A (docscrape) dependency.
                .filter(|(_parent, other)| !other.unit.mode.is_doc_scrape())
                // Skip dependencies induced via dev-dependencies since
                // connections between `links` and build scripts only happens
                // via normal dependencies. Otherwise since dev-dependencies can
                // be cyclic we could have cyclic build-script executions.
                .filter_map(move |(parent, other)| {
                    if state
                        .dev_dependency_edges
                        .contains(&((*parent).clone(), other.unit.clone()))
                    {
                        None
                    } else {
                        Some(other)
                    }
                })
                // Get the RunCustomBuild for other lib.
                .filter_map(|other| {
                    state.unit_dependencies[&other.unit]
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
        state
            .unit_dependencies
            .get_mut(&unit)
            .unwrap()
            .extend(new_deps);
    }
}

impl<'a, 'cfg> State<'a, 'cfg> {
    /// Gets `std_resolve` during building std, otherwise `usr_resolve`.
    fn resolve(&self) -> &'a Resolve {
        if self.is_std {
            self.std_resolve.unwrap()
        } else {
            self.usr_resolve
        }
    }

    /// Gets `std_features` during building std, otherwise `usr_features`.
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
    fn deps(&self, unit: &Unit, unit_for: UnitFor) -> Vec<(PackageId, Vec<&Dependency>)> {
        let pkg_id = unit.pkg.package_id();
        let kind = unit.kind;
        self.resolve()
            .deps(pkg_id)
            .filter_map(|(id, deps)| {
                assert!(!deps.is_empty());
                let deps: Vec<_> = deps
                    .iter()
                    .filter(|dep| {
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
                        if !self.target_data.dep_platform_activated(dep, kind) {
                            return false;
                        }

                        // If this is an optional dependency, and the new feature resolver
                        // did not enable it, don't include it.
                        if dep.is_optional() {
                            // This `unit_for` is from parent dep and *SHOULD* contains its own
                            // artifact dep information inside `artifact_target_for_features`.
                            // So, no need to map any artifact info from an incorrect `dep.artifact()`.
                            let features_for = unit_for.map_to_features_for(IS_NO_ARTIFACT_DEP);
                            if !self.is_dep_activated(pkg_id, features_for, dep.name_in_toml()) {
                                return false;
                            }
                        }

                        // If we've gotten past all that, then this dependency is
                        // actually used!
                        true
                    })
                    .collect();
                if deps.is_empty() {
                    None
                } else {
                    Some((id, deps))
                }
            })
            .collect()
    }
}
