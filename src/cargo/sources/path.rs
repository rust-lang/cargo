use core::source::{Source,PackagePath};
use core::NameVer;
use CargoResult;
use ops::cargo_read_manifest::read_manifest;

struct PathSource {
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
        self.map(|path| {
            let manifest = try!(read_manifest(path.as_str().unwrap()));
            Ok(manifest.get_name_ver())
        })
    }

    fn download(&self, name_ver: Vec<NameVer>)  -> CargoResult<()>{
        Ok(())
    }

    fn get(&self, packages: Vec<NameVer>) -> CargoResult<Vec<PackagePath>> {
        self.map(|path| {
            let manifest = try!(read_manifest(path.as_str().unwrap()));
            let name_ver = manifest.get_name_ver();
            let path = manifest.get_path();

            Ok(PackagePath::new(name_ver, path))
        })
    }
}
