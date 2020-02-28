//! The Cargo "compile" operation.
//!
//! This module contains the entry point for starting the compilation process
//! for commands like `build`, `test`, `doc`, `rustc`, etc.
//!
//! The `compile` function will do all the work to compile a workspace. A
//! rough outline is:
//!
//! - Resolve the dependency graph (see `ops::resolve`).
//! - Download any packages needed (see `PackageSet`).
//! - Generate a list of top-level "units" of work for the targets the user
//!   requested on the command-line. Each `Unit` corresponds to a compiler
//!   invocation. This is done in this module (`generate_targets`).
//! - Build the graph of `Unit` dependencies (see
//!   `core::compiler::context::unit_dependencies`).
//! - Create a `Context` which will perform the following steps:
//!     - Prepare the `target` directory (see `Layout`).
//!     - Create a job queue (see `JobQueue`). The queue checks the
//!       fingerprint of each `Unit` to determine if it should run or be
//!       skipped.
//!     - Execute the queue. Each leaf in the queue's dependency graph is
//!       executed, and then removed from the graph when finished. This
//!       repeats until the queue is empty.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::iter::FromIterator;
use std::sync::Arc;

use crate::core::compiler::unit_dependencies::build_unit_dependencies;
use crate::core::compiler::{standard_lib, unit_graph};
use crate::core::compiler::{BuildConfig, BuildContext, Compilation, Context};
use crate::core::compiler::{CompileKind, CompileMode, RustcTargetData, Unit};
use crate::core::compiler::{DefaultExecutor, Executor, UnitInterner};
use crate::core::profiles::{Profiles, UnitFor};
use crate::core::resolver::features::{self, FeaturesFor};
use crate::core::resolver::{HasDevUnits, Resolve, ResolveOpts};
use crate::core::{FeatureValue, Package, PackageSet, Shell, Summary, Target};
use crate::core::{PackageId, PackageIdSpec, TargetKind, Workspace};
use crate::ops;
use crate::ops::resolve::WorkspaceResolve;
use crate::util::config::Config;
use crate::util::{closest_msg, profile, CargoResult};

/// Contains information about how a package should be compiled.
///
/// Note on distinction between `CompileOptions` and `BuildConfig`:
/// `BuildConfig` contains values that need to be retained after
/// `BuildContext` is created. The other fields are no longer necessary. Think
/// of it as `CompileOptions` are high-level settings requested on the
/// command-line, and `BuildConfig` are low-level settings for actually
/// driving `rustc`.
#[derive(Debug)]
pub struct CompileOptions {
    /// Configuration information for a rustc build
    pub build_config: BuildConfig,
    /// Extra features to build for the root package
    pub features: Vec<String>,
    /// Flag whether all available features should be built for the root package
    pub all_features: bool,
    /// Flag if the default feature should be built for the root package
    pub no_default_features: bool,
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
    /// Extra arguments passed to all selected targets for rustdoc.
    pub local_rustdoc_args: Option<Vec<String>>,
    /// Whether the `--document-private-items` flags was specified and should
    /// be forwarded to `rustdoc`.
    pub rustdoc_document_private_items: bool,
}

