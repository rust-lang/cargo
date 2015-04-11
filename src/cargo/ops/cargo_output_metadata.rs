use std::borrow::Cow;
use std::collections::HashMap;
use std::io::Write;
use std::fs;
use std::path::{Path, PathBuf};
use rustc_serialize::{json, Decoder, Decodable};
use toml;
use util::CargoResult;
use util::config::Config;
use term::color::BLACK;
use core::{Source, Package, PackageId};
use core::manifest::{Target};
use core::registry::PackageRegistry;
use core::resolver::{Resolve, Method};
use sources::PathSource;
use ops;

#[derive(RustcDecodable)]
pub enum OutputFormat {
	Toml,
	Json,
}

/// Where the dependencies should be written to.
pub enum OutputTo {
	Path(PathBuf),
	StdOut,
}

impl Decodable for OutputTo {
	fn decode<D: Decoder>(d: &mut D) -> Result<OutputTo, D::Error> {
		d.read_option(|d, b| {
			if b {
				Ok(OutputTo::Path(try!(Decodable::decode(d))))
			} else {
				Ok(OutputTo::StdOut)
			}
		})
	}
}

pub struct OutputMetadataOptions<'a> {
	pub output_to: OutputTo,
	pub output_format: OutputFormat,
    pub manifest_path: &'a Path,
    pub features: Vec<String>,
}

/// Loads the manifest, resolves the dependencies of the project to the concrete
/// used versions - considering overrides - and writes all dependencies in a TOML
/// format to stdout or the specified file.
///
/// The TOML format is e.g.:
///
///     root = "libA"
///
///     [packages.libA]
///     dependencies = ["libB"]
///     path = "/home/user/.cargo/registry/src/github.com-1ecc6299db9ec823/libA-0.1"
///     version = "0.1"
///
///     [packages.libB]
///     dependencies = []
///     path = "/home/user/.cargo/registry/src/github.com-1ecc6299db9ec823/libB-0.4"
///     version = "0.4"
///
///     [packages.libB.features]
///     featureA = ["featureB"]
///     featureB = []
///
pub fn output_metadata(opt: OutputMetadataOptions, config: &Config) -> CargoResult<()> {
    let (resolved_deps, packages) =
        try!(resolve_dependencies(opt.manifest_path, opt.features, config));

	#[derive(RustcEncodable)]
    struct PackageInfo<'a> {
        version: String,
        path: Cow<'a, str>,
        dependencies: Vec<String>,
        features: Option<&'a HashMap<String, Vec<String>>>,
        targets: &'a[Target]
    };

	#[derive(RustcEncodable)]
	struct ExportInfo<'a> {
        root: String,
		packages: HashMap<String, PackageInfo<'a>>
	}

    let mut output = ExportInfo {
        root: resolved_deps.root().name().to_string(),
        packages: HashMap::new()
	};

    for package in packages.iter() {
        let mut package_info = PackageInfo {
            version: format!("{}", package.version()),
            path: (*package.root()).to_string_lossy(),
            dependencies: Vec::new(),
            features: {
                let features = package.manifest().summary().features();
                if features.is_empty() {
                    None
                } else {
                    Some(features)
                }
            },
            targets: package.manifest().targets()
        };

        if let Some(dep_deps) = resolved_deps.deps(package.package_id()) {
            for dep_dep in dep_deps {
                package_info.dependencies.push(dep_dep.name().to_string());
            }
        }

        output.packages.insert(package.name().to_string(), package_info);
    }

    let serialized_str = match opt.output_format {
		OutputFormat::Toml => toml::encode_str(&output),
		OutputFormat::Json => try!(json::encode(&output)),
	};

    match opt.output_to {
        OutputTo::StdOut         => try!(config.shell().say(serialized_str, BLACK)),
        OutputTo::Path(ref path) => {
            let mut file = try!(fs::File::create(path));
            try!(file.write_all(serialized_str.as_bytes()));
        }
    }

    Ok(())
}

/// Loads the manifest and resolves the dependencies of the project to the
/// concrete used versions. Afterwards available overrides of dependencies are applied.
fn resolve_dependencies(manifest: &Path, features: Vec<String>, config: &Config)
    -> CargoResult<(Resolve, Vec<Package>)> {
    let mut source = try!(PathSource::for_path(manifest.parent().unwrap(), config));
    try!(source.update());

    let package = try!(source.root_package());

    for key in package.manifest().warnings().iter() {
        try!(config.shell().warn(key))
    }

    let override_ids = try!(ops::source_ids_from_config(config, package.root()));
    let mut registry = PackageRegistry::new(config);

    // First, resolve the package's *listed* dependencies, as well as
    // downloading and updating all remotes and such.
    let resolved = try!(ops::resolve_pkg(&mut registry, &package));

    // Second, resolve with precisely what we're doing. Filter out
    // transitive dependencies if necessary, specify features, handle
    // overrides, etc.
    try!(registry.add_overrides(override_ids));

    let rustc_host = config.rustc_host().to_string();
    let default_feature = features.contains(&"default".to_string());
    let filtered_features =
        features.into_iter().filter(|s| s != "default").collect::<Vec<_>>();

    let platform = Some(rustc_host.as_ref());
    let method = Method::Required {
        dev_deps: false,
        features: &filtered_features,
        uses_default_features: default_feature,
        target_platform: platform
    };

    let resolved_specific =
        try!(ops::resolve_with_previous(&mut registry, &package, method, Some(&resolved), None));

    let package_ids: Vec<PackageId> = resolved_specific.iter().cloned().collect();
    let packages = try!(registry.get(&package_ids));
    for package in packages.iter() {
        debug!("{}: {:?}", package.package_id(), package.manifest().targets());
    }
    //debug!("{}", try!(json::encode(&try!(registry.get(&package_ids)))));

    Ok((resolved_specific, packages))
}
