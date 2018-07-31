//!
//! Cargo compile currently does the following steps:
//!
//! All configurations are already injected as environment variables via the
//! main cargo command
//!
//! 1. Read the manifest
//! 2. Shell out to `cargo-resolve` with a list of dependencies and sources as
//!    stdin
//!
//!    a. Shell out to `--do update` and `--do list` for each source
//!    b. Resolve dependencies and return a list of name/version/source
//!
//! 3. Shell out to `--do download` for each source
//! 4. Shell out to `--do get` for each source, and build up the list of paths
//!    to pass to rustc -L
//! 5. Call `cargo-rustc` with the results of the resolver zipped together with
//!    the results of the `get`
//!
//!    a. Topologically sort the dependencies
//!    b. Compile each dependency in order, passing in the -L's pointing at each
//!       previously compiled dependency
//!

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use core::compiler::{BuildConfig, BuildContext, Compilation, Context, DefaultExecutor, Executor};
use core::compiler::{CompileMode, Kind, Unit};
use core::profiles::{ProfileFor, Profiles};
use core::resolver::{Method, Resolve};
use core::{Package, Source, Target};
use core::{PackageId, PackageIdSpec, TargetKind, Workspace};
use ops;
use util::config::Config;
use util::{lev_distance, profile, CargoResult};

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
    /// Extra arguments to be passed to rustdoc (for main crate and dependencies)
    pub target_rustdoc_args: Option<Vec<String>>,
    /// The specified target will be compiled with all the available arguments,
    /// note that this only accounts for the *final* invocation of rustc
    pub target_rustc_args: Option<Vec<String>>,
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
            export_dir: None,
        })
    }

    // Returns the unique specified package, or None
    pub fn get_package<'b>(&self, ws: &'b Workspace) -> CargoResult<Option<&'b Package>> {
        Ok(match self.spec {
            Packages::All | Packages::Default | Packages::OptOut(_) => {
                None
            }
            Packages::Packages(ref xs) => match xs.len() {
                0 => Some(ws.current()?),
                1 => Some(ws.members()
                    .find(|pkg| *pkg.name() == xs[0])
                    .ok_or_else(|| {
                        format_err!("package `{}` is not a member of the workspace", xs[0])
                    })?),
                _ => None,
            },
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
            (false, _, _) => bail!("--exclude can only be used together with --all"),
            (true, 0, _) => Packages::All,
            (true, _, _) => Packages::OptOut(exclude),
        })
    }

    pub fn to_package_id_specs(&self, ws: &Workspace) -> CargoResult<Vec<PackageIdSpec>> {
        let specs = match *self {
            Packages::All => ws.members()
                .map(Package::package_id)
                .map(PackageIdSpec::from_package_id)
                .collect(),
            Packages::OptOut(ref opt_out) => ws.members()
                .map(Package::package_id)
                .map(PackageIdSpec::from_package_id)
                .filter(|p| opt_out.iter().position(|x| *x == p.name()).is_none())
                .collect(),
            Packages::Packages(ref packages) if packages.is_empty() => {
                vec![PackageIdSpec::from_package_id(ws.current()?.package_id())]
            }
            Packages::Packages(ref packages) => packages
                .iter()
                .map(|p| PackageIdSpec::parse(p))
                .collect::<CargoResult<Vec<_>>>()?,
            Packages::Default => ws.default_members()
                .map(Package::package_id)
                .map(PackageIdSpec::from_package_id)
                .collect(),
        };
        if specs.is_empty() {
            if ws.is_virtual() {
                bail!(
                    "manifest path `{}` contains no package: The manifest is virtual, \
                     and the workspace has no members.",
                    ws.root().display()
                )
            }
            bail!("no packages to compile")
        }
        Ok(specs)
    }
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
        lib: bool,
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
    let exec: Arc<Executor> = Arc::new(DefaultExecutor);
    compile_with_exec(ws, options, &exec)
}

/// Like `compile` but allows specifying a custom `Executor` that will be able to intercept build
/// calls and add custom logic. `compile` uses `DefaultExecutor` which just passes calls through.
pub fn compile_with_exec<'a>(
    ws: &Workspace<'a>,
    options: &CompileOptions<'a>,
    exec: &Arc<Executor>,
) -> CargoResult<Compilation<'a>> {
    ws.emit_warnings()?;
    compile_ws(ws, None, options, exec)
}