impl<'a> CompileOptions {
    pub fn new(config: &Config, mode: CompileMode) -> CargoResult<CompileOptions> {
        Ok(CompileOptions {
            build_config: BuildConfig::new(config, None, &[], mode)?,
            features: Vec::new(),
            all_features: false,
            no_default_features: false,
            spec: ops::Packages::Packages(Vec::new()),
            filter: CompileFilter::Default {
                required_features_filterable: false,
            },
            target_rustdoc_args: None,
            target_rustc_args: None,
            local_rustdoc_args: None,
            rustdoc_document_private_items: false,
        })
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Packages {
    Default,
    All,
    OptOut(Vec<String>),
    Packages(Vec<String>),
}

impl Packages {
    pub fn from_flags(all: bool, exclude: Vec<String>, package: Vec<String>) -> CargoResult<Self> {
        Ok(match (all, exclude.len(), package.len()) {
            (false, 0, 0) => Packages::Default,
            (false, 0, _) => Packages::Packages(package),
            (false, _, _) => anyhow::bail!("--exclude can only be used together with --workspace"),
            (true, 0, _) => Packages::All,
            (true, _, _) => Packages::OptOut(exclude),
        })
    }

    pub fn to_package_id_specs(&self, ws: &Workspace<'_>) -> CargoResult<Vec<PackageIdSpec>> {
        let specs = match self {
            Packages::All => ws
                .members()
                .map(Package::package_id)
                .map(PackageIdSpec::from_package_id)
                .collect(),
            Packages::OptOut(opt_out) => {
                let mut opt_out = BTreeSet::from_iter(opt_out.iter().cloned());
                let packages = ws
                    .members()
                    .filter(|pkg| !opt_out.remove(pkg.name().as_str()))
                    .map(Package::package_id)
                    .map(PackageIdSpec::from_package_id)
                    .collect();
                if !opt_out.is_empty() {
                    ws.config().shell().warn(format!(
                        "excluded package(s) {} not found in workspace `{}`",
                        opt_out
                            .iter()
                            .map(|x| x.as_ref())
                            .collect::<Vec<_>>()
                            .join(", "),
                        ws.root().display(),
                    ))?;
                }
                packages
            }
            Packages::Packages(packages) if packages.is_empty() => {
                vec![PackageIdSpec::from_package_id(ws.current()?.package_id())]
            }
            Packages::Packages(packages) => packages
                .iter()
                .map(|p| PackageIdSpec::parse(p))
                .collect::<CargoResult<Vec<_>>>()?,
            Packages::Default => ws
                .default_members()
                .map(Package::package_id)
                .map(PackageIdSpec::from_package_id)
                .collect(),
        };
        if specs.is_empty() {
            if ws.is_virtual() {
                anyhow::bail!(
                    "manifest path `{}` contains no package: The manifest is virtual, \
                     and the workspace has no members.",
                    ws.root().display()
                )
            }
            anyhow::bail!("no packages to compile")
        }
        Ok(specs)
    }

    pub fn get_packages<'ws>(&self, ws: &'ws Workspace<'_>) -> CargoResult<Vec<&'ws Package>> {
        let packages: Vec<_> = match self {
            Packages::Default => ws.default_members().collect(),
            Packages::All => ws.members().collect(),
            Packages::OptOut(opt_out) => ws
                .members()
                .filter(|pkg| !opt_out.iter().any(|name| pkg.name().as_str() == name))
                .collect(),
            Packages::Packages(packages) => packages
                .iter()
                .map(|name| {
                    ws.members()
                        .find(|pkg| pkg.name().as_str() == name)
                        .ok_or_else(|| {
                            anyhow::format_err!(
                                "package `{}` is not a member of the workspace",
                                name
                            )
                        })
                })
                .collect::<CargoResult<Vec<_>>>()?,
        };
        Ok(packages)
    }

    /// Returns whether or not the user needs to pass a `-p` flag to target a
    /// specific package in the workspace.
    pub fn needs_spec_flag(&self, ws: &Workspace<'_>) -> bool {
        match self {
            Packages::Default => ws.default_members().count() > 1,
            Packages::All => ws.members().count() > 1,
            Packages::Packages(_) => true,
            Packages::OptOut(_) => true,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum LibRule {
    /// Include the library, fail if not present
    True,
    /// Include the library if present
    Default,
    /// Exclude the library
    False,
}

#[derive(Debug)]
pub enum FilterRule {
    All,
    Just(Vec<String>),
}

#[derive(Debug)]
pub enum CompileFilter {
    Default {
        /// Flag whether targets can be safely skipped when required-features are not satisfied.
        required_features_filterable: bool,
    },
    Only {
        all_targets: bool,
        lib: LibRule,
        bins: FilterRule,
        examples: FilterRule,
        tests: FilterRule,
        benches: FilterRule,
    },
}

pub fn compile<'a>(ws: &Workspace<'a>, options: &CompileOptions) -> CargoResult<Compilation<'a>> {
    let exec: Arc<dyn Executor> = Arc::new(DefaultExecutor);
    compile_with_exec(ws, options, &exec)
}

/// Like `compile` but allows specifying a custom `Executor` that will be able to intercept build
/// calls and add custom logic. `compile` uses `DefaultExecutor` which just passes calls through.
pub fn compile_with_exec<'a>(
    ws: &Workspace<'a>,
    options: &CompileOptions,
    exec: &Arc<dyn Executor>,
) -> CargoResult<Compilation<'a>> {
    ws.emit_warnings()?;
    compile_ws(ws, options, exec)
}

