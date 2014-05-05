use toml;
use toml::from_toml;
use core::manifest::{SerializedManifest,Manifest};
use {CargoResult,ToCargoError};

pub fn read_manifest(manifest_path: &str) -> CargoResult<Manifest> {
    let root = try!(toml::parse_from_file(manifest_path.clone()).to_cargo_error(format!("Couldn't parse Toml file: {}", manifest_path), 1));
    let toml_manifest = try!(from_toml::<SerializedManifest>(root.clone()).to_cargo_error(|e: toml::Error| format!("Couldn't parse Toml file: {:?}", e), 1));

    Manifest::from_serialized(manifest_path.as_slice(), &toml_manifest)
}
