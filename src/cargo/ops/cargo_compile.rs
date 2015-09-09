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

use std::collections::HashMap;
use std::default::Default;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use core::registry::PackageRegistry;
use core::{Source, SourceId, SourceMap, PackageSet, Package, Target};
use core::{Profile, TargetKind, Profiles};
use core::resolver::{Method, Resolve};
use ops::{self, BuildOutput, ExecEngine};
use util::config::{ConfigValue, Config};
use util::{CargoResult, internal, human, ChainError, profile};

/// Contains information about how a package should be compiled.
pub struct CompileOptions<'a> {
    pub config: &'a Config,
    /// Number of concurrent jobs to use.
    pub jobs: Option<u32>,
    /// The target platform to compile for (example: `i686-unknown-linux-gnu`).
    pub target: Option<&'a str>,
    /// Extra features to build for the root package
    pub features: &'a [String],
    /// Flag if the default feature should be built for the root package
    pub no_default_features: bool,
    /// Root package to build (if None it's the current one)
    pub spec: &'a [String],
    /// Filter to apply to the root package to select which targets will be
    /// built.
    pub filter: CompileFilter<'a>,
    /// Engine which drives compilation
    pub exec_engine: Option<Arc<Box<ExecEngine>>>,
    /// Whether this is a release build or not
    pub release: bool,
    /// Mode for this compile.
    pub mode: CompileMode,
    /// Extra arguments to be passed to rustdoc (for main crate and dependencies)
    pub extra_rustdoc_args: Vec<String>,
    /// The specified target will be compiled with all the available arguments,
    /// note that this only accounts for the *final* invocation of rustc
    pub target_rustc_args: Option<&'a [String]>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum CompileMode {
    Test,
    Build,
    Bench,
    Doc { deps: bool },
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

pub fn compile<'a>(manifest_path: &Path,
                   options: &CompileOptions<'a>)
                   -> CargoResult<ops::Compilation<'a>> {
    debug!("compile; manifest-path={}", manifest_path.display());

    let package = try!(Package::for_path(manifest_path, options.config));
    debug!("loaded package; package={}", package);

    for key in package.manifest().warnings().iter() {
        try!(options.config.shell().warn(key))
    }
    compile_pkg(&package, None, options)
}

pub fn resolve_dependencies<'a>(root_package: &Package,
                                config: &'a Config,
                                source: Option<Box<Source + 'a>>,
                                features: Vec<String>,
                                no_default_features: bool)
                                -> CargoResult<(Vec<Package>, Resolve, SourceMap<'a>)> {

    let override_ids = try!(source_ids_from_config(config, root_package.root()));

    let mut registry = PackageRegistry::new(config);

    if let Some(source) = source {
        registry.add_preloaded(root_package.package_id().source_id(), source);
    }

    // First, resolve the root_package's *listed* dependencies, as well as
    // downloading and updating all remotes and such.
    let resolve = try!(ops::resolve_pkg(&mut registry, root_package));

    // Second, resolve with precisely what we're doing. Filter out
    // transitive dependencies if necessary, specify features, handle
    // overrides, etc.
    let _p = profile::start("resolving w/ overrides...");

    try!(registry.add_overrides(override_ids));

    let method = Method::Required{
        dev_deps: true, // TODO: remove this option?
        features: &features,
        uses_default_features: !no_default_features,
    };

    let resolved_with_overrides =
            try!(ops::resolve_with_previous(&mut registry, root_package,
                                            method, Some(&resolve), None));

    let packages = try!(ops::get_resolved_packages(&resolved_with_overrides,
                                                   &mut registry));

    Ok((packages, resolved_with_overrides, registry.move_sources()))
}