pub fn compile_ws<'a>(
    ws: &Workspace<'a>,
    source: Option<Box<Source + 'a>>,
    options: &CompileOptions<'a>,
    exec: &Arc<Executor>,
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
        ref export_dir,
    } = *options;

    let default_arch_kind = if build_config.requested_target.is_some() {
        Kind::Target
    } else {
        Kind::Host
    };

    let specs = spec.to_package_id_specs(ws)?;
    let features = Method::split_features(features);
    let method = Method::Required {
        dev_deps: ws.require_optional_deps() || filter.need_dev_deps(build_config.mode),
        features: &features,
        all_features,
        uses_default_features: !no_default_features,
    };
    let resolve = ops::resolve_ws_with_method(ws, source, method, &specs)?;
    let (packages, resolve_with_overrides) = resolve;

    let to_builds = specs
        .iter()
        .map(|p| {
            let pkgid = p.query(resolve_with_overrides.iter())?;
            let p = packages.get(pkgid)?;
            p.manifest().print_teapot(ws.config());
            Ok(p)
        })
        .collect::<CargoResult<Vec<_>>>()?;

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

    let profiles = ws.profiles();
    profiles.validate_packages(&mut config.shell(), &packages)?;

    let mut extra_compiler_args = None;

    let units = generate_targets(
        ws,
        profiles,
        &to_builds,
        filter,
        default_arch_kind,
        &resolve_with_overrides,
        build_config,
    )?;

    if let Some(args) = extra_args {
        if units.len() != 1 {
            bail!(
                "extra arguments to `{}` can only be passed to one \
                 target, consider filtering\nthe package by passing \
                 e.g. `--lib` or `--bin NAME` to specify a single target",
                extra_args_name
            );
        }
        extra_compiler_args = Some((units[0], args));
    }

    let ret = {
        let _p = profile::start("compiling");
        let bcx = BuildContext::new(
            ws,
            &resolve_with_overrides,
            &packages,
            config,
            &build_config,
            profiles,
            extra_compiler_args,
        )?;
        let mut cx = Context::new(config, &bcx)?;
        cx.compile(&units, export_dir.clone(), &exec)?
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
    pub fn new(
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
        let rule_bins = FilterRule::new(bins, all_bins);
        let rule_tsts = FilterRule::new(tsts, all_tsts);
        let rule_exms = FilterRule::new(exms, all_exms);
        let rule_bens = FilterRule::new(bens, all_bens);

        if all_targets {
            CompileFilter::Only {
                all_targets: true,
                lib: true,
                bins: FilterRule::All,
                examples: FilterRule::All,
                benches: FilterRule::All,
                tests: FilterRule::All,
            }
        } else if lib_only || rule_bins.is_specific() || rule_tsts.is_specific()
            || rule_exms.is_specific() || rule_bens.is_specific()
        {
            CompileFilter::Only {
                all_targets: false,
                lib: lib_only,
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
    // other subcommands, see generate_targets and generate_default_targets
    pub fn target_run(&self, target: &Target) -> bool {
        match *self {
            CompileFilter::Default { .. } => true,
            CompileFilter::Only {
                lib,
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
                    TargetKind::Lib(..) => return lib,
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

/// Generates all the base targets for the packages the user has requested to
/// compile. Dependencies for these targets are computed later in
/// `unit_dependencies`.
fn generate_targets<'a>(
    ws: &Workspace,
    profiles: &Profiles,
    packages: &[&'a Package],
    filter: &CompileFilter,
    default_arch_kind: Kind,
    resolve: &Resolve,
    build_config: &BuildConfig,
) -> CargoResult<Vec<Unit<'a>>> {
    let mut units = Vec::new();

    // Helper for creating a Unit struct.
    let new_unit = |pkg: &'a Package, target: &'a Target, target_mode: CompileMode| {
        let profile_for = if build_config.mode.is_any_test() {
            // NOTE: The ProfileFor here is subtle.  If you have a profile
            // with `panic` set, the `panic` flag is cleared for
            // tests/benchmarks and their dependencies.  If we left this
            // as an "Any" profile, then the lib would get compiled three
            // times (once with panic, once without, and once with
            // --test).
            //
            // This would cause a problem for Doc tests, which would fail
            // because `rustdoc` would attempt to link with both libraries
            // at the same time. Also, it's probably not important (or
            // even desirable?) for rustdoc to link with a lib with
            // `panic` set.
            //
            // As a consequence, Examples and Binaries get compiled
            // without `panic` set.  This probably isn't a bad deal.
            //
            // Forcing the lib to be compiled three times during `cargo
            // test` is probably also not desirable.
            ProfileFor::TestDependency
        } else {
            ProfileFor::Any
        };
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
            _ => target_mode,
        };
        // Plugins or proc-macro should be built for the host.
        let kind = if target.for_host() {
            Kind::Host
        } else {
            default_arch_kind
        };
        let profile = profiles.get_profile(
            pkg.package_id(),
            ws.is_member(pkg),
            profile_for,
            target_mode,
            build_config.release,
        );
        // Once the profile has been selected for benchmarks, we don't need to
        // distinguish between benches and tests. Switching the mode allows
        // de-duplication of units that are essentially identical.  For
        // example, `cargo build --all-targets --release` creates the units
        // (lib profile:bench, mode:test) and (lib profile:bench, mode:bench)
        // and since these are the same, we want them to be de-duped in
        // `unit_dependencies`.
        let target_mode = match target_mode {
            CompileMode::Bench => CompileMode::Test,
            _ => target_mode,
        };
        Unit {
            pkg,
            target,
            profile,
            kind,
            mode: target_mode,
        }
    };

    for pkg in packages {
        let features = resolve_all_features(resolve, pkg.package_id());
        // Create a list of proposed targets.  The `bool` value indicates
        // whether or not all required features *must* be present. If false,
        // and the features are not available, then it will be silently
        // skipped.  Generally, targets specified by name (`--bin foo`) are
        // required, all others can be silently skipped if features are
        // missing.
        let mut proposals: Vec<(Unit<'a>, bool)> = Vec::new();

        match *filter {
            CompileFilter::Default {
                required_features_filterable,
            } => {
                let default_units = generate_default_targets(pkg.targets(), build_config.mode)
                    .iter()
                    .map(|t| {
                        (
                            new_unit(pkg, t, build_config.mode),
                            !required_features_filterable,
                        )
                    })
                    .collect::<Vec<_>>();
                proposals.extend(default_units);
                if build_config.mode == CompileMode::Test {
                    // Include doctest for lib.
                    if let Some(t) = pkg
                        .targets()
                        .iter()
                        .find(|t| t.is_lib() && t.doctested() && t.doctestable())
                    {
                        proposals.push((new_unit(pkg, t, CompileMode::Doctest), false));
                    }
                }
            }
            CompileFilter::Only {
                all_targets,
                lib,
                ref bins,
                ref examples,
                ref tests,
                ref benches,
            } => {
                if lib {
                    if let Some(target) = pkg.targets().iter().find(|t| t.is_lib()) {
                        if build_config.mode == CompileMode::Doctest && !target.doctestable() {
                            bail!(
                                "doc tests are not supported for crate type(s) `{}` in package `{}`",
                                target.rustc_crate_types().join(", "),
                                pkg.name()
                            );
                        }
                        proposals.push((new_unit(pkg, target, build_config.mode), false));
                    } else if !all_targets {
                        bail!("no library targets found in package `{}`", pkg.name())
                    }
                }
                // If --tests was specified, add all targets that would be
                // generated by `cargo test`.
                let test_filter = match *tests {
                    FilterRule::All => Target::tested,
                    FilterRule::Just(_) => Target::is_test,
                };
                let test_mode = match build_config.mode {
                    CompileMode::Build => CompileMode::Test,
                    CompileMode::Check { .. } => CompileMode::Check { test: true },
                    _ => build_config.mode,
                };
                // If --benches was specified, add all targets that would be
                // generated by `cargo bench`.
                let bench_filter = match *benches {
                    FilterRule::All => Target::benched,
                    FilterRule::Just(_) => Target::is_bench,
                };
                let bench_mode = match build_config.mode {
                    CompileMode::Build => CompileMode::Bench,
                    CompileMode::Check { .. } => CompileMode::Check { test: true },
                    _ => build_config.mode,
                };

                proposals.extend(
                    list_rule_targets(pkg, bins, "bin", Target::is_bin)?
                        .into_iter()
                        .map(|(t, required)| (new_unit(pkg, t, build_config.mode), required))
                        .chain(
                            list_rule_targets(pkg, examples, "example", Target::is_example)?
                                .into_iter()
                                .map(|(t, required)| {
                                    (new_unit(pkg, t, build_config.mode), required)
                                }),
                        )
                        .chain(
                            list_rule_targets(pkg, tests, "test", test_filter)?
                                .into_iter()
                                .map(|(t, required)| (new_unit(pkg, t, test_mode), required)),
                        )
                        .chain(
                            list_rule_targets(pkg, benches, "bench", bench_filter)?
                                .into_iter()
                                .map(|(t, required)| (new_unit(pkg, t, bench_mode), required)),
                        )
                        .collect::<Vec<_>>(),
                );
            }
        }

        // Only include targets that are libraries or have all required
        // features available.
        for (unit, required) in proposals {
            let unavailable_features = match unit.target.required_features() {
                Some(rf) => rf.iter().filter(|f| !features.contains(*f)).collect(),
                None => Vec::new(),
            };
            if unit.target.is_lib() || unavailable_features.is_empty() {
                units.push(unit);
            } else if required {
                let required_features = unit.target.required_features().unwrap();
                let quoted_required_features: Vec<String> = required_features
                    .iter()
                    .map(|s| format!("`{}`", s))
                    .collect();
                bail!(
                    "target `{}` requires the features: {}\n\
                     Consider enabling them by passing e.g. `--features=\"{}\"`",
                    unit.target.name(),
                    quoted_required_features.join(", "),
                    required_features.join(" ")
                );
            }
            // else, silently skip target.
        }
    }
    Ok(units)
}

fn resolve_all_features(
    resolve_with_overrides: &Resolve,
    package_id: &PackageId,
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
fn generate_default_targets(targets: &[Target], mode: CompileMode) -> Vec<&Target> {
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

/// Returns a list of targets based on command-line target selection flags.
/// The return value is a list of `(Target, bool)` pairs.  The `bool` value
/// indicates whether or not all required features *must* be present.
fn list_rule_targets<'a>(
    pkg: &'a Package,
    rule: &FilterRule,
    target_desc: &'static str,
    is_expected_kind: fn(&Target) -> bool,
) -> CargoResult<Vec<(&'a Target, bool)>> {
    match *rule {
        FilterRule::All => Ok(pkg.targets()
            .iter()
            .filter(|t| is_expected_kind(t))
            .map(|t| (t, false))
            .collect()),
        FilterRule::Just(ref names) => names
            .iter()
            .map(|name| find_target(pkg, name, target_desc, is_expected_kind))
            .collect(),
    }
}

/// Find the target for a specifically named target.
fn find_target<'a>(
    pkg: &'a Package,
    target_name: &str,
    target_desc: &'static str,
    is_expected_kind: fn(&Target) -> bool,
) -> CargoResult<(&'a Target, bool)> {
    match pkg.targets()
        .iter()
        .find(|t| t.name() == target_name && is_expected_kind(t))
    {
        // When a target is specified by name, required features *must* be
        // available.
        Some(t) => Ok((t, true)),
        None => {
            let suggestion = pkg.targets()
                .iter()
                .filter(|t| is_expected_kind(t))
                .map(|t| (lev_distance(target_name, t.name()), t))
                .filter(|&(d, _)| d < 4)
                .min_by_key(|t| t.0)
                .map(|t| t.1);
            match suggestion {
                Some(s) => bail!(
                    "no {} target named `{}`\n\nDid you mean `{}`?",
                    target_desc,
                    target_name,
                    s.name()
                ),
                None => bail!("no {} target named `{}`", target_desc, target_name),
            }
        }
    }
}
