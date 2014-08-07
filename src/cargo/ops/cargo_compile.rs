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
use std::collections::{HashMap, HashSet};
use std::result;

use core::registry::PackageRegistry;
use core::{MultiShell, Source, SourceId, PackageSet, Target, PackageId};
use core::{Package, Summary, Resolve};
use core::resolver;
use ops;
use sources::{PathSource};
use util::config::{Config, ConfigValue};
use util::{CargoResult, Wrap, config, internal, human, ChainError};
use util::profile;

pub struct CompileOptions<'a> {
    pub update: bool,
    pub env: &'a str,
    pub shell: &'a mut MultiShell,
    pub jobs: Option<uint>,
    pub target: Option<&'a str>,
}

pub fn compile(manifest_path: &Path,
               options: &mut CompileOptions)
               -> CargoResult<ops::Compilation> {
    let CompileOptions { update, env, ref mut shell, jobs, target } = *options;
    let target = target.map(|s| s.to_string());

    log!(4, "compile; manifest-path={}", manifest_path.display());

    if update {
        return Err(human("The -u flag has been deprecated, please use the \
                          `cargo update` command instead"));
    }

    let mut source = try!(PathSource::for_path(&manifest_path.dir_path()));
    try!(source.update());

    // TODO: Move this into PathSource
    let package = try!(source.get_root_package());
    debug!("loaded package; package={}", package);

    for key in package.get_manifest().get_unused_keys().iter() {
        try!(shell.warn(format!("unused manifest key: {}", key)));
    }

    let user_configs = try!(config::all_configs(os::getcwd()));
    let override_ids = try!(source_ids_from_config(&user_configs,
                                                   manifest_path.dir_path()));

    let (packages, resolve, resolve_with_overrides, sources) = {
        let _p = profile::start("resolving...");
        let lockfile = manifest_path.dir_path().join("Cargo.lock");
        let source_id = package.get_package_id().get_source_id();

        let mut config = try!(Config::new(*shell, jobs, target.clone()));
        let mut registry = PackageRegistry::new(&mut config);

        match try!(ops::load_lockfile(&lockfile, source_id)) {
            Some(r) => try!(add_lockfile_sources(&mut registry, &package, &r)),
            None => try!(registry.add_sources(package.get_source_ids())),
        }

        let resolved = try!(resolver::resolve(package.get_package_id(),
                                              package.get_dependencies(),
                                              &mut registry));

        try!(registry.add_overrides(override_ids));
        let resolved_with_overrides =
                try!(resolver::resolve(package.get_package_id(),
                                       package.get_dependencies(),
                                       &mut registry));

        let req: Vec<PackageId> = resolved_with_overrides.iter().map(|r| {
            r.clone()
        }).collect();
        let packages = try!(registry.get(req.as_slice()).wrap({
            human("Unable to get packages from source")
        }));

        (packages, resolved, resolved_with_overrides, registry.move_sources())
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

    try!(ops::write_resolve(&package, &resolve));

    return Ok(ret);
}

fn source_ids_from_config(configs: &HashMap<String, config::ConfigValue>,
                          cur_path: Path) -> CargoResult<Vec<SourceId>> {
    debug!("loaded config; configs={}", configs);

    let config_paths = configs.find_equiv(&"paths").map(|v| v.clone());
    let config_paths = config_paths.unwrap_or_else(|| ConfigValue::new());

    let paths = try!(config_paths.list().chain_error(|| {
        internal("invalid configuration for the key `path`")
    }));

    // Make sure we don't override the local package, even if it's in the list
    // of override paths
    result::collect(paths.iter().filter(|p| {
        cur_path != os::make_absolute(&Path::new(p.as_slice()))
    }).map(|p| SourceId::for_path(&Path::new(p.as_slice()))))
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
            })).to_string());
        }
    }

    match target.find_equiv(&"linker") {
        None => {}
        Some(linker) => {
            config.set_linker(try!(linker.string().chain_error(|| {
                internal("invalid configuration for key `ar`")
            })).to_string());
        }
    }

    Ok(())
}

/// When a lockfile is present, we want to keep as many dependencies at their
/// original revision as possible. We need to account, however, for
/// modifications to the manifest in terms of modifying, adding, or deleting
/// dependencies.
///
/// This method will add any appropriate sources from the lockfile into the
/// registry, and add all other sources from the root package to the registry.
/// Any dependency which has not been modified has its source added to the
/// registry (to retain the precise field if possible). Any dependency which
/// *has* changed has its source id listed in the manifest added and all of its
/// transitive dependencies are blacklisted to not be added from the lockfile.
///
/// TODO: this won't work too well for registry-based packages, but we don't
///       have many of those anyway so we should be ok for now.
fn add_lockfile_sources(registry: &mut PackageRegistry,
                        root: &Package,
                        resolve: &Resolve) -> CargoResult<()> {
    let deps = resolve.deps(root.get_package_id()).move_iter().flat_map(|deps| {
        deps.map(|d| (d.get_name(), d))
    }).collect::<HashMap<_, _>>();

    let mut sources = vec![root.get_package_id().get_source_id().clone()];
    let mut to_avoid = HashSet::new();
    let mut to_add = HashSet::new();
    for dep in root.get_dependencies().iter() {
        match deps.find(&dep.get_name()) {
            Some(&lockfile_dep) => {
                let summary = Summary::new(lockfile_dep, []);
                if dep.matches(&summary) {
                    fill_with_deps(resolve, lockfile_dep, &mut to_add);
                } else {
                    fill_with_deps(resolve, lockfile_dep, &mut to_avoid);
                    sources.push(dep.get_source_id().clone());
                }
            }
            None => sources.push(dep.get_source_id().clone()),
        }
    }

    // Only afterward once we know the entire blacklist are the lockfile
    // sources added.
    for addition in to_add.iter() {
        if !to_avoid.contains(addition) {
            sources.push(addition.get_source_id().clone());
        }
    }

    return registry.add_sources(sources);

    fn fill_with_deps<'a>(resolve: &'a Resolve, dep: &'a PackageId,
                          set: &mut HashSet<&'a PackageId>) {
        if !set.insert(dep) { return }
        for mut deps in resolve.deps(dep).move_iter() {
            for dep in deps {
                fill_with_deps(resolve, dep, set);
            }
        }
    }
}