#[allow(deprecated)] // connect => join in 1.3
pub fn compile_pkg<'a>(root_package: &Package,
                       source: Option<Box<Source + 'a>>,
                       options: &CompileOptions<'a>)
                       -> CargoResult<ops::Compilation<'a>> {
    let CompileOptions { config, jobs, target, spec, features,
                         no_default_features, release, mode,
                         ref filter, ref exec_engine,
                         ref extra_rustdoc_args,
                         ref target_rustc_args } = *options;

    let target = target.map(|s| s.to_string());
    let features = features.iter().flat_map(|s| {
        s.split(' ')
    }).map(|s| s.to_string()).collect::<Vec<String>>();

    if jobs == Some(0) {
        return Err(human("jobs must be at least 1"))
    }

    let (packages, resolve_with_overrides, sources) = {
        try!(resolve_dependencies(root_package, config, source, features,
                                  no_default_features))
    };

    let mut invalid_spec = vec![];
    let pkgids = if spec.len() > 0 {
        spec.iter().filter_map(|p| {
            match resolve_with_overrides.query(&p) {
                Ok(p) => Some(p),
                Err(..) => { invalid_spec.push(p.to_string()); None }
            }
        }).collect::<Vec<_>>()
    } else {
        vec![root_package.package_id()]
    };

    if spec.len() > 0 && invalid_spec.len() > 0 {
        return Err(human(format!("could not find package matching spec `{}`",
                                 invalid_spec.connect(", "))));
    }

    let to_builds = packages.iter().filter(|p| pkgids.contains(&p.package_id()))
                            .collect::<Vec<_>>();

    let mut general_targets = Vec::new();
    let mut package_targets = Vec::new();

    let profiles = root_package.manifest().profiles();
    match *target_rustc_args {
        Some(args) => {
            if to_builds.len() == 1 {
                let targets = try!(generate_targets(to_builds[0], profiles,
                                                    mode, filter, release));
                if targets.len() == 1 {
                    let (target, profile) = targets[0];
                    let mut profile = profile.clone();
                    profile.rustc_args = Some(args.to_vec());
                    general_targets.push((target, profile));
                } else {
                    return Err(human("extra arguments to `rustc` can only be \
                                      passed to one target, consider \
                                      filtering\nthe package by passing e.g. \
                                      `--lib` or `--bin NAME` to specify \
                                      a single target"))

                }
            } else {
                panic!("`rustc` should not accept multiple `-p` flags")
            }
        }
        None => {
            for &to_build in to_builds.iter() {
                let targets = try!(generate_targets(to_build, profiles, mode,
                                                    filter, release));
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
    let mut target_with_rustdoc = None;
    if !extra_rustdoc_args.is_empty() {
        let mut target_with_rustdoc_inner = Vec::new();
        for &(target, profile) in &targets {
            if profile.doc {
                let mut profile = profile.clone();
                profile.rustdoc_args = Some(extra_rustdoc_args.clone());
                target_with_rustdoc_inner.push((target, profile));
            }
        }
        target_with_rustdoc = Some(target_with_rustdoc_inner);
    };

    let targets = target_with_rustdoc.as_ref().map_or(targets,
                                             |o| o.into_iter()
                                                  .map(|&(t, ref p)| (t, p))
                                                  .collect());
    let ret = {
        let _p = profile::start("compiling");
        let mut build_config = try!(scrape_build_config(config, jobs, target));
        build_config.exec_engine = exec_engine.clone();
        build_config.release = release;
        if let CompileMode::Doc { deps } = mode {
            build_config.doc_all = deps;
        }

        try!(ops::compile_targets(&package_targets,
                                  &PackageSet::new(&packages),
                                  &resolve_with_overrides,
                                  &sources,
                                  config,
                                  build_config,
                                  root_package.manifest().profiles(),
                                  ))
    };

    ret.to_doc_test = to_builds.iter().map(|&p| p.clone()).collect();

    return Ok(ret);
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
                    TargetKind::Example => examples,
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
                        release: bool)
                        -> CargoResult<Vec<(&'a Target, &'a Profile)>> {
    let build = if release {&profiles.release} else {&profiles.dev};
    let test = if release {&profiles.bench} else {&profiles.test};
    let profile = match mode {
        CompileMode::Test => test,
        CompileMode::Bench => &profiles.bench,
        CompileMode::Build => build,
        CompileMode::Doc { .. } => &profiles.doc,
    };
    return match *filter {
        CompileFilter::Everything => {
            match mode {
                CompileMode::Bench => {
                    Ok(pkg.targets().iter().filter(|t| t.benched()).map(|t| {
                        (t, profile)
                    }).collect::<Vec<_>>())
                }
                CompileMode::Test => {
                    let mut base = pkg.targets().iter().filter(|t| {
                        t.tested()
                    }).map(|t| {
                        (t, if t.is_example() {build} else {profile})
                    }).collect::<Vec<_>>();

                    // Always compile the library if we're testing everything as
                    // it'll be needed for doctests
                    if let Some(t) = pkg.targets().iter().find(|t| t.is_lib()) {
                        if t.doctested() {
                            base.push((t, build));
                        }
                    }
                    Ok(base)
                }
                CompileMode::Build => {
                    Ok(pkg.targets().iter().filter(|t| {
                        t.is_bin() || t.is_lib()
                    }).map(|t| (t, profile)).collect())
                }
                CompileMode::Doc { .. } => {
                    Ok(pkg.targets().iter().filter(|t| t.documented())
                          .map(|t| (t, profile)).collect())
                }
            }
        }
        CompileFilter::Only { lib, bins, examples, tests, benches } => {
            let mut targets = Vec::new();

            if lib {
                if let Some(t) = pkg.targets().iter().find(|t| t.is_lib()) {
                    targets.push((t, profile));
                } else {
                    return Err(human(format!("no library targets found")))
                }
            }

            {
                let mut find = |names: &[String], desc, kind, profile| {
                    for name in names {
                        let target = pkg.targets().iter().find(|t| {
                            t.name() == *name && *t.kind() == kind
                        });
                        let t = match target {
                            Some(t) => t,
                            None => return Err(human(format!("no {} target \
                                                              named `{}`",
                                                             desc, name))),
                        };
                        debug!("found {} `{}`", desc, name);
                        targets.push((t, profile));
                    }
                    Ok(())
                };
                try!(find(bins, "bin", TargetKind::Bin, profile));
                try!(find(examples, "example", TargetKind::Example, build));
                try!(find(tests, "test", TargetKind::Test, test));
                try!(find(benches, "bench", TargetKind::Bench, &profiles.bench));
            }
            Ok(targets)
        }
    };
}

/// Read the `paths` configuration variable to discover all path overrides that
/// have been configured.
fn source_ids_from_config(config: &Config, cur_path: &Path)
                          -> CargoResult<Vec<SourceId>> {

    let configs = try!(config.values());
    debug!("loaded config; configs={:?}", configs);
    let config_paths = match configs.get("paths") {
        Some(cfg) => cfg,
        None => return Ok(Vec::new())
    };
    let paths = try!(config_paths.list().chain_error(|| {
        internal("invalid configuration for the key `paths`")
    }));

    paths.iter().map(|&(ref s, ref p)| {
        // The path listed next to the string is the config file in which the
        // key was located, so we want to pop off the `.cargo/config` component
        // to get the directory containing the `.cargo` folder.
        p.parent().unwrap().parent().unwrap().join(s)
    }).filter(|p| {
        // Make sure we don't override the local package, even if it's in the
        // list of override paths.
        cur_path != &**p
    }).map(|p| SourceId::for_path(&p)).collect()
}

/// Parse all config files to learn about build configuration. Currently
/// configured options are:
///
/// * build.jobs
/// * target.$target.ar
/// * target.$target.linker
/// * target.$target.libfoo.metadata
fn scrape_build_config(config: &Config,
                       jobs: Option<u32>,
                       target: Option<String>)
                       -> CargoResult<ops::BuildConfig> {
    let cfg_jobs = match try!(config.get_i64("build.jobs")) {
        Some((n, p)) => {
            if n <= 0 {
                return Err(human(format!("build.jobs must be positive, \
                                          but found {} in {:?}", n, p)));
            } else if n >= u32::max_value() as i64 {
                return Err(human(format!("build.jobs is too large: \
                                          found {} in {:?}", n, p)));
            } else {
                Some(n as u32)
            }
        }
        None => None,
    };
    let jobs = jobs.or(cfg_jobs).unwrap_or(::num_cpus::get() as u32);
    let mut base = ops::BuildConfig {
        jobs: jobs,
        requested_target: target.clone(),
        ..Default::default()
    };
    base.host = try!(scrape_target_config(config, &config.rustc_info().host));
    base.target = match target.as_ref() {
        Some(triple) => try!(scrape_target_config(config, &triple)),
        None => base.host.clone(),
    };
    Ok(base)
}

fn scrape_target_config(config: &Config, triple: &str)
                        -> CargoResult<ops::TargetConfig> {

    let key = format!("target.{}", triple);
    let mut ret = ops::TargetConfig {
        ar: try!(config.get_path(&format!("{}.ar", key))),
        linker: try!(config.get_path(&format!("{}.linker", key))),
        overrides: HashMap::new(),
    };
    let table = match try!(config.get_table(&key)) {
        Some((table, _)) => table,
        None => return Ok(ret),
    };
    for (lib_name, _) in table.into_iter() {
        if lib_name == "ar" || lib_name == "linker" { continue }

        let mut output = BuildOutput {
            library_paths: Vec::new(),
            library_links: Vec::new(),
            cfgs: Vec::new(),
            metadata: Vec::new(),
        };
        let key = format!("{}.{}", key, lib_name);
        let table = try!(config.get_table(&key)).unwrap().0;
        for (k, _) in table.into_iter() {
            let key = format!("{}.{}", key, k);
            match try!(config.get(&key)).unwrap() {
                ConfigValue::String(v, path) => {
                    if k == "rustc-flags" {
                        let whence = format!("in `{}` (in {})", key,
                                             path.display());
                        let (paths, links) = try!(
                            BuildOutput::parse_rustc_flags(&v, &whence)
                        );
                        output.library_paths.extend(paths.into_iter());
                        output.library_links.extend(links.into_iter());
                    } else {
                        output.metadata.push((k, v));
                    }
                },
                ConfigValue::List(a, p) => {
                    if k == "rustc-link-lib" {
                        output.library_links.extend(a.into_iter().map(|v| v.0));
                    } else if k == "rustc-link-search" {
                        output.library_paths.extend(a.into_iter().map(|v| {
                            PathBuf::from(&v.0)
                        }));
                    } else if k == "rustc-cfg" {
                        output.cfgs.extend(a.into_iter().map(|v| v.0));
                    } else {
                        try!(config.expected("string", &k,
                                             ConfigValue::List(a, p)));
                    }
                },
                // technically could be a list too, but that's the exception to
                // the rule...
                cv => { try!(config.expected("string", &k, cv)); }
            }
        }
        ret.overrides.insert(lib_name, output);
    }

    Ok(ret)
}
