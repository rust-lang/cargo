use toml;
use toml::from_toml;
use core::Package;
use core::manifest::{TomlManifest};
use util::{toml_error,human_error,CargoResult,CargoError};

pub fn read_manifest(path: &str) -> CargoResult<Package> {
    let root = try!(parse_from_file(path).map_err(|err: CargoError|
        human_error("Cargo.toml is not valid Toml".to_str(), format!("path={}", path), err)));

    let toml = try!(load_toml(root).map_err(|err: CargoError|
        human_error("Cargo.toml is not a valid Cargo manifest".to_str(), format!("path={}", path), err)));

    toml.to_package(path)
}

fn parse_from_file(path: &str) -> CargoResult<toml::Value> {
    toml::parse_from_file(path.clone()).map_err(to_cargo_err)
}

fn load_toml(root: toml::Value) -> CargoResult<TomlManifest> {
    from_toml::<TomlManifest>(root).map_err(to_cargo_err)
}

fn to_cargo_err(err: toml::Error) -> CargoError {
    debug!("toml; err={}", err);
    toml_error("Problem loading manifest", err)
}
