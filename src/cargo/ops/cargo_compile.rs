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

use std::collections::{HashMap, HashSet};
use std::default::Default;
use std::path::PathBuf;
use std::sync::Arc;

use core::{Source, Package, Target};
use core::{Profile, TargetKind, Profiles, Workspace, PackageIdSpec};
use ops::{self, BuildOutput, Executor, DefaultExecutor};
use util::config::Config;
use util::{CargoResult, profile};

/// Contains information about how a package should be compiled.
pub struct CompileOptions<'a> {
    pub config: &'a Config,
    /// Number of concurrent jobs to use.
    pub jobs: Option<u32>,
    /// The target platform to compile for (example: `i686-unknown-linux-gnu`).
    pub target: Option<&'a str>,
    /// Extra features to build for the root package
    pub features: &'a [String],
    /// Flag whether all available features should be built for the root package
    pub all_features: bool,
    /// Flag if the default feature should be built for the root package
    pub no_default_features: bool,
    /// A set of packages to build.
    pub spec: Packages<'a>,
    /// Filter to apply to the root package to select which targets will be
    /// built.
    pub filter: CompileFilter<'a>,
    /// Whether this is a release build or not
    pub release: bool,
    /// Mode for this compile.
    pub mode: CompileMode,
    /// `--error_format` flag for the compiler.
    pub message_format: MessageFormat,
    /// Extra arguments to be passed to rustdoc (for main crate and dependencies)
    pub target_rustdoc_args: Option<&'a [String]>,
    /// The specified target will be compiled with all the available arguments,
    /// note that this only accounts for the *final* invocation of rustc
    pub target_rustc_args: Option<&'a [String]>,
}

