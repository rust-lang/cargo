use std::io::File;
use util;
use url::Url;
use core::{Package,Manifest};
use util::{CargoResult,io_error};

pub fn read_manifest(contents: &[u8], namespace: &Url) -> CargoResult<Manifest> {
    util::toml::to_manifest(contents, namespace)
}

pub fn read_package(path: &Path, namespace: &Url) -> CargoResult<Package> {
    log!(5, "read_package; path={}; namespace={}", path.display(), namespace);
    let mut file = try!(File::open(path).map_err(io_error));
    let data = try!(file.read_to_end().map_err(io_error));
    let manifest = try!(read_manifest(data.as_slice(), namespace));

    Ok(Package::new(&manifest, path))
}
