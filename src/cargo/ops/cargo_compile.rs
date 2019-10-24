//! The Cargo "compile" operation.
//!
//! This module contains the entry point for starting the compilation process
//! for commands like `build`, `test`, `doc`, `rustc`, etc.
//!
//! The `compile` function will do all the work to compile a workspace. A
//! rough outline is:
//!
//! - Resolve the dependency graph (see `ops::resolve`).
//! - Download any packages needed (see `PackageSet`). Note that dependency
//!   downloads are deferred until `build_unit_dependencies`.
//! - Generate a list of top-level "units" of work for the targets the user
//!   requested on the command-line. Each `Unit` corresponds to a compiler
//!   invocation. This is done in this module (`generate_targets`).
//! - Create a `Context` which will perform the following steps:
//!     - Build the graph of `Unit` dependencies (see
//!       `core::compiler::context::unit_dependencies`).
//!     - Prepare the `target` directory (see `Layout`).
//!     - Create a job queue (see `JobQueue`). The queue checks the
//!       fingerprint of each `Unit` to determine if it should run or be
//!       skipped.
//!     - Execute the queue. Each leaf in the queue's dependency graph is
//!       executed, and then removed from the graph when finished. This
//!       repeats until the queue is empty.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::iter::FromIterator;
use std::path::PathBuf;
use std::sync::Arc;

use crate::core::compiler::standard_lib;
use crate::core::compiler::unit_dependencies::build_unit_dependencies;
use crate::core::compiler::{BuildConfig, BuildContext, Compilation, Context};
use crate::core::compiler::{CompileKind, CompileMode, Unit};
use crate::core::compiler::{DefaultExecutor, Executor, UnitInterner};
use crate::core::profiles::{Profiles, UnitFor};
use crate::core::resolver::{Resolve, ResolveOpts};
use crate::core::{LibKind, Package, PackageSet, Target};
use crate::core::{PackageId, PackageIdSpec, TargetKind, Workspace};
use crate::ops;
use crate::ops::resolve::WorkspaceResolve;
use crate::util::config::Config;
use crate::util::{closest_msg, profile, CargoResult};

/// Contains information about how a package should be compiled.
#[derive(Debug)]
pub struct CompileOptions<'a> {
    pub config: &'a Config,
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
    /// The directory to copy final artifacts to. Note that even if `out_dir` is
    /// set, a copy of artifacts still could be found a `target/(debug\release)`
    /// as usual.
    // Note that, although the cmd-line flag name is `out-dir`, in code we use
    // `export_dir`, to avoid confusion with out dir at `target/debug/deps`.
    pub export_dir: Option<PathBuf>,
}

