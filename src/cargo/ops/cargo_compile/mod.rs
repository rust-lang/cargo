//! The entry point for starting the compilation process for commands like
//! `build`, `test`, `doc`, `rustc`, etc.
//!
//! The [`compile`] function will do all the work to compile a workspace. A
//! rough outline is:
//!
//! 1. Resolve the dependency graph (see [`ops::resolve`]).
//! 2. Download any packages needed (see [`PackageSet`].
//! 3. Generate a list of top-level "units" of work for the targets the user
//!   requested on the command-line. Each [`Unit`] corresponds to a compiler
//!   invocation. This is done in this module ([`UnitGenerator::generate_root_units`]).
//! 4. Starting from the root [`Unit`]s, generate the [`UnitGraph`] by walking the dependency graph
//!   from the resolver.  See also [`unit_dependencies`].
//! 5. Construct the [`BuildContext`] with all of the information collected so
//!   far. This is the end of the "front end" of compilation.
//! 6. Create a [`Context`] which coordinates the compilation process
//!   and will perform the following steps:
//!     1. Prepare the `target` directory (see [`Layout`]).
//!     2. Create a [`JobQueue`]. The queue checks the
//!       fingerprint of each `Unit` to determine if it should run or be
//!       skipped.
//!     3. Execute the queue via [`drain_the_queue`]. Each leaf in the queue's dependency graph is
//!        executed, and then removed from the graph when finished. This repeats until the queue is
//!        empty.  Note that this is the only point in cargo that currently uses threads.
//! 7. The result of the compilation is stored in the [`Compilation`] struct. This can be used for
//!    various things, such as running tests after the compilation  has finished.
//!
//! **Note**: "target" inside this module generally refers to ["Cargo Target"],
//! which corresponds to artifact that will be built in a package. Not to be
//! confused with target-triple or target architecture.
//!
//! [`unit_dependencies`]: crate::core::compiler::unit_dependencies
//! [`Layout`]: crate::core::compiler::Layout
//! [`JobQueue`]: crate::core::compiler::job_queue
//! [`drain_the_queue`]: crate::core::compiler::job_queue
//! ["Cargo Target"]: https://doc.rust-lang.org/nightly/cargo/reference/cargo-targets.html

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::core::compiler::unit_dependencies::build_unit_dependencies;
use crate::core::compiler::unit_graph::{self, UnitDep, UnitGraph};
use crate::core::compiler::{standard_lib, CrateType, TargetInfo};
use crate::core::compiler::{BuildConfig, BuildContext, Compilation, Context};
use crate::core::compiler::{CompileKind, CompileMode, CompileTarget, RustcTargetData, Unit};
use crate::core::compiler::{DefaultExecutor, Executor, UnitInterner};
use crate::core::profiles::Profiles;
use crate::core::resolver::features::{self, CliFeatures, FeaturesFor};
use crate::core::resolver::{HasDevUnits, Resolve};
use crate::core::{PackageId, PackageSet, SourceId, TargetKind, Workspace};
use crate::drop_println;
use crate::ops;
use crate::ops::resolve::WorkspaceResolve;
use crate::util::config::Config;
use crate::util::interning::InternedString;
use crate::util::{profile, CargoResult, StableHasher};

mod compile_filter;
pub use compile_filter::{CompileFilter, FilterRule, LibRule};

mod unit_generator;
use unit_generator::UnitGenerator;

mod packages;

pub use packages::Packages;