pub fn compile_ws<'a>(
    ws: &Workspace<'a>,
    options: &CompileOptions,
    exec: &Arc<dyn Executor>,
) -> CargoResult<Compilation<'a>> {
    let interner = UnitInterner::new();
    let bcx = create_bcx(ws, options, &interner)?;
    if options.build_config.unit_graph {
        unit_graph::emit_serialized_unit_graph(&bcx.roots, &bcx.unit_graph)?;
        return Ok(Compilation::new(&bcx)?);
    }

    let _p = profile::start("compiling");
    let cx = Context::new(&bcx)?;
    cx.compile(exec)
}

pub fn create_bcx<'a, 'cfg>(
    ws: &'a Workspace<'cfg>,
    options: &'a CompileOptions,
    interner: &'a UnitInterner,
) -> CargoResult<BuildContext<'a, 'cfg>> {
    let CompileOptions {
        ref build_config,
        ref spec,
        ref features,
        all_features,
        no_default_features,
        ref filter,
        ref target_rustdoc_args,
        ref target_rustc_args,
        ref local_rustdoc_args,
        rustdoc_document_private_items,
    } = *options;
    let config = ws.config();

    match build_config.mode {
        CompileMode::Test
        | CompileMode::Build
        | CompileMode::Check { .. }
        | CompileMode::Bench
        | CompileMode::RunCustomBuild => {
            if std::env::var("RUST_FLAGS").is_ok() {
                config.shell().warn(
                    "Cargo does not read `RUST_FLAGS` environment variable. Did you mean `RUSTFLAGS`?",
                )?;
            }
        }
        CompileMode::Doc { .. } | CompileMode::Doctest => {
            if std::env::var("RUSTDOC_FLAGS").is_ok() {
                config.shell().warn(
                    "Cargo does not read `RUSTDOC_FLAGS` environment variable. Did you mean `RUSTDOCFLAGS`?"
                )?;
            }
        }
    }

    let target_data = RustcTargetData::new(ws, &build_config.requested_kinds)?;

    let specs = spec.to_package_id_specs(ws)?;
    let dev_deps = ws.require_optional_deps() || filter.need_dev_deps(build_config.mode);
    let opts = ResolveOpts::new(dev_deps, features, all_features, !no_default_features);
    let has_dev_units = if filter.need_dev_deps(build_config.mode) {
        HasDevUnits::Yes
    } else {
        HasDevUnits::No
    };
    let resolve = ops::resolve_ws_with_opts(
        ws,
        &target_data,
        &build_config.requested_kinds,
        &opts,
        &specs,
        has_dev_units,
        crate::core::resolver::features::ForceAllTargets::No,
    )?;
    let WorkspaceResolve {
        mut pkg_set,
        workspace_resolve,
        targeted_resolve: resolve,
        resolved_features,
    } = resolve;

    let std_resolve_features = if let Some(crates) = &config.cli_unstable().build_std {
        if build_config.build_plan {
            config
                .shell()
                .warn("-Zbuild-std does not currently fully support --build-plan")?;
        }
        if build_config.requested_kinds[0].is_host() {
            // TODO: This should eventually be fixed. Unfortunately it is not
            // easy to get the host triple in BuildConfig. Consider changing
            // requested_target to an enum, or some other approach.
            anyhow::bail!("-Zbuild-std requires --target");
        }
        let (std_package_set, std_resolve, std_features) =
            standard_lib::resolve_std(ws, &target_data, &build_config.requested_kinds, crates)?;
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
        (&Some(ref args), _) => (Some(args.clone()), "rustc"),
        (_, &Some(ref args)) => (Some(args.clone()), "rustdoc"),
        _ => (None, ""),
    };

    if extra_args.is_some() && to_builds.len() != 1 {
        panic!(
            "`{}` should not accept multiple `-p` flags",
            extra_args_name
        );
    }

    let profiles = Profiles::new(
        ws.profiles(),
        config,
        build_config.requested_profile,
        ws.features(),
    )?;
    profiles.validate_packages(
        ws.profiles(),
        &mut config.shell(),
        workspace_resolve.as_ref().unwrap_or(&resolve),
    )?;

    let units = generate_targets(
        ws,
        &to_builds,
        filter,
        &build_config.requested_kinds,
        build_config.mode,
        &resolve,
        &workspace_resolve,
        &resolved_features,
        &pkg_set,
        &profiles,
        interner,
    )?;

    let std_roots = if let Some(crates) = &config.cli_unstable().build_std {
        // Only build libtest if it looks like it is needed.
        let mut crates = crates.clone();
        if !crates.iter().any(|c| c == "test")
            && units
                .iter()
                .any(|unit| unit.mode.is_rustc_test() && unit.target.harness())
        {
            // Only build libtest when libstd is built (libtest depends on libstd)
            if crates.iter().any(|c| c == "std") {
                crates.push("test".to_string());
            }
        }
        let (std_resolve, std_features) = std_resolve_features.as_ref().unwrap();
        standard_lib::generate_std_roots(
            &crates,
            std_resolve,
            std_features,
            &build_config.requested_kinds,
            &pkg_set,
            interner,
            &profiles,
        )?
    } else {
        Default::default()
    };

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
    for unit in &units {
        if unit.mode.is_doc() || unit.mode.is_doc_test() {
            let mut extra_args = local_rustdoc_args.clone();

            // Add `--document-private-items` rustdoc flag if requested or if
            // the target is a binary. Binary crates get their private items
            // documented by default.
            if rustdoc_document_private_items || unit.target.is_bin() {
                let mut args = extra_args.take().unwrap_or_else(|| vec![]);
                args.push("--document-private-items".into());
                extra_args = Some(args);
            }

            if let Some(args) = extra_args {
                extra_compiler_args
                    .entry(unit.clone())
                    .or_default()
                    .extend(args);
            }
        }
    }

    let unit_graph = build_unit_dependencies(
        ws,
        &pkg_set,
        &resolve,
        &resolved_features,
        std_resolve_features.as_ref(),
        &units,
        &std_roots,
        build_config.mode,
        &target_data,
        &profiles,
        interner,
    )?;

    let bcx = BuildContext::new(
        ws,
        pkg_set,
        build_config,
        profiles,
        extra_compiler_args,
        target_data,
        units,
        unit_graph,
    )?;

    Ok(bcx)
}

