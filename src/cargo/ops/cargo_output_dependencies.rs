use std::io;
use std::collections::HashMap;
use std::collections::hash_map::Entry::{Occupied, Vacant};
use util::{CargoResult, profile};
use util::config::Config;
use core::{Source, SourceId, PackageId};
use core::registry::PackageRegistry;
use core::resolver::{Resolve, Method};
use sources::PathSource;
use ops;

#[derive(Eq, PartialEq)]
/// Where the dependencies should be written to.
pub enum OutputTo {
    Path(Path),
    StdOut
}

pub struct OutputOptions<'a, 'b: 'a> {
    pub out: OutputTo,
    pub config: &'a Config<'b>
}

/// Loads the manifest, resolves the dependencies of the project to the concrete
/// used versions - considering overrides - and writes all dependencies in a TOML
/// format to stdout or the specified file.
///
/// The TOML format is e.g.:
///
///     [dependencies.libA]
///     version = "0.1",
///     path = '/home/user/.cargo/registry/src/github.com-1ecc6299db9ec823/libA-0.1'
///     dependencies = ["libB"]
///
///     [dependencies.libB]
///     version = "0.4",
///     path = '/home/user/.cargo/registry/src/github.com-1ecc6299db9ec823/libB-0.4'
///     dependencies = []
///
pub fn output_dependencies(manifest: &Path, options: &OutputOptions) -> CargoResult<()> {
    let OutputOptions { ref out, config } = *options;
    let resolved_deps = try!(resolve_dependencies(manifest, config));
    let src_paths = try!(source_paths(&resolved_deps, config));

    struct Dependency {
        name: String,
        version: String,
        path: Path,
        dependencies: Vec<String>
    };

    let mut dependencies: Vec<Dependency> = Vec::new();

    for (package_id, src_path) in src_paths.iter() {
        if *package_id == *resolved_deps.root() {
            continue;
        }

        let mut dependency = Dependency {
            name: package_id.get_name().to_string(),
            version: format!("{}", package_id.get_version()),
            path: src_path.clone(),
            dependencies: Vec::new()
        };

        if let Some(mut dep_deps) = resolved_deps.deps(package_id) {
            for dep_dep in dep_deps {
                dependency.dependencies.push(dep_dep.get_name().to_string());
            }
        }

        dependencies.push(dependency);
    }

    let mut toml_str = String::new();

    for dep in dependencies.iter() {
        toml_str.push_str(format!("\n[dependencies.{}]\n", dep.name).as_slice());
        toml_str.push_str(format!("version = \"{}\"\n", dep.version).as_slice());
        toml_str.push_str(format!("path = '{:?}'\n", dep.path).as_slice());
        toml_str.push_str(format!("dependencies = {:?}\n", dep.dependencies).as_slice());
    }

    match *out {
        OutputTo::StdOut         => println!("{}", toml_str),
        OutputTo::Path(ref path) => {
            let mut file = try!(io::File::open_mode(path, io::Truncate, io::ReadWrite));
            try!(file.write_str(toml_str.as_slice()));
        }
    }

    Ok(())
}

/// Returns the source path for each `PackageId` in `Resolve`.
fn source_paths(resolve: &Resolve, config: &Config) -> CargoResult<HashMap<PackageId, Path>> {
    let package_ids: Vec<&PackageId> = resolve.iter().collect();
    let mut source_id_to_package_ids: HashMap<&SourceId, Vec<PackageId>> = HashMap::new();

    // group PackageId by SourceId
    for package_id in package_ids.iter() {
        match source_id_to_package_ids.entry(package_id.get_source_id()) {
            Occupied(mut entry) => entry.get_mut().push((*package_id).clone()),
            Vacant(entry)       => { entry.insert(vec![(*package_id).clone()]); }
        }
    }

    let mut package_id_to_path: HashMap<PackageId, Path> = HashMap::new();

    for (mut source_id, ref package_ids) in source_id_to_package_ids.iter() {
        let mut source = source_id.load(config);
        try!(source.update());
        try!(source.download(package_ids.as_slice()));
        let packages = try!(source.get(package_ids.as_slice()));

        for package in packages.iter() {
            match package_id_to_path.entry(package.get_package_id().clone()) {
                Occupied(_)   => {},
                Vacant(entry) => { entry.insert(package.get_root()); }
            }
        }
    }

    Ok(package_id_to_path)
}

/// Loads the manifest and resolves the dependencies of the project to the
/// concrete used versions. Afterwards available overrides of dependencies are applied.
fn resolve_dependencies(manifest: &Path, config: &Config) -> CargoResult<Resolve> {
    let mut source = try!(PathSource::for_path(&manifest.dir_path(), config));
    try!(source.update());

    let package = try!(source.get_root_package());
    debug!("loaded package; package={}", package);

    for key in package.get_manifest().get_warnings().iter() {
        try!(config.shell().warn(key))
    }

    let override_ids = try!(ops::source_ids_from_config(config, package.get_root()));
    let rustc_host = config.rustc_host().to_string();
    let mut registry = PackageRegistry::new(config);

    // First, resolve the package's *listed* dependencies, as well as
    // downloading and updating all remotes and such.
    let resolved = try!(ops::resolve_pkg(&mut registry, &package));

    // Second, resolve with precisely what we're doing. Filter out
    // transitive dependencies if necessary, specify features, handle
    // overrides, etc.
    let _p = profile::start("resolving w/ overrides...");

    try!(registry.add_overrides(override_ids));

    let platform = Some(rustc_host.as_slice());
    let method = Method::Required(false, &[], true, platform);

    ops::resolve_with_previous(&mut registry, &package, method, Some(&resolved), None)
}