/// Contains information about how a package should be compiled.
///
/// Note on distinction between `CompileOptions` and [`BuildConfig`]:
/// `BuildConfig` contains values that need to be retained after
/// [`BuildContext`] is created. The other fields are no longer necessary. Think
/// of it as `CompileOptions` are high-level settings requested on the
/// command-line, and `BuildConfig` are low-level settings for actually
/// driving `rustc`.
#[derive(Debug, Clone)]
pub struct CompileOptions {
    /// Configuration information for a rustc build
    pub build_config: BuildConfig,
    /// Feature flags requested by the user.
    pub cli_features: CliFeatures,
    /// A set of packages to build.
    pub spec: Packages,
    /// Filter to apply to the root package to select which targets will be
    /// built.
    pub filter: CompileFilter,
    /// Extra arguments to be passed to rustdoc (single target only)
    pub target_rustdoc_args: Option<Vec<String>>,
    /// The specified target will be compiled with all the available arguments,
    /// note that this only accounts for the *final* invocation of rustc
    pub target_rustc_args: Option<Vec<String>>,
    /// Crate types to be passed to rustc (single target only)
    pub target_rustc_crate_types: Option<Vec<String>>,
    /// Whether the `--document-private-items` flags was specified and should
    /// be forwarded to `rustdoc`.
    pub rustdoc_document_private_items: bool,
    /// Whether the build process should check the minimum Rust version
    /// defined in the cargo metadata for a crate.
    pub honor_rust_version: bool,
}

impl CompileOptions {
    pub fn new(config: &Config, mode: CompileMode) -> CargoResult<CompileOptions> {
        let jobs = None;
        let keep_going = false;
        Ok(CompileOptions {
            build_config: BuildConfig::new(config, jobs, keep_going, &[], mode)?,
            cli_features: CliFeatures::new_all(false),
            spec: ops::Packages::Packages(Vec::new()),
            filter: CompileFilter::Default {
                required_features_filterable: false,
            },
            target_rustdoc_args: None,
            target_rustc_args: None,
            target_rustc_crate_types: None,
            rustdoc_document_private_items: false,
            honor_rust_version: true,
        })
    }
}

/// Compiles!
///
/// This uses the [`DefaultExecutor`]. To use a custom [`Executor`], see [`compile_with_exec`].
pub fn compile<'a>(ws: &Workspace<'a>, options: &CompileOptions) -> CargoResult<Compilation<'a>> {
    let exec: Arc<dyn Executor> = Arc::new(DefaultExecutor);
    compile_with_exec(ws, options, &exec)
}

/// Like [`compile`] but allows specifying a custom [`Executor`]
/// that will be able to intercept build calls and add custom logic.
///
/// [`compile`] uses [`DefaultExecutor`] which just passes calls through.
pub fn compile_with_exec<'a>(
    ws: &Workspace<'a>,
    options: &CompileOptions,
    exec: &Arc<dyn Executor>,
) -> CargoResult<Compilation<'a>> {
    ws.emit_warnings()?;
    compile_ws(ws, options, exec)
}

/// Like [`compile_with_exec`] but without warnings from manifest parsing.
pub fn compile_ws<'a>(
    ws: &Workspace<'a>,
    options: &CompileOptions,
    exec: &Arc<dyn Executor>,
) -> CargoResult<Compilation<'a>> {
    let interner = UnitInterner::new();
    let bcx = create_bcx(ws, options, &interner)?;
    if options.build_config.unit_graph {
        unit_graph::emit_serialized_unit_graph(&bcx.roots, &bcx.unit_graph, ws.config())?;
        return Compilation::new(&bcx);
    }
    let _p = profile::start("compiling");
    let cx = Context::new(&bcx)?;
    cx.compile(exec)
}

/// Executes `rustc --print <VALUE>`.
///
/// * `print_opt_value` is the VALUE passed through.
pub fn print<'a>(
    ws: &Workspace<'a>,
    options: &CompileOptions,
    print_opt_value: &str,
) -> CargoResult<()> {
    let CompileOptions {
        ref build_config,
        ref target_rustc_args,
        ..
    } = *options;
    let config = ws.config();
    let rustc = config.load_global_rustc(Some(ws))?;
    for (index, kind) in build_config.requested_kinds.iter().enumerate() {
        if index != 0 {
            drop_println!(config);
        }
        let target_info = TargetInfo::new(config, &build_config.requested_kinds, &rustc, *kind)?;
        let mut process = rustc.process();
        process.args(&target_info.rustflags);
        if let Some(args) = target_rustc_args {
            process.args(args);
        }
        if let CompileKind::Target(t) = kind {
            process.arg("--target").arg(t.rustc_target());
        }
        process.arg("--print").arg(print_opt_value);
        process.exec()?;
    }
    Ok(())
}

