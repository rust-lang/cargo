use std::ascii::AsciiExt;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use core::resolver::Resolve;
use core::{Source, Package};
use ops;
use rustc_serialize::json;
use sources::PathSource;
use toml;
use util::config::Config;
use util::{paths, CargoResult};


/// Where the dependencies should be written to.
pub enum OutputTo {
    File(PathBuf),
    StdOut,
}

pub struct OutputMetadataOptions<'a> {
    pub features: Vec<String>,
    pub output_format: String,
    pub output_to: OutputTo,
    pub manifest_path: &'a Path,
    pub no_default_features: bool,
}

/// Loads the manifest, resolves the dependencies of the project to the concrete
/// used versions - considering overrides - and writes all dependencies in a TOML
/// format to stdout or the specified file.
///
/// The TOML format is e.g.:
/// ```toml
/// root = "libA"
///
/// [packages.libA]
/// dependencies = ["libB"]
/// path = "/home/user/.cargo/registry/src/github.com-1ecc6299db9ec823/libA-0.1"
/// version = "0.1"
///
/// [packages.libB]
/// dependencies = []
/// path = "/home/user/.cargo/registry/src/github.com-1ecc6299db9ec823/libB-0.4"
/// version = "0.4"
///
/// [packages.libB.features]
/// featureA = ["featureB"]
/// featureB = []
/// ```
pub fn output_metadata(opt: OutputMetadataOptions, config: &Config) -> CargoResult<()> {
    let deps = try!(resolve_dependencies(opt.manifest_path,
                                         config,
                                         opt.features,
                                         opt.no_default_features));
    let (resolved_deps, packages) = deps;

    #[derive(RustcEncodable)]
    struct RootPackageInfo<'a> {
        name: &'a str,
        version: String,
        features: Option<&'a HashMap<String, Vec<String>>>,
    }

    #[derive(RustcEncodable)]
    struct ExportInfo<'a> {
        root: RootPackageInfo<'a>,
        packages: Vec<&'a Package>,
    }

    let mut output = ExportInfo {
        root: RootPackageInfo {
            name: resolved_deps.root().name(),
            version: format!("{}", resolved_deps.root().version()),
            features: None,
        },
        packages: Vec::new(),
    };

    for package in packages.iter() {
        output.packages.push(&package);
        if package.package_id() == resolved_deps.root() {
            let features = package.manifest().summary().features();
            if !features.is_empty() {
                output.root.features = Some(features);
            }
        }
    }

    let serialized_str = match &opt.output_format.to_ascii_uppercase()[..] {
        "TOML" => toml::encode_str(&output),
        "JSON" => try!(json::encode(&output)),
        _ => bail!("unknown format: {}, supported formats are TOML, JSON.",
                   opt.output_format),
    };

    match opt.output_to {
        OutputTo::StdOut => println!("{}", serialized_str),
        OutputTo::File(ref path) => try!(paths::write(path, serialized_str.as_bytes()))
    }

    Ok(())
}

/// Loads the manifest and resolves the dependencies of the project to the
/// concrete used versions. Afterwards available overrides of dependencies are applied.
fn resolve_dependencies(manifest: &Path,
                        config: &Config,
                        features: Vec<String>,
                        no_default_features: bool)
                        -> CargoResult<(Resolve, Vec<Package>)> {
    let mut source = try!(PathSource::for_path(manifest.parent().unwrap(), config));
    try!(source.update());

    let package = try!(source.root_package());

    let deps = try!(ops::resolve_dependencies(&package,
                                              config,
                                              Some(Box::new(source)),
                                              features,
                                              no_default_features));

    let (packages, resolve_with_overrides, _) = deps;

    Ok((resolve_with_overrides, packages))
}