impl<'a> CompileOptions<'a> {
    pub fn default(config: &'a Config, mode: CompileMode) -> CompileOptions<'a>
    {
        CompileOptions {
            config: config,
            jobs: None,
            target: None,
            features: &[],
            all_features: false,
            no_default_features: false,
            spec: ops::Packages::Packages(&[]),
            mode: mode,
            release: false,
            filter: ops::CompileFilter::new(false, &[], &[], &[], &[]),
            message_format: MessageFormat::Human,
            target_rustdoc_args: None,
            target_rustc_args: None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CompileMode {
    Test,
    Build,
    Check,
    Bench,
    Doc { deps: bool },
    Doctest,
}

#[derive(Clone, Copy, PartialEq, Eq, RustcDecodable)]
pub enum MessageFormat {
    Human,
    Json
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Packages<'a> {
    All,
    Packages(&'a [String]),
}

impl<'a> Packages<'a> {
    pub fn into_package_id_specs(self, ws: &Workspace) -> CargoResult<Vec<PackageIdSpec>> {
        let specs = match self {
            Packages::All => {
                ws.members()
                    .map(Package::package_id)
                    .map(PackageIdSpec::from_package_id)
                    .collect()
            }
            Packages::Packages(packages) => {
                packages.iter().map(|p| PackageIdSpec::parse(&p)).collect::<CargoResult<Vec<_>>>()?
            }
        };
        Ok(specs)
    }
}

pub enum CompileFilter<'a> {
    Everything,
    Only {
        lib: bool,
        bins: &'a [String],
        examples: &'a [String],
        tests: &'a [String],
        benches: &'a [String],
    }
}

pub fn compile<'a>(ws: &Workspace<'a>, options: &CompileOptions<'a>)
                   -> CargoResult<ops::Compilation<'a>> {
    compile_with_exec(ws, options, Arc::new(DefaultExecutor))
}

pub fn compile_with_exec<'a>(ws: &Workspace<'a>,
                             options: &CompileOptions<'a>, 
                             exec: Arc<Executor>)
                             -> CargoResult<ops::Compilation<'a>> {
    for member in ws.members() {
        for key in member.manifest().warnings().iter() {
            options.config.shell().warn(key)?
        }
    }
    compile_ws(ws, None, options, exec)
}

pub fn compile_ws<'a>(ws: &Workspace<'a>,
                      source: Option<Box<Source + 'a>>,
                      options: &CompileOptions<'a>,
                      exec: Arc<Executor>)
                      -> CargoResult<ops::Compilation<'a>> {
    let CompileOptions { config, jobs, target, spec, features,
                         all_features, no_default_features,
                         release, mode, message_format,
                         ref filter,
                         ref target_rustdoc_args,
                         ref target_rustc_args } = *options;

    let target = target.map(|s| s.to_string());

    if jobs == Some(0) {
        bail!("jobs must be at least 1")
    }

    let profiles = ws.profiles();

    let specs = spec.into_package_id_specs(ws)?;
    let resolve = ops::resolve_ws_precisely(ws,
                                            source,
                                            features,
                                            all_features,
                                            no_default_features,
                                            &specs)?;
    let (packages, resolve_with_overrides) = resolve;

    let mut pkgids = Vec::new();
    if specs.len() > 0 {
        for p in specs.iter() {
            pkgids.push(p.query(resolve_with_overrides.iter())?);
        }
    } else {
        let root_package = ws.current()?;
        let all_features = resolve_with_overrides.features(root_package.package_id());
        generate_targets(root_package, profiles, mode, filter, all_features, release)?;
        pkgids.push(root_package.package_id());
    };

    let to_builds = pkgids.iter().map(|id| {
        packages.get(id)
    }).collect::<CargoResult<Vec<_>>>()?;

    let mut general_targets = Vec::new();
    let mut package_targets = Vec::new();

    match (*target_rustc_args, *target_rustdoc_args) {
        (Some(..), _) |
        (_, Some(..)) if to_builds.len() != 1 => {
            panic!("`rustc` and `rustdoc` should not accept multiple `-p` flags")
        }
        (Some(args), _) => {
            let all_features = resolve_with_overrides.features(to_builds[0].package_id());
            let targets = generate_targets(to_builds[0], profiles,
                                           mode, filter, all_features, release)?;
            if targets.len() == 1 {
                let (target, profile) = targets[0];
                let mut profile = profile.clone();
                profile.rustc_args = Some(args.to_vec());
                general_targets.push((target, profile));
            } else {
                bail!("extra arguments to `rustc` can only be passed to one \
                       target, consider filtering\nthe package by passing \
                       e.g. `--lib` or `--bin NAME` to specify a single target")
            }
        }
        (None, Some(args)) => {
            let all_features = resolve_with_overrides.features(to_builds[0].package_id());
            let targets = generate_targets(to_builds[0], profiles,
                                           mode, filter, all_features, release)?;
            if targets.len() == 1 {
                let (target, profile) = targets[0];
                let mut profile = profile.clone();
                profile.rustdoc_args = Some(args.to_vec());
                general_targets.push((target, profile));
            } else {
                bail!("extra arguments to `rustdoc` can only be passed to one \
                       target, consider filtering\nthe package by passing e.g. \
                       `--lib` or `--bin NAME` to specify a single target")
            }
        }
        (None, None) => {
            for &to_build in to_builds.iter() {
                let all_features = resolve_with_overrides.features(to_build.package_id());
                let targets = generate_targets(to_build, profiles, mode,
                                               filter, all_features, release)?;
                package_targets.push((to_build, targets));
            }
        }
    };

    for &(target, ref profile) in &general_targets {
        for &to_build in to_builds.iter() {
            package_targets.push((to_build, vec![(target, profile)]));
        }
    }

    let mut ret = {
        let _p = profile::start("compiling");
        let mut build_config = scrape_build_config(config, jobs, target)?;
        build_config.release = release;
        build_config.test = mode == CompileMode::Test || mode == CompileMode::Bench;
        build_config.json_messages = message_format == MessageFormat::Json;
        if let CompileMode::Doc { deps } = mode {
            build_config.doc_all = deps;
        }

        ops::compile_targets(ws,
                             &package_targets,
                             &packages,
                             &resolve_with_overrides,
                             config,
                             build_config,
                             profiles,
                             exec)?
    };

    ret.to_doc_test = to_builds.iter().map(|&p| p.clone()).collect();

    Ok(ret)
}