/// Prepares all required information for the actual compilation.
///
/// For how it works and what data it collects,
/// please see the [module-level documentation](self).
pub fn create_bcx<'a, 'cfg>(
    ws: &'a Workspace<'cfg>,
    options: &'a CompileOptions,
    interner: &'a UnitInterner,
) -> CargoResult<BuildContext<'a, 'cfg>> {
    let CompileOptions {
        ref build_config,
        ref spec,
        ref cli_features,
        ref filter,
        ref target_rustdoc_args,
        ref target_rustc_args,
        ref target_rustc_crate_types,
        rustdoc_document_private_items,
        honor_rust_version,
    } = *options;
    let config = ws.config();

    // Perform some pre-flight validation.
    match build_config.mode {
        CompileMode::Test
        | CompileMode::Build
        | CompileMode::Check { .. }
        | CompileMode::Bench
        | CompileMode::RunCustomBuild => {
            if ws.config().get_env("RUST_FLAGS").is_ok() {
                config.shell().warn(
                    "Cargo does not read `RUST_FLAGS` environment variable. Did you mean `RUSTFLAGS`?",
                )?;
            }
        }
        CompileMode::Doc { .. } | CompileMode::Doctest | CompileMode::Docscrape => {
            if ws.config().get_env("RUSTDOC_FLAGS").is_ok() {
                config.shell().warn(
                    "Cargo does not read `RUSTDOC_FLAGS` environment variable. Did you mean `RUSTDOCFLAGS`?"
                )?;
            }
        }
    }
    config.validate_term_config()?;

    let mut target_data = RustcTargetData::new(ws, &build_config.requested_kinds)?;

    let specs = spec.to_package_id_specs(ws)?;
    let has_dev_units = {
        // Rustdoc itself doesn't need dev-dependencies. But to scrape examples from packages in the
        // workspace, if any of those packages need dev-dependencies, then we need include dev-dependencies
        // to scrape those packages.
        let any_pkg_has_scrape_enabled = ws
            .members_with_features(&specs, cli_features)?
            .iter()
            .any(|(pkg, _)| {
                pkg.targets()
                    .iter()
                    .any(|target| target.is_example() && target.doc_scrape_examples().is_enabled())
            });

        if filter.need_dev_deps(build_config.mode)
            || (build_config.mode.is_doc() && any_pkg_has_scrape_enabled)
        {
            HasDevUnits::Yes
        } else {
            HasDevUnits::No
        }
    };
    let max_rust_version = ws.rust_version();
    let resolve = ops::resolve_ws_with_opts(
        ws,
        &mut target_data,
        &build_config.requested_kinds,
        cli_features,
        &specs,
        has_dev_units,
        crate::core::resolver::features::ForceAllTargets::No,
        max_rust_version,
    )?;
    let WorkspaceResolve {
        mut pkg_set,
        workspace_resolve,
        targeted_resolve: resolve,
        resolved_features,
    } = resolve;

    let std_resolve_features = if let Some(crates) = &config.cli_unstable().build_std {
        let (std_package_set, std_resolve, std_features) =
            standard_lib::resolve_std(ws, &mut target_data, &build_config, crates)?;
        pkg_set.add_set(std_package_set);
        Some((std_resolve, std_features))
    } else {
        None
    };

    // Find the packages in the resolver that the user wants to build (those
    // passed in with `-p` or the defaults from the workspace), and convert
    // Vec<PackageIdSpec> to a Vec<PackageId>.
    let to_build_ids = resolve.specs_to_ids(&specs)?;
    // Now get the `Package` for each `PackageId`. This may trigger a download
    // if the user specified `-p` for a dependency that is not downloaded.
    // Dependencies will be downloaded during build_unit_dependencies.
    let mut to_builds = pkg_set.get_many(to_build_ids)?;

    // The ordering here affects some error messages coming out of cargo, so
    // let's be test and CLI friendly by always printing in the same order if
    // there's an error.
    to_builds.sort_by_key(|p| p.package_id());

    for pkg in to_builds.iter() {
        pkg.manifest().print_teapot(config);

        if build_config.mode.is_any_test()
            && !ws.is_member(pkg)
            && pkg.dependencies().iter().any(|dep| !dep.is_transitive())
        {
            anyhow::bail!(
                "package `{}` cannot be tested because it requires dev-dependencies \
                 and is not a member of the workspace",
                pkg.name()
            );
        }
    }

    let (extra_args, extra_args_name) = match (target_rustc_args, target_rustdoc_args) {
        (Some(args), _) => (Some(args.clone()), "rustc"),
        (_, Some(args)) => (Some(args.clone()), "rustdoc"),
        _ => (None, ""),
    };

    if extra_args.is_some() && to_builds.len() != 1 {
        panic!(
            "`{}` should not accept multiple `-p` flags",
            extra_args_name
        );
    }

    let profiles = Profiles::new(ws, build_config.requested_profile)?;
    profiles.validate_packages(
        ws.profiles(),
        &mut config.shell(),
        workspace_resolve.as_ref().unwrap_or(&resolve),
    )?;

    // If `--target` has not been specified, then the unit graph is built
    // assuming `--target $HOST` was specified. See
    // `rebuild_unit_graph_shared` for more on why this is done.
    let explicit_host_kind = CompileKind::Target(CompileTarget::new(&target_data.rustc.host)?);
    let explicit_host_kinds: Vec<_> = build_config
        .requested_kinds
        .iter()
        .map(|kind| match kind {
            CompileKind::Host => explicit_host_kind,
            CompileKind::Target(t) => CompileKind::Target(*t),
        })
        .collect();

    // Passing `build_config.requested_kinds` instead of
    // `explicit_host_kinds` here so that `generate_root_units` can do
    // its own special handling of `CompileKind::Host`. It will
    // internally replace the host kind by the `explicit_host_kind`
    // before setting as a unit.
    let generator = UnitGenerator {
        ws,
        packages: &to_builds,
        filter,
        requested_kinds: &build_config.requested_kinds,
        explicit_host_kind,
        mode: build_config.mode,
        resolve: &resolve,
        workspace_resolve: &workspace_resolve,
        resolved_features: &resolved_features,
        package_set: &pkg_set,
        profiles: &profiles,
        interner,
        has_dev_units,
    };
    let mut units = generator.generate_root_units()?;

    if let Some(args) = target_rustc_crate_types {
        override_rustc_crate_types(&mut units, args, interner)?;
    }

    let should_scrape = build_config.mode.is_doc() && config.cli_unstable().rustdoc_scrape_examples;
    let mut scrape_units = if should_scrape {
        UnitGenerator {
            mode: CompileMode::Docscrape,
            ..generator
        }
        .generate_scrape_units(&units)?
    } else {
        Vec::new()
    };

    let std_roots = if let Some(crates) = standard_lib::std_crates(config, Some(&units)) {
        let (std_resolve, std_features) = std_resolve_features.as_ref().unwrap();
        standard_lib::generate_std_roots(
            &crates,
            std_resolve,
            std_features,
            &explicit_host_kinds,
            &pkg_set,
            interner,
            &profiles,
        )?
    } else {
        Default::default()
    };

    let mut unit_graph = build_unit_dependencies(
        ws,
        &pkg_set,
        &resolve,
        &resolved_features,
        std_resolve_features.as_ref(),
        &units,
        &scrape_units,
        &std_roots,
        build_config.mode,
        &target_data,
        &profiles,
        interner,
    )?;

    // TODO: In theory, Cargo should also dedupe the roots, but I'm uncertain
    // what heuristics to use in that case.
    if build_config.mode == (CompileMode::Doc { deps: true }) {
        remove_duplicate_doc(build_config, &units, &mut unit_graph);
    }

    let host_kind_requested = build_config
        .requested_kinds
        .iter()
        .any(CompileKind::is_host);
    let should_share_deps = host_kind_requested
        || config.cli_unstable().bindeps
            && unit_graph
                .iter()
                .any(|(unit, _)| unit.artifact_target_for_features.is_some());
    if should_share_deps {
        // Rebuild the unit graph, replacing the explicit host targets with
        // CompileKind::Host, removing `artifact_target_for_features` and merging any dependencies
        // shared with build and artifact dependencies.
        (units, scrape_units, unit_graph) = rebuild_unit_graph_shared(
            interner,
            unit_graph,
            &units,
            &scrape_units,
            host_kind_requested.then_some(explicit_host_kind),
        );
    }

    let mut extra_compiler_args = HashMap::new();
    if let Some(args) = extra_args {
        if units.len() != 1 {
            anyhow::bail!(
                "extra arguments to `{}` can only be passed to one \
                 target, consider filtering\nthe package by passing, \
                 e.g., `--lib` or `--bin NAME` to specify a single target",
                extra_args_name
            );
        }
        extra_compiler_args.insert(units[0].clone(), args);
    }

    for unit in units
        .iter()
        .filter(|unit| unit.mode.is_doc() || unit.mode.is_doc_test())
        .filter(|unit| rustdoc_document_private_items || unit.target.is_bin())
    {
        // Add `--document-private-items` rustdoc flag if requested or if
        // the target is a binary. Binary crates get their private items
        // documented by default.
        let mut args = vec!["--document-private-items".into()];
        if unit.target.is_bin() {
            // This warning only makes sense if it's possible to document private items
            // sometimes and ignore them at other times. But cargo consistently passes
            // `--document-private-items`, so the warning isn't useful.
            args.push("-Arustdoc::private-intra-doc-links".into());
        }
        extra_compiler_args
            .entry(unit.clone())
            .or_default()
            .extend(args);
    }

    if honor_rust_version {
        // Remove any pre-release identifiers for easier comparison
        let current_version = &target_data.rustc.version;
        let untagged_version = semver::Version::new(
            current_version.major,
            current_version.minor,
            current_version.patch,
        );

        for unit in unit_graph.keys() {
            let Some(version) = unit.pkg.rust_version() else {
                continue;
            };

            let req = version.caret_req();
            if req.matches(&untagged_version) {
                continue;
            }

            let guidance = if ws.is_ephemeral() {
                if ws.ignore_lock() {
                    "Try re-running cargo install with `--locked`".to_string()
                } else {
                    String::new()
                }
            } else if !unit.is_local() {
                format!(
                    "Either upgrade to rustc {} or newer, or use\n\
                     cargo update {}@{} --precise ver\n\
                     where `ver` is the latest version of `{}` supporting rustc {}",
                    version,
                    unit.pkg.name(),
                    unit.pkg.version(),
                    unit.pkg.name(),
                    current_version,
                )
            } else {
                String::new()
            };

            anyhow::bail!(
                "package `{}` cannot be built because it requires rustc {} or newer, \
                 while the currently active rustc version is {}\n{}",
                unit.pkg,
                version,
                current_version,
                guidance,
            );
        }
    }

    let bcx = BuildContext::new(
        ws,
        pkg_set,
        build_config,
        profiles,
        extra_compiler_args,
        target_data,
        units,
        unit_graph,
        scrape_units,
    )?;

    Ok(bcx)
}

