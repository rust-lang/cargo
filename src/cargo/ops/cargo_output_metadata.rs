use std::collections::HashMap;
use std::io::Write;
use std::fs;
use std::path::{Path, PathBuf};
use rustc_serialize::{json, Decoder, Decodable};
use toml;
use util::CargoResult;
use util::config::Config;
use core::{Source, Package};
use core::resolver::Resolve;
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
                let path = PathBuf::from(try!(d.read_str()));
                Ok(OutputTo::Path(path))
            } else {
                Ok(OutputTo::StdOut)
            }
        })
    }
}

pub struct OutputMetadataOptions<'a> {
    pub features: Vec<String>,
    pub output_format: OutputFormat,
    pub output_to: OutputTo,
    pub manifest_path: &'a Path,
    pub no_default_features: bool,
    pub target: Option<String>,
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
    let (resolved_deps, packages) =
        try!(resolve_dependencies(
            opt.manifest_path, config, opt.features, opt.target, opt.no_default_features));

    #[derive(RustcEncodable)]
    struct RootPackageInfo<'a> {
        name: &'a str,
        version: String,
        features: Option<&'a HashMap<String, Vec<String>>>,
    }

    #[derive(RustcEncodable)]
    struct ExportInfo<'a> {
        root: RootPackageInfo<'a>,
        packages: Vec<&'a Package>
    }

    let mut output = ExportInfo {
        root: RootPackageInfo {
            name: resolved_deps.root().name(),
            version: format!("{}", resolved_deps.root().version()),
            features: None,
        },
        packages: Vec::new()
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

    let serialized_str = match opt.output_format {
        OutputFormat::Toml => toml::encode_str(&output),
        OutputFormat::Json => try!(json::encode(&output)),
    };

    match opt.output_to {
        OutputTo::StdOut => println!("{}", serialized_str),
        OutputTo::Path(ref path) => {
            let mut file = try!(fs::File::create(path));
            try!(file.write_all(serialized_str.as_bytes()));
        }
    }

    Ok(())
}

/// Loads the manifest and resolves the dependencies of the project to the
/// concrete used versions. Afterwards available overrides of dependencies are applied.
fn resolve_dependencies(manifest: &Path, config: &Config, features: Vec<String>,
                        target: Option<String>, no_default_features: bool)
    -> CargoResult<(Resolve, Vec<Package>)> {
    let mut source = try!(PathSource::for_path(manifest.parent().unwrap(), config));
    try!(source.update());

    let package = try!(source.root_package());

    let (packages, resolve_with_overrides, _) =
            try!(ops::cargo_compile::resolve_dependencies(
                &package, config, &target, features, no_default_features));

    Ok((resolve_with_overrides, packages))
}
