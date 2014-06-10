use toml;
use core::Package;
use util::toml::toml_to_package;
use util::{CargoResult,human_error,toml_error};

pub fn read_manifest(path: &str) -> CargoResult<Package> {
    let root = try!(parse_from_file(path));
    toml_to_package(root, &Path::new(path))
}

fn parse_from_file(path: &str) -> CargoResult<toml::Value> {
    toml::parse_from_file(path.clone()).map_err(|err| {
        let err = toml_error("Couldn't parse Toml", err);
        human_error("Cargo.toml is not valid Toml".to_str(), format!("path={}", path), err)
    })
}