/// This is used to rebuild the unit graph, sharing host dependencies if possible.
///
/// This will translate any unit's `CompileKind::Target(host)` to
/// `CompileKind::Host` if `to_host` is not `None` and the kind is equal to `to_host`.
/// This also handles generating the unit `dep_hash`, and merging shared units if possible.
///
/// This is necessary because if normal dependencies used `CompileKind::Host`,
/// there would be no way to distinguish those units from build-dependency
/// units or artifact dependency units.
/// This can cause a problem if a shared normal/build/artifact dependency needs
/// to link to another dependency whose features differ based on whether or
/// not it is a normal, build or artifact dependency. If all units used
/// `CompileKind::Host`, then they would end up being identical, causing a
/// collision in the `UnitGraph`, and Cargo would end up randomly choosing one
/// value or the other.
///
/// The solution is to keep normal, build and artifact dependencies separate when
/// building the unit graph, and then run this second pass which will try to
/// combine shared dependencies safely. By adding a hash of the dependencies
/// to the `Unit`, this allows the `CompileKind` to be changed back to `Host`
/// and `artifact_target_for_features` to be removed without fear of an unwanted
/// collision for build or artifact dependencies.
fn rebuild_unit_graph_shared(
    interner: &UnitInterner,
    unit_graph: UnitGraph,
    roots: &[Unit],
    scrape_units: &[Unit],
    to_host: Option<CompileKind>,
) -> (Vec<Unit>, Vec<Unit>, UnitGraph) {
    let mut result = UnitGraph::new();
    // Map of the old unit to the new unit, used to avoid recursing into units
    // that have already been computed to improve performance.
    let mut memo = HashMap::new();
    let new_roots = roots
        .iter()
        .map(|root| {
            traverse_and_share(
                interner,
                &mut memo,
                &mut result,
                &unit_graph,
                root,
                false,
                to_host,
            )
        })
        .collect();
    // If no unit in the unit graph ended up having scrape units attached as dependencies,
    // then they won't have been discovered in traverse_and_share and hence won't be in
    // memo. So we filter out missing scrape units.
    let new_scrape_units = scrape_units
        .iter()
        .map(|unit| memo.get(unit).unwrap().clone())
        .collect();
    (new_roots, new_scrape_units, result)
}

