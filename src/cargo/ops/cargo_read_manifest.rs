use std::io::File;
use util;
use core::{Package,Manifest,SourceId};
use util::{CargoResult, human, important_paths};

pub fn read_manifest(contents: &[u8], source_id: &SourceId)
    -> CargoResult<(Manifest, Vec<Path>)>
{
    util::toml::to_manifest(contents, source_id).map_err(human)
}

pub fn read_package(path: &Path, source_id: &SourceId)
    -> CargoResult<(Package, Vec<Path>)>
{
    log!(5, "read_package; path={}; source-id={}", path.display(), source_id);
    let mut file = try!(File::open(path));
    let data = try!(file.read_to_end());
    let (manifest, nested) = try!(read_manifest(data.as_slice(),
                                                      source_id));

    Ok((Package::new(manifest, path, source_id), nested))
}

pub fn read_packages(path: &Path, source_id: &SourceId)
    -> CargoResult<Vec<Package>>
{
    let manifest = try!(important_paths::find_project_manifest_exact(path, "Cargo.toml"));
    let (pkg, nested) = try!(read_package(&manifest, source_id));
    let mut ret = vec!(pkg);

    for p in nested.iter() {
        ret.push_all(try!(read_packages(&path.join(p), source_id)).as_slice());
    }

    Ok(ret)
}