impl<'a> CompileOptions<'a> {
    pub fn new(config: &'a Config, mode: CompileMode) -> CargoResult<CompileOptions<'a>> {
        Ok(CompileOptions {
            config,
            build_config: BuildConfig::new(config, None, &None, mode)?,
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
            export_dir: None,
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
            (false, _, _) => failure::bail!("--exclude can only be used together with --workspace"),
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
                failure::bail!(
                    "manifest path `{}` contains no package: The manifest is virtual, \
                     and the workspace has no members.",
                    ws.root().display()
                )
            }
            failure::bail!("no packages to compile")
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
                            failure::format_err!(
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

pub fn compile<'a>(
    ws: &Workspace<'a>,
    options: &CompileOptions<'a>,
) -> CargoResult<Compilation<'a>> {
    let exec: Arc<dyn Executor> = Arc::new(DefaultExecutor);
    compile_with_exec(ws, options, &exec)
}

/// Like `compile` but allows specifying a custom `Executor` that will be able to intercept build
/// calls and add custom logic. `compile` uses `DefaultExecutor` which just passes calls through.
pub fn compile_with_exec<'a>(
    ws: &Workspace<'a>,
    options: &CompileOptions<'a>,
    exec: &Arc<dyn Executor>,
) -> CargoResult<Compilation<'a>> {
    ws.emit_warnings()?;
    compile_ws(ws, options, exec)
}

pub fn compile_ws<'a>(
    ws: &Workspace<'a>,
    options: &CompileOptions<'a>,
    exec: &Arc<dyn Executor>,
) -> CargoResult<Compilation<'a>> {
    let CompileOptions {
        config,
        ref build_config,
        ref spec,
        ref features,
        all_features,
        no_default_features,
        ref filter,
        ref target_rustdoc_args,
        ref target_rustc_args,
        ref local_rustdoc_args,
        ref export_dir,
    } = *options;

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

    let profiles = ws.profiles();

    // Early check for whether the profile is defined.
    let _ = profiles.base_profile(&build_config.profile_kind)?;

    let specs = spec.to_package_id_specs(ws)?;
    let dev_deps = ws.require_optional_deps() || filter.need_dev_deps(build_config.mode);
    let opts = ResolveOpts::new(dev_deps, features, all_features, !no_default_features);
    let resolve = ops::resolve_ws_with_opts(ws, opts, &specs)?;
    let WorkspaceResolve {
        mut pkg_set,
        workspace_resolve,
        targeted_resolve: resolve,
    } = resolve;

    let std_resolve = if let Some(crates) = &config.cli_unstable().build_std {
        if build_config.build_plan {
            config
                .shell()
                .warn("-Zbuild-std does not currently fully support --build-plan")?;
        }
        if build_config.requested_kind.is_host() {
            // TODO: This should eventually be fixed. Unfortunately it is not
            // easy to get the host triple in BuildConfig. Consider changing
            // requested_target to an enum, or some other approach.
            failure::bail!("-Zbuild-std requires --target");
        }
        let (mut std_package_set, std_resolve) = standard_lib::resolve_std(ws, crates)?;
        remove_dylib_crate_type(&mut std_package_set)?;
        pkg_set.add_set(std_package_set);
        Some(std_resolve)
    } else {
        None
    };

    // Find the packages in the resolver that the user wants to build (those
    // passed in with `-p` or the defaults from the workspace), and convert
    // Vec<PackageIdSpec> to a Vec<&PackageId>.
    let to_build_ids = specs
        .iter()
        .map(|s| s.query(resolve.iter()))
        .collect::<CargoResult<Vec<_>>>()?;
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
            failure::bail!(
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

    profiles.validate_packages(
        &mut config.shell(),
        workspace_resolve.as_ref().unwrap_or(&resolve),
    )?;

    let interner = UnitInterner::new();
    let mut bcx = BuildContext::new(
        ws,
        &pkg_set,
        config,
        build_config,
        profiles,
        &interner,
        HashMap::new(),
    )?;
    let units = generate_targets(
        ws,
        profiles,
        &to_builds,
        filter,
        build_config.requested_kind,
        &resolve,
        &bcx,
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
        standard_lib::generate_std_roots(
            &bcx,
            &crates,
            std_resolve.as_ref().unwrap(),
            build_config.requested_kind,
        )?
    } else {
        Vec::new()
    };

    if let Some(args) = extra_args {
        if units.len() != 1 {
            failure::bail!(
                "extra arguments to `{}` can only be passed to one \
                 target, consider filtering\nthe package by passing, \
                 e.g., `--lib` or `--bin NAME` to specify a single target",
                extra_args_name
            );
        }
        bcx.extra_compiler_args.insert(units[0], args);
    }
    if let Some(args) = local_rustdoc_args {
        for unit in &units {
            if unit.mode.is_doc() || unit.mode.is_doc_test() {
                bcx.extra_compiler_args.insert(*unit, args.clone());
            }
        }
    }

    let unit_dependencies =
        build_unit_dependencies(&bcx, &resolve, std_resolve.as_ref(), &units, &std_roots)?;

    let ret = {
        let _p = profile::start("compiling");
        let cx = Context::new(config, &bcx, unit_dependencies, build_config.requested_kind)?;
        cx.compile(&units, export_dir.clone(), exec)?
    };

    Ok(ret)
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
            CompileMode::Build | CompileMode::Doc { .. } | CompileMode::Check { .. } => match *self
            {
                CompileFilter::Default { .. } => false,
                CompileFilter::Only {
                    ref examples,
                    ref tests,
                    ref benches,
                    ..
                } => examples.is_specific() || tests.is_specific() || benches.is_specific(),
            },
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
fn generate_targets<'a>(
    ws: &Workspace<'_>,
    profiles: &Profiles,
    packages: &[&'a Package],
    filter: &CompileFilter,
    default_arch_kind: CompileKind,
    resolve: &'a Resolve,
    bcx: &BuildContext<'a, '_>,
) -> CargoResult<Vec<Unit<'a>>> {
    // Helper for creating a `Unit` struct.
    let new_unit = |pkg: &'a Package, target: &'a Target, target_mode: CompileMode| {
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
            UnitFor::new_test(bcx.config)
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
        let kind = default_arch_kind.for_target(target);
        let profile = profiles.get_profile(
            pkg.package_id(),
            ws.is_member(pkg),
            unit_for,
            target_mode,
            bcx.build_config.profile_kind.clone(),
        );
        let features = resolve.features_sorted(pkg.package_id());
        bcx.units.intern(
            pkg,
            target,
            profile,
            kind,
            target_mode,
            features,
            /*is_std*/ false,
        )
    };

    // Create a list of proposed targets.
    let mut proposals: Vec<Proposal<'_>> = Vec::new();

    match *filter {
        CompileFilter::Default {
            required_features_filterable,
        } => {
            for pkg in packages {
                let default = filter_default_targets(pkg.targets(), bcx.build_config.mode);
                proposals.extend(default.into_iter().map(|target| Proposal {
                    pkg,
                    target,
                    requires_features: !required_features_filterable,
                    mode: bcx.build_config.mode,
                }));
                if bcx.build_config.mode == CompileMode::Test {
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
                for proposal in
                    filter_targets(packages, Target::is_lib, false, bcx.build_config.mode)
                {
                    let Proposal { target, pkg, .. } = proposal;
                    if bcx.build_config.mode.is_doc_test() && !target.doctestable() {
                        ws.config().shell().warn(format!(
                            "doc tests are not supported for crate type(s) `{}` in package `{}`",
                            target.rustc_crate_types().join(", "),
                            pkg.name()
                        ))?;
                    } else {
                        libs.push(proposal)
                    }
                }
                if !all_targets && libs.is_empty() && *lib == LibRule::True {
                    let names = packages.iter().map(|pkg| pkg.name()).collect::<Vec<_>>();
                    if names.len() == 1 {
                        failure::bail!("no library targets found in package `{}`", names[0]);
                    } else {
                        failure::bail!(
                            "no library targets found in packages: {}",
                            names.join(", ")
                        );
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
            let test_mode = match bcx.build_config.mode {
                CompileMode::Build => CompileMode::Test,
                CompileMode::Check { .. } => CompileMode::Check { test: true },
                _ => bcx.build_config.mode,
            };
            // If `--benches` was specified, add all targets that would be
            // generated by `cargo bench`.
            let bench_filter = match benches {
                FilterRule::All => Target::benched,
                FilterRule::Just(_) => Target::is_bench,
            };
            let bench_mode = match bcx.build_config.mode {
                CompileMode::Build => CompileMode::Bench,
                CompileMode::Check { .. } => CompileMode::Check { test: true },
                _ => bcx.build_config.mode,
            };

            proposals.extend(list_rule_targets(
                packages,
                bins,
                "bin",
                Target::is_bin,
                bcx.build_config.mode,
            )?);
            proposals.extend(list_rule_targets(
                packages,
                examples,
                "example",
                Target::is_example,
                bcx.build_config.mode,
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
                let features = features_map
                    .entry(pkg)
                    .or_insert_with(|| resolve_all_features(resolve, pkg.package_id()));
                rf.iter().filter(|f| !features.contains(*f)).collect()
            }
            None => Vec::new(),
        };
        if target.is_lib() || unavailable_features.is_empty() {
            let unit = new_unit(pkg, target, mode);
            units.insert(unit);
        } else if requires_features {
            let required_features = target.required_features().unwrap();
            let quoted_required_features: Vec<String> = required_features
                .iter()
                .map(|s| format!("`{}`", s))
                .collect();
            failure::bail!(
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

fn resolve_all_features(
    resolve_with_overrides: &Resolve,
    package_id: PackageId,
) -> HashSet<String> {
    let mut features = resolve_with_overrides.features(package_id).clone();

    // Include features enabled for use by dependencies so targets can also use them with the
    // required-features field when deciding whether to be built or skipped.
    for (dep, _) in resolve_with_overrides.deps(package_id) {
        for feature in resolve_with_overrides.features(dep) {
            features.insert(dep.name().to_string() + "/" + feature);
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
        failure::bail!(
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

/// When using `-Zbuild-std` we're building the standard library, but a
/// technical detail of the standard library right now is that it builds itself
/// as both an `rlib` and a `dylib`. We don't actually want to really publicize
/// the `dylib` and in general it's a pain to work with, so when building libstd
/// we want to remove the `dylib` crate type.
///
/// Cargo doesn't have a fantastic way of doing that right now, so let's hack
/// around it a bit and (ab)use the fact that we have mutable access to
/// `PackageSet` here to rewrite downloaded packages. We iterate over all `path`
/// packages (which should download immediately and not actually cause blocking
/// here) and edit their manifests to only list one `LibKind` for an `Rlib`.
fn remove_dylib_crate_type(set: &mut PackageSet<'_>) -> CargoResult<()> {
    let ids = set
        .package_ids()
        .filter(|p| p.source_id().is_path())
        .collect::<Vec<_>>();
    set.get_many(ids.iter().cloned())?;

    for id in ids {
        let pkg = set.lookup_mut(id).expect("should be downloaded now");

        for target in pkg.manifest_mut().targets_mut() {
            if let TargetKind::Lib(crate_types) = target.kind_mut() {
                crate_types.truncate(0);
                crate_types.push(LibKind::Rlib);
            }
        }
    }

    Ok(())
}
