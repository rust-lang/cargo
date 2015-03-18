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
use std::num::ToPrimitive;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use core::registry::PackageRegistry;
use core::{Source, SourceId, PackageSet, Package, Target, PackageId};
use core::resolver::Method;
use ops::{self, BuildOutput, ExecEngine};
use sources::{PathSource};
use util::config::{ConfigValue, Config};
use util::{CargoResult, internal, human, ChainError, profile};

/// Contains informations about how a package should be compiled.
pub struct CompileOptions<'a, 'b: 'a> {
    pub env: &'a str,
    pub config: &'a Config<'b>,
    /// Number of concurrent jobs to use.
    pub jobs: Option<u32>,
    /// The target platform to compile for (example: `i686-unknown-linux-gnu`).
    pub target: Option<&'a str>,
    /// True if dev-dependencies must be compiled.
    pub dev_deps: bool,
    pub features: &'a [String],
    pub no_default_features: bool,
    pub spec: Option<&'a str>,
    pub lib_only: bool,
    pub exec_engine: Option<Arc<Box<ExecEngine>>>,
}

pub fn compile(manifest_path: &Path,
               options: &CompileOptions)
               -> CargoResult<ops::Compilation> {
    debug!("compile; manifest-path={}", manifest_path.display());

    let mut source = try!(PathSource::for_path(manifest_path.parent().unwrap(),
                                               options.config));
    try!(source.update());

    // TODO: Move this into PathSource
    let package = try!(source.root_package());
    debug!("loaded package; package={}", package);

    for key in package.manifest().warnings().iter() {
        try!(options.config.shell().warn(key))
    }
    compile_pkg(&package, options)
}

pub fn compile_pkg(package: &Package, options: &CompileOptions)
                   -> CargoResult<ops::Compilation> {
    let CompileOptions { env, config, jobs, target, spec,
                         dev_deps, features, no_default_features,
                         lib_only, ref exec_engine } = *options;

    let target = target.map(|s| s.to_string());
    let features = features.iter().flat_map(|s| {
        s.split(' ')
    }).map(|s| s.to_string()).collect::<Vec<String>>();

    if spec.is_some() && (no_default_features || features.len() > 0) {
        return Err(human("features cannot be modified when the main package \
                          is not being built"))
    }
    if jobs == Some(0) {
        return Err(human("jobs must be at least 1"))
    }

    let override_ids = try!(source_ids_from_config(config, package.root()));

    let (packages, resolve_with_overrides, sources) = {
        let rustc_host = config.rustc_host().to_string();
        let mut registry = PackageRegistry::new(config);

        // First, resolve the package's *listed* dependencies, as well as
        // downloading and updating all remotes and such.
        let resolve = try!(ops::resolve_pkg(&mut registry, package));

        // Second, resolve with precisely what we're doing. Filter out
        // transitive dependencies if necessary, specify features, handle
        // overrides, etc.
        let _p = profile::start("resolving w/ overrides...");

        try!(registry.add_overrides(override_ids));

        let platform = target.as_ref().map(|e| e.as_slice()).or(Some(rustc_host.as_slice()));

        let method = Method::Required{
            dev_deps: dev_deps,
            features: &features,
            uses_default_features: !no_default_features,
            target_platform: platform};

        let resolved_with_overrides =
                try!(ops::resolve_with_previous(&mut registry, package, method,
                                                Some(&resolve), None));

        let req: Vec<PackageId> = resolved_with_overrides.iter().map(|r| {
            r.clone()
        }).collect();
        let packages = try!(registry.get(&req).chain_error(|| {
            human("Unable to get packages from source")
        }));

        (packages, resolved_with_overrides, registry.move_sources())
    };

    let pkgid = match spec {
        Some(spec) => try!(resolve_with_overrides.query(spec)),
        None => package.package_id(),
    };
    let to_build = packages.iter().find(|p| p.package_id() == pkgid).unwrap();
    let targets = to_build.targets().iter().filter(|target| {
        target.profile().is_custom_build() || match env {
            // doc-all == document everything, so look for doc targets
            "doc" | "doc-all" => target.profile().env() == "doc",
            env => target.profile().env() == env,
        }
    }).filter(|target| !lib_only || target.is_lib()).collect::<Vec<&Target>>();

    if lib_only && targets.len() == 0 {
        return Err(human("There is no lib to build, remove `--lib` flag".to_string()));
    }

    let ret = {
        let _p = profile::start("compiling");
        let lib_overrides = try!(scrape_build_config(config, jobs, target));

        try!(ops::compile_targets(&env, &targets, to_build,
                                  &PackageSet::new(&packages),
                                  &resolve_with_overrides, &sources,
                                  config, lib_overrides, exec_engine.clone()))
    };

    return Ok(ret);
}

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

fn scrape_build_config(config: &Config,
                       jobs: Option<u32>,
                       target: Option<String>) -> CargoResult<ops::BuildConfig> {
    let cfg_jobs = match try!(config.get_i64("build.jobs")) {
        Some((n, p)) => {
            match n.to_u32() {
                Some(n) => Some(n),
                None if n <= 0 => {
                    return Err(human(format!("build.jobs must be positive, \
                                              but found {} in {:?}", n, p)));
                }
                None => {
                    return Err(human(format!("build.jobs is too large: \
                                              found {} in {:?}", n, p)));
                }
            }
        }
        None => None,
    };
    #[allow(deprecated)]
    fn num_cpus() -> u32 { ::std::os::num_cpus() as u32 }
    let jobs = jobs.or(cfg_jobs).unwrap_or(num_cpus());
    let mut base = ops::BuildConfig {
        jobs: jobs,
        requested_target: target.clone(),
        ..Default::default()
    };
    base.host = try!(scrape_target_config(config, config.rustc_host()));
    base.target = match target.as_ref() {
        Some(triple) => try!(scrape_target_config(config, &triple)),
        None => base.host.clone(),
    };
    Ok(base)
}

fn scrape_target_config(config: &Config, triple: &str)
                        -> CargoResult<ops::TargetConfig> {

    let key = format!("target.{}", triple);
    let ar = try!(config.get_string(&format!("{}.ar", key)));
    let linker = try!(config.get_string(&format!("{}.linker", key)));

    let mut ret = ops::TargetConfig {
        ar: ar.map(|p| p.0),
        linker: linker.map(|p| p.0),
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
            metadata: Vec::new(),
        };
        let key = format!("{}.{}", key, lib_name);
        let table = try!(config.get_table(&key)).unwrap().0;
        for (k, _) in table.into_iter() {
            let key = format!("{}.{}", key, k);
            match try!(config.get(&key)).unwrap() {
                ConfigValue::String(v, path) => {
                    if k == "rustc-flags" {
                        let whence = format!("in `{}` (in {:?})", key, path);
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
                        output.library_links.extend(a.into_iter().map(|(v, _)| v));
                    } else if k == "rustc-link-search" {
                        output.library_paths.extend(a.into_iter().map(|(v, _)| PathBuf::new(&v)));
                    } else {
                        try!(config.expected("string", &k, ConfigValue::List(a, p)));
                    }
                },
                // technically could be a list too, but that's the exception to the rule...
                cv => { try!(config.expected("string", &k, cv)); }
            }
        }
        ret.overrides.insert(lib_name, output);
    }

    Ok(ret)
}
