use core::{NameVer,Package};
use core::source::Source;
use core::manifest::Manifest;
use CargoResult;
use cargo_read_manifest = ops::cargo_read_manifest::read_manifest;

pub struct PathSource {
    paths: Vec<Path>
}

impl PathSource {
    pub fn new(paths: Vec<Path>) -> PathSource {
        PathSource { paths: paths }
    }

    fn map<T>(&self, callback: |&Path| -> CargoResult<T>) -> CargoResult<Vec<T>> {
        let mut ret = Vec::with_capacity(self.paths.len());

        for path in self.paths.iter() {
            ret.push(try!(callback(path)));
        }

        Ok(ret)
    }
}

impl Source for PathSource {
    fn update(&self) -> CargoResult<()> { Ok(()) }

    fn list(&self) -> CargoResult<Vec<NameVer>> {
        Ok(self.paths.iter().filter_map(|path| {
            match read_manifest(path) {
                Ok(ref manifest) => Some(manifest.get_name_ver()),
                Err(_) => None
            }
        }).collect())
    }

    fn download(&self, name_ver: &[NameVer])  -> CargoResult<()>{
        Ok(())
    }

    fn get(&self, packages: &[NameVer]) -> CargoResult<Vec<Package>> {
        Ok(self.paths.iter().filter_map(|path| {
            match read_manifest(path) {
                Ok(ref manifest) => Some(Package::from_manifest(manifest)),
                Err(_) => None
            }
        }).collect())
    }
}

fn read_manifest(path: &Path) -> CargoResult<Manifest> {
    let joined = path.join("Cargo.toml");
    cargo_read_manifest(joined.as_str().unwrap())
}
