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

use std::os;
use std::collections::HashMap;

use core::registry::PackageRegistry;
use core::{MultiShell, Source, SourceId, PackageSet, Package, Target, PackageId};
use core::resolver;
use ops::{mod, BuildOutput};
use sources::{PathSource};
use util::config::{Config, ConfigValue};
use util::{CargoResult, Wrap, config, internal, human, ChainError, profile};

/// Contains informations about how a package should be compiled.
pub struct CompileOptions<'a> {
    pub env: &'a str,
    pub shell: &'a mut MultiShell,
    /// Number of concurrent jobs to use.
    pub jobs: Option<uint>,
    /// The target platform to compile for (example: `i686-unknown-linux-gnu`).
    pub target: Option<&'a str>,
    /// True if dev-dependencies must be compiled.
    pub dev_deps: bool,
    pub features: &'a [String],
    pub no_default_features: bool,
    pub spec: Option<&'a str>,
}

pub fn compile(manifest_path: &Path,
               options: &mut CompileOptions)
               -> CargoResult<ops::Compilation> {
    log!(4, "compile; manifest-path={}", manifest_path.display());

    let mut source = try!(PathSource::for_path(&manifest_path.dir_path()));
    try!(source.update());

    // TODO: Move this into PathSource
    let package = try!(source.get_root_package());
    debug!("loaded package; package={}", package);

    for key in package.get_manifest().get_warnings().iter() {
        try!(options.shell.warn(key))
    }
    compile_pkg(&package, options)
}

pub fn compile_pkg(package: &Package, options: &mut CompileOptions)
                   -> CargoResult<ops::Compilation> {
    let CompileOptions { env, ref mut shell, jobs, target, spec,
                         dev_deps, features, no_default_features } = *options;
    let target = target.map(|s| s.to_string());
    let features = features.iter().flat_map(|s| {
        s.as_slice().split(' ')
    }).map(|s| s.to_string()).collect::<Vec<String>>();

    if spec.is_some() && (no_default_features || features.len() > 0) {
        return Err(human("features cannot be modified when the main package \
                          is not being built"))
    }

    let user_configs = try!(config::all_configs(os::getcwd()));
    let override_ids = try!(source_ids_from_config(&user_configs,
                                                   package.get_root()));
    let config = try!(Config::new(*shell, jobs, target.clone()));

    let (packages, resolve_with_overrides, sources) = {
        let rustc_host = config.rustc_host().to_string();
        let mut registry = PackageRegistry::new(&config);

        // First, resolve the package's *listed* dependencies, as well as
        // downloading and updating all remotes and such.
        let resolve = try!(ops::resolve_pkg(&mut registry, package));

        // Second, resolve with precisely what we're doing. Filter out
        // transitive dependencies if necessary, specify features, handle
        // overrides, etc.
        let _p = profile::start("resolving w/ overrides...");

        try!(registry.add_overrides(override_ids));

        let platform = target.as_ref().map(|e| e.as_slice()).or(Some(rustc_host.as_slice()));
        let method = resolver::ResolveRequired(dev_deps, features.as_slice(),
                                               !no_default_features,
                                               platform);
        let resolved_with_overrides =
                try!(ops::resolve_with_previous(&mut registry, package, method,
                                                Some(&resolve), None));

        let req: Vec<PackageId> = resolved_with_overrides.iter().map(|r| {
            r.clone()
        }).collect();
        let packages = try!(registry.get(req.as_slice()).wrap({
            human("Unable to get packages from source")
        }));

        (packages, resolved_with_overrides, registry.move_sources())
    };

    debug!("packages={}", packages);

    let to_build = match spec {
        Some(spec) => {
            let pkgid = try!(resolve_with_overrides.query(spec));
            packages.iter().find(|p| p.get_package_id() == pkgid).unwrap()
        }
        None => package,
    };

    let targets = to_build.get_targets().iter().filter(|target| {
        match env {
            // doc-all == document everything, so look for doc targets
            "doc" | "doc-all" => target.get_profile().get_env() == "doc",
            env => target.get_profile().get_env() == env,
        }
    }).collect::<Vec<&Target>>();

    let ret = {
        let _p = profile::start("compiling");
        let lib_overrides = try!(scrape_target_config(&config, &user_configs));

        try!(ops::compile_targets(env.as_slice(), targets.as_slice(), to_build,
                                  &PackageSet::new(packages.as_slice()),
                                  &resolve_with_overrides, &sources,
                                  &config, lib_overrides))
    };

    return Ok(ret);
}

fn source_ids_from_config(configs: &HashMap<String, config::ConfigValue>,
                          cur_path: Path) -> CargoResult<Vec<SourceId>> {
    debug!("loaded config; configs={}", configs);

    let config_paths = match configs.find_equiv("paths") {
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
        p.dir_path().dir_path().join(s.as_slice())
    }).filter(|p| {
        // Make sure we don't override the local package, even if it's in the
        // list of override paths.
        cur_path != *p
    }).map(|p| SourceId::for_path(&p)).collect()
}

fn scrape_target_config(config: &Config,
                        configs: &HashMap<String, config::ConfigValue>)
                        -> CargoResult<HashMap<String, BuildOutput>> {
    let target = match configs.find_equiv("target") {
        None => return Ok(HashMap::new()),
        Some(target) => try!(target.table().chain_error(|| {
            internal("invalid configuration for the key `target`")
        })),
    };
    let triple = config.target().unwrap_or(config.rustc_host()).to_string();
    let target = match target.find(&triple) {
        None => return Ok(HashMap::new()),
        Some(target) => try!(target.table().chain_error(|| {
            internal(format!("invalid configuration for the key \
                              `target.{}`", triple))
        })),
    };

    let mut ret = HashMap::new();
    for (k, v) in target.iter() {
        match k.as_slice() {
            "ar" | "linker" => {
                let v = try!(v.string().chain_error(|| {
                    internal(format!("invalid configuration for key `{}`", k))
                })).ref0().to_string();
                if k.as_slice() == "linker" {
                    config.set_linker(v);
                } else {
                    config.set_ar(v);
                }
            }
            lib_name => {
                let table = try!(v.table().chain_error(|| {
                    internal(format!("invalid configuration for the key \
                                      `target.{}.{}`", triple, lib_name))
                }));
                let mut output = BuildOutput {
                    library_paths: Vec::new(),
                    library_links: Vec::new(),
                    metadata: Vec::new(),
                };
                for (k, v) in table.iter() {
                    let v = try!(v.string().chain_error(|| {
                        internal(format!("invalid configuration for the key \
                                          `target.{}.{}.{}`", triple, lib_name,
                                          k))
                    })).val0();
                    if k.as_slice() == "rustc-flags" {
                        let whence = format!("in `target.{}.{}.rustc-flags`",
                                             triple, lib_name);
                        let whence = whence.as_slice();
                        let (paths, links) = try!(
                            BuildOutput::parse_rustc_flags(v.as_slice(), whence)
                        );
                        output.library_paths.extend(paths.into_iter());
                        output.library_links.extend(links.into_iter());
                    } else {
                        output.metadata.push((k.to_string(), v.to_string()));
                    }
                }
                ret.insert(lib_name.to_string(), output);
            }
        }
    }

    Ok(ret)
}