/// Recursive function for rebuilding the graph.
///
/// This walks `unit_graph`, starting at the given `unit`. It inserts the new
/// units into `new_graph`, and returns a new updated version of the given
/// unit (`dep_hash` is filled in, and `kind` switched if necessary).
fn traverse_and_share(
    interner: &UnitInterner,
    memo: &mut HashMap<Unit, Unit>,
    new_graph: &mut UnitGraph,
    unit_graph: &UnitGraph,
    unit: &Unit,
    unit_is_for_host: bool,
    to_host: Option<CompileKind>,
) -> Unit {
    if let Some(new_unit) = memo.get(unit) {
        // Already computed, no need to recompute.
        return new_unit.clone();
    }
    let mut dep_hash = StableHasher::new();
    let new_deps: Vec<_> = unit_graph[unit]
        .iter()
        .map(|dep| {
            let new_dep_unit = traverse_and_share(
                interner,
                memo,
                new_graph,
                unit_graph,
                &dep.unit,
                dep.unit_for.is_for_host(),
                to_host,
            );
            new_dep_unit.hash(&mut dep_hash);
            UnitDep {
                unit: new_dep_unit,
                ..dep.clone()
            }
        })
        .collect();
    // Here, we have recursively traversed this unit's dependencies, and hashed them: we can
    // finalize the dep hash.
    let new_dep_hash = dep_hash.finish();

    // This is the key part of the sharing process: if the unit is a runtime dependency, whose
    // target is the same as the host, we canonicalize the compile kind to `CompileKind::Host`.
    // A possible host dependency counterpart to this unit would have that kind, and if such a unit
    // exists in the current `unit_graph`, they will unify in the new unit graph map `new_graph`.
    // The resulting unit graph will be optimized with less units, thanks to sharing these host
    // dependencies.
    let canonical_kind = match to_host {
        Some(to_host) if to_host == unit.kind => CompileKind::Host,
        _ => unit.kind,
    };

    let mut profile = unit.profile.clone();

    // If this is a build dependency, and it's not shared with runtime dependencies, we can weaken
    // its debuginfo level to optimize build times. We do nothing if it's an artifact dependency,
    // as it and its debuginfo may end up embedded in the main program.
    if unit_is_for_host
        && to_host.is_some()
        && profile.debuginfo.is_deferred()
        && !unit.artifact.is_true()
    {
        // We create a "probe" test to see if a unit with the same explicit debuginfo level exists
        // in the graph. This is the level we'd expect if it was set manually or the default value
        // set by a profile for a runtime dependency: its canonical value.
        let canonical_debuginfo = profile.debuginfo.finalize();
        let mut canonical_profile = profile.clone();
        canonical_profile.debuginfo = canonical_debuginfo;
        let unit_probe = interner.intern(
            &unit.pkg,
            &unit.target,
            canonical_profile,
            to_host.unwrap(),
            unit.mode,
            unit.features.clone(),
            unit.is_std,
            unit.dep_hash,
            unit.artifact,
            unit.artifact_target_for_features,
        );

        // We can now turn the deferred value into its actual final value.
        profile.debuginfo = if unit_graph.contains_key(&unit_probe) {
            // The unit is present in both build time and runtime subgraphs: we canonicalize its
            // level to the other unit's, thus ensuring reuse between the two to optimize build times.
            canonical_debuginfo
        } else {
            // The unit is only present in the build time subgraph, we can weaken its debuginfo
            // level to optimize build times.
            canonical_debuginfo.weaken()
        }
    }

    let new_unit = interner.intern(
        &unit.pkg,
        &unit.target,
        profile,
        canonical_kind,
        unit.mode,
        unit.features.clone(),
        unit.is_std,
        new_dep_hash,
        unit.artifact,
        // Since `dep_hash` is now filled in, there's no need to specify the artifact target
        // for target-dependent feature resolution
        None,
    );
    assert!(memo.insert(unit.clone(), new_unit.clone()).is_none());
    new_graph.entry(new_unit.clone()).or_insert(new_deps);
    new_unit
}