impl FilterRule {
    pub fn new(targets: Vec<String>, all: bool) -> FilterRule {
        if all {
            FilterRule::All
        } else {
            FilterRule::Just(targets)
        }
    }

    pub fn none() -> FilterRule {
        FilterRule::Just(Vec::new())
    }

    fn matches(&self, target: &Target) -> bool {
        match *self {
            FilterRule::All => true,
            FilterRule::Just(ref targets) => targets.iter().any(|x| *x == target.name()),
        }
    }

    fn is_specific(&self) -> bool {
        match *self {
            FilterRule::All => true,
            FilterRule::Just(ref targets) => !targets.is_empty(),
        }
    }

    pub fn try_collect(&self) -> Option<Vec<String>> {
        match *self {
            FilterRule::All => None,
            FilterRule::Just(ref targets) => Some(targets.clone()),
        }
    }
}

impl CompileFilter {
    /// Construct a CompileFilter from raw command line arguments.
    pub fn from_raw_arguments(
        lib_only: bool,
        bins: Vec<String>,
        all_bins: bool,
        tsts: Vec<String>,
        all_tsts: bool,
        exms: Vec<String>,
        all_exms: bool,
        bens: Vec<String>,
        all_bens: bool,
        all_targets: bool,
    ) -> CompileFilter {
        if all_targets {
            return CompileFilter::new_all_targets();
        }
        let rule_lib = if lib_only {
            LibRule::True
        } else {
            LibRule::False
        };
        let rule_bins = FilterRule::new(bins, all_bins);
        let rule_tsts = FilterRule::new(tsts, all_tsts);
        let rule_exms = FilterRule::new(exms, all_exms);
        let rule_bens = FilterRule::new(bens, all_bens);

        CompileFilter::new(rule_lib, rule_bins, rule_tsts, rule_exms, rule_bens)
    }

