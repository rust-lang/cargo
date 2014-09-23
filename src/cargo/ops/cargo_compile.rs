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
use core::{MultiShell, Source, SourceId, PackageSet, Target, PackageId};
use core::resolver;
use ops;
use sources::{PathSource};
use util::config::{Config, ConfigValue};
use util::{CargoResult, Wrap, config, internal, human, ChainError, profile};

pub struct CompileOptions<'a> {
    pub env: &'a str,
    pub shell: &'a mut MultiShell<'a>,
    pub jobs: Option<uint>,
    pub target: Option<&'a str>,
    pub dev_deps: bool,
    pub features: &'a [String],
    pub no_default_features: bool,
}

pub fn compile(manifest_path: &Path,
               options: &mut CompileOptions)
               -> CargoResult<ops::Compilation> {
    let CompileOptions { env, ref mut shell, jobs, target,
                         dev_deps, features, no_default_features } = *options;
    let target = target.map(|s| s.to_string());
    let features = features.iter().flat_map(|s| {
        s.as_slice().split(' ')
    }).map(|s| s.to_string()).collect::<Vec<String>>();

    log!(4, "compile; manifest-path={}", manifest_path.display());

    let mut source = try!(PathSource::for_path(&manifest_path.dir_path()));
    try!(source.update());

    // TODO: Move this into PathSource
    let package = try!(source.get_root_package());
    debug!("loaded package; package={}", package);

    for key in package.get_manifest().get_warnings().iter() {
        try!(shell.warn(key))
    }

    let user_configs = try!(config::all_configs(os::getcwd()));
    let override_ids = try!(source_ids_from_config(&user_configs,
                                                   manifest_path.dir_path()));

    let (packages, resolve_with_overrides, sources) = {
        let mut config = try!(Config::new(*shell, jobs, target.clone()));
        let mut registry = PackageRegistry::new(&mut config);

        // First, resolve the package's *listed* dependencies, as well as
        // downloading and updating all remotes and such.
        try!(ops::resolve_and_fetch(&mut registry, &package));

        // Second, resolve with precisely what we're doing. Filter out
        // transitive dependencies if necessary, specify features, handle
        // overrides, etc.
        let _p = profile::start("resolving w/ overrides...");

        try!(registry.add_overrides(override_ids));
        let method = resolver::ResolveRequired(dev_deps, features.as_slice(),
                                               !no_default_features);
        let resolved_with_overrides =
                try!(resolver::resolve(package.get_summary(), method,
                                       &mut registry));

        let req: Vec<PackageId> = resolved_with_overrides.iter().map(|r| {
            r.clone()
        }).collect();
        let packages = try!(registry.get(req.as_slice()).wrap({
            human("Unable to get packages from source")
        }));

        (packages, resolved_with_overrides, registry.move_sources())
    };

    debug!("packages={}", packages);

    let targets = package.get_targets().iter().filter(|target| {
        match env {
            // doc-all == document everything, so look for doc targets
            "doc" | "doc-all" => target.get_profile().get_env() == "doc",
            env => target.get_profile().get_env() == env,
        }
    }).collect::<Vec<&Target>>();

    let ret = {
        let _p = profile::start("compiling");
        let mut config = try!(Config::new(*shell, jobs, target));
        try!(scrape_target_config(&mut config, &user_configs));

        try!(ops::compile_targets(env.as_slice(), targets.as_slice(), &package,
                                  &PackageSet::new(packages.as_slice()),
                                  &resolve_with_overrides, &sources,
                                  &mut config))
    };

    return Ok(ret);
}

fn source_ids_from_config(configs: &HashMap<String, config::ConfigValue>,
                          cur_path: Path) -> CargoResult<Vec<SourceId>> {
    debug!("loaded config; configs={}", configs);

    let config_paths = match configs.find_equiv(&"paths") {
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

fn scrape_target_config(config: &mut Config,
                        configs: &HashMap<String, config::ConfigValue>)
                        -> CargoResult<()> {
    let target = match configs.find_equiv(&"target") {
        None => return Ok(()),
        Some(target) => try!(target.table().chain_error(|| {
            internal("invalid configuration for the key `target`")
        })),
    };
    let target = match config.target() {
        None => target,
        Some(triple) => match target.find_equiv(&triple) {
            None => return Ok(()),
            Some(target) => try!(target.table().chain_error(|| {
                internal(format!("invalid configuration for the key \
                                  `target.{}`", triple))
            })),
        },
    };

    match target.find_equiv(&"ar") {
        None => {}
        Some(ar) => {
            config.set_ar(try!(ar.string().chain_error(|| {
                internal("invalid configuration for key `ar`")
            })).ref0().to_string());
        }
    }

    match target.find_equiv(&"linker") {
        None => {}
        Some(linker) => {
            config.set_linker(try!(linker.string().chain_error(|| {
                internal("invalid configuration for key `ar`")
            })).ref0().to_string());
        }
    }

    Ok(())
}
