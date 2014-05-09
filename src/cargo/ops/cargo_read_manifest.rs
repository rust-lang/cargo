use toml;
use toml::from_toml;
use core::Package;
use core::manifest::{TomlManifest};
use util::{other_error,CargoResult,CargoError};

pub fn read_manifest(path: &str) -> CargoResult<Package> {
    let root = try!(parse_from_file(path));
    let toml = try!(load_toml(path, root));
    toml.to_package(path)
}

fn parse_from_file(path: &str) -> CargoResult<toml::Value> {
    toml::parse_from_file(path.clone()).map_err(|err| to_cargo_err(path, err))
}

fn load_toml(path: &str, root: toml::Value) -> CargoResult<TomlManifest> {
    from_toml::<TomlManifest>(root).map_err(|err| to_cargo_err(path, err))
}

fn to_cargo_err(path: &str, err: toml::Error) -> CargoError {
    other_error("Cargo.toml is not valid Toml")
        .with_detail(format!("path={}; err={}", path, err.to_str()))
}