    /// Construct a CompileFilter from underlying primitives.
    pub fn new(
        rule_lib: LibRule,
        rule_bins: FilterRule,
        rule_tsts: FilterRule,
        rule_exms: FilterRule,
        rule_bens: FilterRule,
    ) -> CompileFilter {
        if rule_lib == LibRule::True
            || rule_bins.is_specific()
            || rule_tsts.is_specific()
            || rule_exms.is_specific()
            || rule_bens.is_specific()
        {
            CompileFilter::Only {
                all_targets: false,
                lib: rule_lib,
                bins: rule_bins,
                examples: rule_exms,
                benches: rule_bens,
                tests: rule_tsts,
            }
        } else {
            CompileFilter::Default {
                required_features_filterable: true,
            }
        }
    }

    pub fn new_all_targets() -> CompileFilter {
        CompileFilter::Only {
            all_targets: true,
            lib: LibRule::Default,
            bins: FilterRule::All,
            examples: FilterRule::All,
            benches: FilterRule::All,
            tests: FilterRule::All,
        }
    }

    pub fn need_dev_deps(&self, mode: CompileMode) -> bool {
        match mode {
            CompileMode::Test | CompileMode::Doctest | CompileMode::Bench => true,
            CompileMode::Check { test: true } => true,
            CompileMode::Build | CompileMode::Doc { .. } | CompileMode::Check { test: false } => {
                match *self {
                    CompileFilter::Default { .. } => false,
                    CompileFilter::Only {
                        ref examples,
                        ref tests,
                        ref benches,
                        ..
                    } => examples.is_specific() || tests.is_specific() || benches.is_specific(),
                }
            }
            CompileMode::RunCustomBuild => panic!("Invalid mode"),
        }
    }

    // this selects targets for "cargo run". for logic to select targets for
    // other subcommands, see generate_targets and filter_default_targets
    pub fn target_run(&self, target: &Target) -> bool {
        match *self {
            CompileFilter::Default { .. } => true,
            CompileFilter::Only {
                ref lib,
                ref bins,
                ref examples,
                ref tests,
                ref benches,
                ..
            } => {
                let rule = match *target.kind() {
                    TargetKind::Bin => bins,
                    TargetKind::Test => tests,
                    TargetKind::Bench => benches,
                    TargetKind::ExampleBin | TargetKind::ExampleLib(..) => examples,
                    TargetKind::Lib(..) => {
                        return match *lib {
                            LibRule::True => true,
                            LibRule::Default => true,
                            LibRule::False => false,
                        };
                    }
                    TargetKind::CustomBuild => return false,
                };
                rule.matches(target)
            }
        }
    }

    pub fn is_specific(&self) -> bool {
        match *self {
            CompileFilter::Default { .. } => false,
            CompileFilter::Only { .. } => true,
        }
    }
}

/// A proposed target.
///
/// Proposed targets are later filtered into actual `Unit`s based on whether or
/// not the target requires its features to be present.
#[derive(Debug)]
struct Proposal<'a> {
    pkg: &'a Package,
    target: &'a Target,
    /// Indicates whether or not all required features *must* be present. If
    /// false, and the features are not available, then it will be silently
    /// skipped. Generally, targets specified by name (`--bin foo`) are
    /// required, all others can be silently skipped if features are missing.
    requires_features: bool,
    mode: CompileMode,
}

