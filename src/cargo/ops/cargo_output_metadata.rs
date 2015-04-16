use std::collections::HashMap;
use std::io::Write;
use std::fs;
use std::path::{Path, PathBuf};
use rustc_serialize::{json, Decoder, Decodable};
use toml;
use util::CargoResult;
use util::config::Config;
use term::color::BLACK;
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
                Ok(OutputTo::Path(try!(Decodable::decode(d))))
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
    struct ExportInfo<'a> {
        root: String,
        root_features: Option<&'a HashMap<String, Vec<String>>>,
        packages: HashMap<String, &'a Package>
    }

    let mut output = ExportInfo {
        root: resolved_deps.root().name().to_string(),
        root_features: None,
        packages: HashMap::new()
    };

    for package in packages.iter() {
        output.packages.insert(package.name().to_string(), &package);
        if package.package_id() == resolved_deps.root() {
            let features = package.manifest().summary().features();
            if !features.is_empty() {
                output.root_features = Some(features);
            }
        }
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
