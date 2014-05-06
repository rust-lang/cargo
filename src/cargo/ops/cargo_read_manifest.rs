use toml;
use toml::from_toml;
use core::manifest::{SerializedManifest,Manifest};
use core::errors::{CLIError,CLIResult,ToResult};

pub fn read_manifest(manifest_path: &str) -> CLIResult<Manifest> {
    let root = try!(toml::parse_from_file(manifest_path.clone()).to_result(|err|
        CLIError::new(format!("Cargo.toml was not valid Toml: {}", manifest_path), Some(err.to_str()), 1)));

    let toml_manifest = try!(from_toml::<SerializedManifest>(root.clone()).to_result(|err: toml::Error|
        CLIError::new(format!("Cargo.toml was not in the right format: {}", manifest_path), Some(err.to_str()), 1)));

    Manifest::from_serialized(manifest_path.as_slice(), &toml_manifest).to_result(|err|
        CLIError::new(format!("Cargo.toml was not in the right format: {}", manifest_path), Some(err.to_str()), 1))
}