/// Generates all the base targets for the packages the user has requested to
/// compile. Dependencies for these targets are computed later in `unit_dependencies`.
fn generate_targets(
    ws: &Workspace<'_>,
    packages: &[&Package],
    filter: &CompileFilter,
    requested_kinds: &[CompileKind],
    mode: CompileMode,
    resolve: &Resolve,
    workspace_resolve: &Option<Resolve>,
    resolved_features: &features::ResolvedFeatures,
    package_set: &PackageSet<'_>,
    profiles: &Profiles,
    interner: &UnitInterner,
) -> CargoResult<Vec<Unit>> {
    let config = ws.config();
    // Helper for creating a list of `Unit` structures
    let new_unit =
        |units: &mut HashSet<Unit>, pkg: &Package, target: &Target, target_mode: CompileMode| {
            let unit_for = if target_mode.is_any_test() {
                // NOTE: the `UnitFor` here is subtle. If you have a profile
                // with `panic` set, the `panic` flag is cleared for
                // tests/benchmarks and their dependencies. If this
                // was `normal`, then the lib would get compiled three
                // times (once with panic, once without, and once with
                // `--test`).
                //
                // This would cause a problem for doc tests, which would fail
                // because `rustdoc` would attempt to link with both libraries
                // at the same time. Also, it's probably not important (or
                // even desirable?) for rustdoc to link with a lib with
                // `panic` set.
                //
                // As a consequence, Examples and Binaries get compiled
                // without `panic` set. This probably isn't a bad deal.
                //
                // Forcing the lib to be compiled three times during `cargo
                // test` is probably also not desirable.
                UnitFor::new_test(config)
            } else if target.for_host() {
                // Proc macro / plugin should not have `panic` set.
                UnitFor::new_compiler()
            } else {
                UnitFor::new_normal()
            };
            // Custom build units are added in `build_unit_dependencies`.
            assert!(!target.is_custom_build());
            let target_mode = match target_mode {
                CompileMode::Test => {
                    if target.is_example() && !filter.is_specific() && !target.tested() {
                        // Examples are included as regular binaries to verify
                        // that they compile.
                        CompileMode::Build
                    } else {
                        CompileMode::Test
                    }
                }
                CompileMode::Build => match *target.kind() {
                    TargetKind::Test => CompileMode::Test,
                    TargetKind::Bench => CompileMode::Bench,
                    _ => CompileMode::Build,
                },
                // `CompileMode::Bench` is only used to inform `filter_default_targets`
                // which command is being used (`cargo bench`). Afterwards, tests
                // and benches are treated identically. Switching the mode allows
                // de-duplication of units that are essentially identical. For
                // example, `cargo build --all-targets --release` creates the units
                // (lib profile:bench, mode:test) and (lib profile:bench, mode:bench)
                // and since these are the same, we want them to be de-duplicated in
                // `unit_dependencies`.
                CompileMode::Bench => CompileMode::Test,
                _ => target_mode,
            };

            let is_local = pkg.package_id().source_id().is_path();
            let profile = profiles.get_profile(
                pkg.package_id(),
                ws.is_member(pkg),
                is_local,
                unit_for,
                target_mode,
            );

            // No need to worry about build-dependencies, roots are never build dependencies.
            let features_for = FeaturesFor::from_for_host(target.proc_macro());
            let features = resolved_features.activated_features(pkg.package_id(), features_for);

            for kind in requested_kinds {
                let unit = interner.intern(
                    pkg,
                    target,
                    profile,
                    kind.for_target(target),
                    target_mode,
                    features.clone(),
                    /*is_std*/ false,
                );
                units.insert(unit);
            }
        };

    // Create a list of proposed targets.
    let mut proposals: Vec<Proposal<'_>> = Vec::new();

    match *filter {
        CompileFilter::Default {
            required_features_filterable,
        } => {
            for pkg in packages {
                let default = filter_default_targets(pkg.targets(), mode);
                proposals.extend(default.into_iter().map(|target| Proposal {
                    pkg,
                    target,
                    requires_features: !required_features_filterable,
                    mode,
                }));
                if mode == CompileMode::Test {
                    if let Some(t) = pkg
                        .targets()
                        .iter()
                        .find(|t| t.is_lib() && t.doctested() && t.doctestable())
                    {
                        proposals.push(Proposal {
                            pkg,
                            target: t,
                            requires_features: false,
                            mode: CompileMode::Doctest,
                        });
                    }
                }
            }
        }
        CompileFilter::Only {
            all_targets,
            ref lib,
            ref bins,
            ref examples,
            ref tests,
            ref benches,
        } => {
            if *lib != LibRule::False {
                let mut libs = Vec::new();
                for proposal in filter_targets(packages, Target::is_lib, false, mode) {
                    let Proposal { target, pkg, .. } = proposal;
                    if mode.is_doc_test() && !target.doctestable() {
                        let types = target.rustc_crate_types();
                        let types_str: Vec<&str> = types.iter().map(|t| t.as_str()).collect();
                        ws.config().shell().warn(format!(
                            "doc tests are not supported for crate type(s) `{}` in package `{}`",
                            types_str.join(", "),
                            pkg.name()
                        ))?;
                    } else {
                        libs.push(proposal)
                    }
                }
                if !all_targets && libs.is_empty() && *lib == LibRule::True {
                    let names = packages.iter().map(|pkg| pkg.name()).collect::<Vec<_>>();
                    if names.len() == 1 {
                        anyhow::bail!("no library targets found in package `{}`", names[0]);
                    } else {
                        anyhow::bail!("no library targets found in packages: {}", names.join(", "));
                    }
                }
                proposals.extend(libs);
            }

            // If `--tests` was specified, add all targets that would be
            // generated by `cargo test`.
            let test_filter = match tests {
                FilterRule::All => Target::tested,
                FilterRule::Just(_) => Target::is_test,
            };
            let test_mode = match mode {
                CompileMode::Build => CompileMode::Test,
                CompileMode::Check { .. } => CompileMode::Check { test: true },
                _ => mode,
            };
            // If `--benches` was specified, add all targets that would be
            // generated by `cargo bench`.
            let bench_filter = match benches {
                FilterRule::All => Target::benched,
                FilterRule::Just(_) => Target::is_bench,
            };
            let bench_mode = match mode {
                CompileMode::Build => CompileMode::Bench,
                CompileMode::Check { .. } => CompileMode::Check { test: true },
                _ => mode,
            };

            proposals.extend(list_rule_targets(
                packages,
                bins,
                "bin",
                Target::is_bin,
                mode,
            )?);
            proposals.extend(list_rule_targets(
                packages,
                examples,
                "example",
                Target::is_example,
                mode,
            )?);
            proposals.extend(list_rule_targets(
                packages,
                tests,
                "test",
                test_filter,
                test_mode,
            )?);
            proposals.extend(list_rule_targets(
                packages,
                benches,
                "bench",
                bench_filter,
                bench_mode,
            )?);
        }
    }

    // Only include targets that are libraries or have all required
    // features available.
    //
    // `features_map` is a map of &Package -> enabled_features
    // It is computed by the set of enabled features for the package plus
    // every enabled feature of every enabled dependency.
    let mut features_map = HashMap::new();
    let mut units = HashSet::new();
    for Proposal {
        pkg,
        target,
        requires_features,
        mode,
    } in proposals
    {
        let unavailable_features = match target.required_features() {
            Some(rf) => {
                warn_on_missing_features(
                    workspace_resolve,
                    rf,
                    pkg.summary(),
                    &mut config.shell(),
                )?;

                let features = features_map.entry(pkg).or_insert_with(|| {
                    resolve_all_features(resolve, resolved_features, package_set, pkg.package_id())
                });
                rf.iter().filter(|f| !features.contains(*f)).collect()
            }
            None => Vec::new(),
        };
        if target.is_lib() || unavailable_features.is_empty() {
            new_unit(&mut units, pkg, target, mode);
        } else if requires_features {
            let required_features = target.required_features().unwrap();
            let quoted_required_features: Vec<String> = required_features
                .iter()
                .map(|s| format!("`{}`", s))
                .collect();
            anyhow::bail!(
                "target `{}` in package `{}` requires the features: {}\n\
                 Consider enabling them by passing, e.g., `--features=\"{}\"`",
                target.name(),
                pkg.name(),
                quoted_required_features.join(", "),
                required_features.join(" ")
            );
        }
        // else, silently skip target.
    }
    Ok(units.into_iter().collect())
}