/// Removes duplicate CompileMode::Doc units that would cause problems with
/// filename collisions.
///
/// Rustdoc only separates units by crate name in the file directory
/// structure. If any two units with the same crate name exist, this would
/// cause a filename collision, causing different rustdoc invocations to stomp
/// on one another's files.
///
/// Unfortunately this does not remove all duplicates, as some of them are
/// either user error, or difficult to remove. Cases that I can think of:
///
/// - Same target name in different packages. See the `collision_doc` test.
/// - Different sources. See `collision_doc_sources` test.
///
/// Ideally this would not be necessary.
fn remove_duplicate_doc(
    build_config: &BuildConfig,
    root_units: &[Unit],
    unit_graph: &mut UnitGraph,
) {
    // First, create a mapping of crate_name -> Unit so we can see where the
    // duplicates are.
    let mut all_docs: HashMap<String, Vec<Unit>> = HashMap::new();
    for unit in unit_graph.keys() {
        if unit.mode.is_doc() {
            all_docs
                .entry(unit.target.crate_name())
                .or_default()
                .push(unit.clone());
        }
    }
    // Keep track of units to remove so that they can be efficiently removed
    // from the unit_deps.
    let mut removed_units: HashSet<Unit> = HashSet::new();
    let mut remove = |units: Vec<Unit>, reason: &str, cb: &dyn Fn(&Unit) -> bool| -> Vec<Unit> {
        let (to_remove, remaining_units): (Vec<Unit>, Vec<Unit>) = units
            .into_iter()
            .partition(|unit| cb(unit) && !root_units.contains(unit));
        for unit in to_remove {
            tracing::debug!(
                "removing duplicate doc due to {} for package {} target `{}`",
                reason,
                unit.pkg,
                unit.target.name()
            );
            unit_graph.remove(&unit);
            removed_units.insert(unit);
        }
        remaining_units
    };
    // Iterate over the duplicates and try to remove them from unit_graph.
    for (_crate_name, mut units) in all_docs {
        if units.len() == 1 {
            continue;
        }
        // Prefer target over host if --target was not specified.
        if build_config
            .requested_kinds
            .iter()
            .all(CompileKind::is_host)
        {
            // Note these duplicates may not be real duplicates, since they
            // might get merged in rebuild_unit_graph_shared. Either way, it
            // shouldn't hurt to remove them early (although the report in the
            // log might be confusing).
            units = remove(units, "host/target merger", &|unit| unit.kind.is_host());
            if units.len() == 1 {
                continue;
            }
        }
        // Prefer newer versions over older.
        let mut source_map: HashMap<(InternedString, SourceId, CompileKind), Vec<Unit>> =
            HashMap::new();
        for unit in units {
            let pkg_id = unit.pkg.package_id();
            // Note, this does not detect duplicates from different sources.
            source_map
                .entry((pkg_id.name(), pkg_id.source_id(), unit.kind))
                .or_default()
                .push(unit);
        }
        let mut remaining_units = Vec::new();
        for (_key, mut units) in source_map {
            if units.len() > 1 {
                units.sort_by(|a, b| a.pkg.version().partial_cmp(b.pkg.version()).unwrap());
                // Remove any entries with version < newest.
                let newest_version = units.last().unwrap().pkg.version().clone();
                let keep_units = remove(units, "older version", &|unit| {
                    unit.pkg.version() < &newest_version
                });
                remaining_units.extend(keep_units);
            } else {
                remaining_units.extend(units);
            }
        }
        if remaining_units.len() == 1 {
            continue;
        }
        // Are there other heuristics to remove duplicates that would make
        // sense? Maybe prefer path sources over all others?
    }
    // Also remove units from the unit_deps so there aren't any dangling edges.
    for unit_deps in unit_graph.values_mut() {
        unit_deps.retain(|unit_dep| !removed_units.contains(&unit_dep.unit));
    }
    // Remove any orphan units that were detached from the graph.
    let mut visited = HashSet::new();
    fn visit(unit: &Unit, graph: &UnitGraph, visited: &mut HashSet<Unit>) {
        if !visited.insert(unit.clone()) {
            return;
        }
        for dep in &graph[unit] {
            visit(&dep.unit, graph, visited);
        }
    }
    for unit in root_units {
        visit(unit, unit_graph, &mut visited);
    }
    unit_graph.retain(|unit, _| visited.contains(unit));
}

