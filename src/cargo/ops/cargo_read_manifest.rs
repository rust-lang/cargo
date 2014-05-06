use toml;
use toml::from_toml;
use core;
use core::manifest::{SerializedManifest};
use core::errors::{CLIError,CLIResult,ToResult};

pub fn read_manifest(manifest_path: &str) -> CLIResult<core::Package> {
    let root = try!(toml::parse_from_file(manifest_path.clone()).to_result(|err|
        CLIError::new(format!("Cargo.toml was not valid Toml: {}", manifest_path), Some(err.to_str()), 1)));

    let toml_manifest = try!(from_toml::<SerializedManifest>(root.clone()).to_result(|err: toml::Error|
        CLIError::new(format!("Cargo.toml was not in the right format: {}", manifest_path), Some(err.to_str()), 1)));

    toml_manifest.to_package(manifest_path.as_slice()).to_result(|err|
        CLIError::new(format!("Cargo.toml was not in the right format: {}", manifest_path), Some(err.to_str()), 1))
}