fn warn_on_missing_features(
    resolve: &Option<Resolve>,
    required_features: &[String],
    summary: &Summary,
    shell: &mut Shell,
) -> CargoResult<()> {
    let resolve = match resolve {
        None => return Ok(()),
        Some(resolve) => resolve,
    };

    for feature in required_features {
        match FeatureValue::new(feature.into(), summary) {
            // No need to do anything here, since the feature must exist to be parsed as such
            FeatureValue::Feature(_) => {}
            // Possibly mislabeled feature that was not found
            FeatureValue::Crate(krate) => {
                if !summary
                    .dependencies()
                    .iter()
                    .any(|dep| dep.name_in_toml() == krate && dep.is_optional())
                {
                    shell.warn(format!(
                        "feature `{}` is not present in [features] section.",
                        krate
                    ))?;
                }
            }
            // Handling of dependent_crate/dependent_crate_feature syntax
            FeatureValue::CrateFeature(krate, feature) => {
                match resolve
                    .deps(summary.package_id())
                    .find(|(_dep_id, deps)| deps.iter().any(|dep| dep.name_in_toml() == krate))
                {
                    Some((dep_id, _deps)) => {
                        let dep_summary = resolve.summary(dep_id);
                        if !dep_summary.features().contains_key(&feature)
                            && !dep_summary
                                .dependencies()
                                .iter()
                                .any(|dep| dep.name_in_toml() == feature && dep.is_optional())
                        {
                            shell.warn(format!(
                                "feature `{}` does not exist in package `{}`.",
                                feature, dep_id
                            ))?;
                        }
                    }
                    None => {
                        shell.warn(format!(
                            "dependency `{}` specified in required-features as `{}/{}` \
                             does not exist.",
                            krate, krate, feature
                        ))?;
                    }
                }
            }
        }
    }
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

/// Given a list of all targets for a package, filters out only the targets
/// that are automatically included when the user doesn't specify any targets.
fn filter_default_targets(targets: &[Target], mode: CompileMode) -> Vec<&Target> {
    match mode {
        CompileMode::Bench => targets.iter().filter(|t| t.benched()).collect(),
        CompileMode::Test => targets
            .iter()
            .filter(|t| t.tested() || t.is_example())
            .collect(),
        CompileMode::Build | CompileMode::Check { .. } => targets
            .iter()
            .filter(|t| t.is_bin() || t.is_lib())
            .collect(),
        CompileMode::Doc { .. } => {
            // `doc` does lib and bins (bin with same name as lib is skipped).
            targets
                .iter()
                .filter(|t| {
                    t.documented()
                        && (!t.is_bin()
                            || !targets.iter().any(|l| l.is_lib() && l.name() == t.name()))
                })
                .collect()
        }
        CompileMode::Doctest | CompileMode::RunCustomBuild => panic!("Invalid mode {:?}", mode),
    }
}

/// Returns a list of proposed targets based on command-line target selection flags.
fn list_rule_targets<'a>(
    packages: &[&'a Package],
    rule: &FilterRule,
    target_desc: &'static str,
    is_expected_kind: fn(&Target) -> bool,
    mode: CompileMode,
) -> CargoResult<Vec<Proposal<'a>>> {
    let mut proposals = Vec::new();
    match rule {
        FilterRule::All => {
            proposals.extend(filter_targets(packages, is_expected_kind, false, mode))
        }
        FilterRule::Just(names) => {
            for name in names {
                proposals.extend(find_named_targets(
                    packages,
                    name,
                    target_desc,
                    is_expected_kind,
                    mode,
                )?);
            }
        }
    }
    Ok(proposals)
}

