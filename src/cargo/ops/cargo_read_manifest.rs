use toml;
use hammer::FlagConfig;
use serialize::Decoder;
use toml::from_toml;
use {CargoResult,ToCargoError,core};
use core::manifest::{SerializedManifest,Manifest};


#[deriving(Decodable,Eq,Clone,Ord)]
pub struct ReadManifestFlags {
    manifest_path: ~str
}

impl FlagConfig for ReadManifestFlags {}

pub fn read_manifest(manifest_path: &str) -> CargoResult<core::Manifest> {
    match execute(ReadManifestFlags { manifest_path: manifest_path.to_owned() }) {
        Ok(manifest) => Ok(manifest.unwrap()),
        Err(e) => Err(e)
    }
}

pub fn execute(flags: ReadManifestFlags) -> CargoResult<Option<core::Manifest>> {
    let manifest_path = flags.manifest_path;
    let root = try!(toml::parse_from_file(manifest_path.clone()).to_cargo_error(format!("Couldn't parse Toml file: {}", manifest_path), 1));

    let toml_manifest = try!(from_toml::<SerializedManifest>(root.clone()).to_cargo_error(|e: toml::Error| format!("Couldn't parse Toml file: {:?}", e), 1));

    Manifest::from_serialized(manifest_path.as_slice(), &toml_manifest).map(|manifest| {
        Some(manifest)
    })
}
