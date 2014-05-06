use std::fmt;
use std::fmt::{Show,Formatter};
use core::{NameVer,Package,Summary};
use core::source::Source;
use core::errors::{CargoResult,CargoCLIError,ToResult};
use cargo_read_manifest = ops::cargo_read_manifest::read_manifest;

pub struct PathSource {
    paths: Vec<Path>
}

impl PathSource {
    pub fn new(paths: Vec<Path>) -> PathSource {
        PathSource { paths: paths }
    }
}

impl Show for PathSource {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f.buf, "the paths source")
    }
}

impl Source for PathSource {
    fn update(&self) -> CargoResult<()> { Ok(()) }

    fn list(&self) -> CargoResult<Vec<Summary>> {
        Ok(self.paths.iter().filter_map(|path| {
            match read_manifest(path) {
                Ok(ref pkg) => Some(pkg.get_summary().clone()),
                Err(_) => None
            }
        }).collect())
    }

    fn download(&self, _: &[NameVer])  -> CargoResult<()>{
        Ok(())
    }

    fn get(&self, _: &[NameVer]) -> CargoResult<Vec<Package>> {
        Ok(self.paths.iter().filter_map(|path| {
            match read_manifest(path) {
                Ok(pkg) => Some(pkg),
                Err(_) => None
            }
        }).collect())
    }
}

fn read_manifest(path: &Path) -> CargoResult<Package> {
    let joined = path.join("Cargo.toml");
    cargo_read_manifest(joined.as_str().unwrap()).to_result(|err| CargoCLIError(err))
}