/// Finds the targets for a specifically named target.
fn find_named_targets<'a>(
    packages: &[&'a Package],
    target_name: &str,
    target_desc: &'static str,
    is_expected_kind: fn(&Target) -> bool,
    mode: CompileMode,
) -> CargoResult<Vec<Proposal<'a>>> {
    let filter = |t: &Target| t.name() == target_name && is_expected_kind(t);
    let proposals = filter_targets(packages, filter, true, mode);
    if proposals.is_empty() {
        let targets = packages.iter().flat_map(|pkg| {
            pkg.targets()
                .iter()
                .filter(|target| is_expected_kind(target))
        });
        let suggestion = closest_msg(target_name, targets, |t| t.name());
        anyhow::bail!(
            "no {} target named `{}`{}",
            target_desc,
            target_name,
            suggestion
        );
    }
    Ok(proposals)
}

fn filter_targets<'a>(
    packages: &[&'a Package],
    predicate: impl Fn(&Target) -> bool,
    requires_features: bool,
    mode: CompileMode,
) -> Vec<Proposal<'a>> {
    let mut proposals = Vec::new();
    for pkg in packages {
        for target in pkg.targets().iter().filter(|t| predicate(t)) {
            proposals.push(Proposal {
                pkg,
                target,
                requires_features,
                mode,
            });
        }
    }
    proposals
}