impl<'a> CompileFilter<'a> {
    pub fn new(lib_only: bool,
               bins: &'a [String],
               tests: &'a [String],
               examples: &'a [String],
               benches: &'a [String]) -> CompileFilter<'a> {
        if lib_only || !bins.is_empty() || !tests.is_empty() ||
           !examples.is_empty() || !benches.is_empty() {
            CompileFilter::Only {
                lib: lib_only, bins: bins, examples: examples, benches: benches,
                tests: tests,
            }
        } else {
            CompileFilter::Everything
        }
    }

    pub fn matches(&self, target: &Target) -> bool {
        match *self {
            CompileFilter::Everything => true,
            CompileFilter::Only { lib, bins, examples, tests, benches } => {
                let list = match *target.kind() {
                    TargetKind::Bin => bins,
                    TargetKind::Test => tests,
                    TargetKind::Bench => benches,
                    TargetKind::ExampleBin |
                    TargetKind::ExampleLib(..) => examples,
                    TargetKind::Lib(..) => return lib,
                    TargetKind::CustomBuild => return false,
                };
                list.iter().any(|x| *x == target.name())
            }
        }
    }
}

/// Given the configuration for a build, this function will generate all
/// target/profile combinations needed to be built.
fn generate_targets<'a>(pkg: &'a Package,
                        profiles: &'a Profiles,
                        mode: CompileMode,
                        filter: &CompileFilter,
                        features: Option<&HashSet<String>>,
                        release: bool)
                        -> CargoResult<Vec<(&'a Target, &'a Profile)>> {
    let build = if release {&profiles.release} else {&profiles.dev};
    let test = if release {&profiles.bench} else {&profiles.test};
    let profile = match mode {
        CompileMode::Test => test,
        CompileMode::Bench => &profiles.bench,
        CompileMode::Build => build,
        CompileMode::Check => &profiles.check,
        CompileMode::Doc { .. } => &profiles.doc,
        CompileMode::Doctest => &profiles.doctest,
    };
    let mut targets = match *filter {
        CompileFilter::Everything => {
            match mode {
                CompileMode::Bench => {
                    pkg.targets().iter().filter(|t| t.benched()).map(|t| {
                        (t, profile)
                    }).collect::<Vec<_>>()
                }
                CompileMode::Test => {
                    let deps = if release {
                        &profiles.bench_deps
                    } else {
                        &profiles.test_deps
                    };
                    let mut base = pkg.targets().iter().filter(|t| {
                        t.tested()
                    }).map(|t| {
                        (t, if t.is_example() {deps} else {profile})
                    }).collect::<Vec<_>>();

                    // Always compile the library if we're testing everything as
                    // it'll be needed for doctests
                    if let Some(t) = pkg.targets().iter().find(|t| t.is_lib()) {
                        if t.doctested() {
                            base.push((t, deps));
                        }
                    }
                    base
                }
                CompileMode::Build | CompileMode::Check => {
                    pkg.targets().iter().filter(|t| {
                        t.is_bin() || t.is_lib()
                    }).map(|t| (t, profile)).collect()
                }
                CompileMode::Doc { .. } => {
                    pkg.targets().iter().filter(|t| t.documented())
                       .map(|t| (t, profile)).collect()
                }
                CompileMode::Doctest => {
                    if let Some(t) = pkg.targets().iter().find(|t| t.is_lib()) {
                        if t.doctested() {
                            return Ok(vec![(t, profile)])
                        }
                    }

                    Vec::new()
                }
            }
        }
        CompileFilter::Only { lib, bins, examples, tests, benches } => {
            let mut targets = Vec::new();

            if lib {
                if let Some(t) = pkg.targets().iter().find(|t| t.is_lib()) {
                    targets.push((t, profile));
                } else {
                    bail!("no library targets found")
                }
            }

            {
                let mut find = |names: &[String],
                                desc,
                                is_expected_kind: fn(&Target) -> bool,
                                profile| {
                    for name in names {
                        let target = pkg.targets().iter().find(|t| {
                            t.name() == *name && is_expected_kind(t)
                        });
                        let t = match target {
                            Some(t) => t,
                            None => {
                                let suggestion = pkg.find_closest_target(name, is_expected_kind);
                                match suggestion {
                                    Some(s) => {
                                        let suggested_name = s.name();
                                        bail!("no {} target named `{}`\n\nDid you mean `{}`?",
                                              desc, name, suggested_name)
                                    }
                                    None => bail!("no {} target named `{}`", desc, name),
                                }
                            }
                        };
                        debug!("found {} `{}`", desc, name);

                        targets.push((t, profile));
                    }
                    Ok(())
                };
                find(bins, "bin", Target::is_bin, profile)?;
                find(examples, "example", Target::is_example, build)?;
                find(tests, "test", Target::is_test, test)?;
                find(benches, "bench", Target::is_bench, &profiles.bench)?;
            }
            targets
        }
    };

    //Collect the targets that are libraries or have all required features available.
    let no_features = HashSet::new();
    let features = features.unwrap_or(&no_features);
    let mut compatible_targets = Vec::with_capacity(targets.len());
    for (target, profile) in targets.drain(0..) {
        if target.is_lib() || match target.required_features() {
            Some(f) => !f.iter().any(|f| !features.contains(f)),
            None => true,
        } {
            compatible_targets.push((target, profile));
            continue;
        }

        if let CompileFilter::Only { .. } = *filter {
            let required_features = target.required_features().unwrap();
            let quoted_required_features: Vec<String> = required_features.iter()
                                                                         .map(|s| format!("`{}`",s))
                                                                         .collect();
            bail!("target `{}` requires the features: {}\n\
                  Consider enabling them by passing e.g. `--features=\"{}\"`",
                  target.name(),
                  quoted_required_features.join(", "),
                  required_features.join(" "));
        }
    }

    Ok(compatible_targets)
}

