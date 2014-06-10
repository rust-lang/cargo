use std::io::File;
use util;
use core::{Package,Manifest};
use util::{CargoResult,io_error};

pub fn read_manifest(contents: &[u8]) -> CargoResult<Manifest> {
    util::toml::to_manifest(contents)
}

pub fn read_package(path: &Path) -> CargoResult<Package> {
    let mut file = try!(File::open(path).map_err(io_error));
    let data = try!(file.read_to_end().map_err(io_error));
    let manifest = try!(read_manifest(data.as_slice()));

    Ok(Package::new(&manifest, &path.dir_path()))
}