/// Override crate types for given units.
///
/// This is primarily used by `cargo rustc --crate-type`.
fn override_rustc_crate_types(
    units: &mut [Unit],
    args: &[String],
    interner: &UnitInterner,
) -> CargoResult<()> {
    if units.len() != 1 {
        anyhow::bail!(
            "crate types to rustc can only be passed to one \
            target, consider filtering\nthe package by passing, \
            e.g., `--lib` or `--example` to specify a single target"
        );
    }

    let unit = &units[0];
    let override_unit = |f: fn(Vec<CrateType>) -> TargetKind| {
        let crate_types = args.iter().map(|s| s.into()).collect();
        let mut target = unit.target.clone();
        target.set_kind(f(crate_types));
        interner.intern(
            &unit.pkg,
            &target,
            unit.profile.clone(),
            unit.kind,
            unit.mode,
            unit.features.clone(),
            unit.is_std,
            unit.dep_hash,
            unit.artifact,
            unit.artifact_target_for_features,
        )
    };
    units[0] = match unit.target.kind() {
        TargetKind::Lib(_) => override_unit(TargetKind::Lib),
        TargetKind::ExampleLib(_) => override_unit(TargetKind::ExampleLib),
        _ => {
            anyhow::bail!(
                "crate types can only be specified for libraries and example libraries.\n\
                Binaries, tests, and benchmarks are always the `bin` crate type"
            );
        }
    };

    Ok(())
}

/// Gets all of the features enabled for a package, plus its dependencies'
/// features.
///
/// Dependencies are added as `dep_name/feat_name` because `required-features`
/// wants to support that syntax.
pub fn resolve_all_features(
    resolve_with_overrides: &Resolve,
    resolved_features: &features::ResolvedFeatures,
    package_set: &PackageSet<'_>,
    package_id: PackageId,
) -> HashSet<String> {
    let mut features: HashSet<String> = resolved_features
        .activated_features(package_id, FeaturesFor::NormalOrDev)
        .iter()
        .map(|s| s.to_string())
        .collect();

    // Include features enabled for use by dependencies so targets can also use them with the
    // required-features field when deciding whether to be built or skipped.
    for (dep_id, deps) in resolve_with_overrides.deps(package_id) {
        let is_proc_macro = package_set
            .get_one(dep_id)
            .expect("packages downloaded")
            .proc_macro();
        for dep in deps {
            let features_for = FeaturesFor::from_for_host(is_proc_macro || dep.is_build());
            for feature in resolved_features
                .activated_features_unverified(dep_id, features_for)
                .unwrap_or_default()
            {
                features.insert(format!("{}/{}", dep.name_in_toml(), feature));
            }
        }
    }

    features
}