/// Parse all config files to learn about build configuration. Currently
/// configured options are:
///
/// * build.jobs
/// * build.target
/// * target.$target.ar
/// * target.$target.linker
/// * target.$target.libfoo.metadata
fn scrape_build_config(config: &Config,
                       jobs: Option<u32>,
                       target: Option<String>)
                       -> CargoResult<ops::BuildConfig> {
    let cfg_jobs = match config.get_i64("build.jobs")? {
        Some(v) => {
            if v.val <= 0 {
                bail!("build.jobs must be positive, but found {} in {}",
                      v.val, v.definition)
            } else if v.val >= u32::max_value() as i64 {
                bail!("build.jobs is too large: found {} in {}", v.val,
                      v.definition)
            } else {
                Some(v.val as u32)
            }
        }
        None => None,
    };
    let jobs = jobs.or(cfg_jobs).unwrap_or(::num_cpus::get() as u32);
    let cfg_target = config.get_string("build.target")?.map(|s| s.val);
    let target = target.or(cfg_target);
    let mut base = ops::BuildConfig {
        host_triple: config.rustc()?.host.clone(),
        requested_target: target.clone(),
        jobs: jobs,
        ..Default::default()
    };
    base.host = scrape_target_config(config, &base.host_triple)?;
    base.target = match target.as_ref() {
        Some(triple) => scrape_target_config(config, &triple)?,
        None => base.host.clone(),
    };
    Ok(base)
}

fn scrape_target_config(config: &Config, triple: &str)
                        -> CargoResult<ops::TargetConfig> {

    let key = format!("target.{}", triple);
    let mut ret = ops::TargetConfig {
        ar: config.get_path(&format!("{}.ar", key))?.map(|v| v.val),
        linker: config.get_path(&format!("{}.linker", key))?.map(|v| v.val),
        overrides: HashMap::new(),
    };
    let table = match config.get_table(&key)? {
        Some(table) => table.val,
        None => return Ok(ret),
    };
    for (lib_name, value) in table {
        if lib_name == "ar" || lib_name == "linker" || lib_name == "rustflags" {
            continue
        }

        let mut output = BuildOutput {
            library_paths: Vec::new(),
            library_links: Vec::new(),
            cfgs: Vec::new(),
            metadata: Vec::new(),
            rerun_if_changed: Vec::new(),
            warnings: Vec::new(),
        };
        for (k, value) in value.table(&lib_name)?.0 {
            let key = format!("{}.{}", key, k);
            match &k[..] {
                "rustc-flags" => {
                    let (flags, definition) = value.string(&k)?;
                    let whence = format!("in `{}` (in {})", key,
                                         definition.display());
                    let (paths, links) =
                        BuildOutput::parse_rustc_flags(&flags, &whence)
                    ?;
                    output.library_paths.extend(paths);
                    output.library_links.extend(links);
                }
                "rustc-link-lib" => {
                    let list = value.list(&k)?;
                    output.library_links.extend(list.iter()
                                                    .map(|v| v.0.clone()));
                }
                "rustc-link-search" => {
                    let list = value.list(&k)?;
                    output.library_paths.extend(list.iter().map(|v| {
                        PathBuf::from(&v.0)
                    }));
                }
                "rustc-cfg" => {
                    let list = value.list(&k)?;
                    output.cfgs.extend(list.iter().map(|v| v.0.clone()));
                }
                "warning" | "rerun-if-changed" => {
                    bail!("`{}` is not supported in build script overrides", k);
                }
                _ => {
                    let val = value.string(&k)?.0;
                    output.metadata.push((k.clone(), val.to_string()));
                }
            }
        }
        ret.overrides.insert(lib_name, output);
    }

    Ok(ret)
}
