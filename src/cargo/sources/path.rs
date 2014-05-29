use std::fmt;
use std::fmt::{Show,Formatter};
use core::{NameVer,Package,Summary};
use core::source::Source;
use ops;
use util::{CargoResult};

pub struct PathSource {
    paths: Vec<Path>
}

impl PathSource {
    pub fn new(paths: Vec<Path>) -> PathSource {
        log!(4, "new; paths={}", display(paths.as_slice()));
        PathSource { paths: paths }
    }
}

impl Show for PathSource {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "the paths source")
    }
}

impl Source for PathSource {
    fn update(&self) -> CargoResult<()> { Ok(()) }

    fn list(&self) -> CargoResult<Vec<Summary>> {
        Ok(self.paths.iter().filter_map(|path| {
            match read_manifest(path) {
                Ok(ref pkg) => Some(pkg.get_summary().clone()),
                Err(e) => {
                    log!(4, "failed to read manifest; path={}; err={}", path.display(), e);
                    None
                }
            }
        }).collect())
    }

    fn download(&self, _: &[NameVer])  -> CargoResult<()>{
        Ok(())
    }

    fn get(&self, name_vers: &[NameVer]) -> CargoResult<Vec<Package>> {
        Ok(self.paths.iter().filter_map(|path| {
            match read_manifest(path) {
                Ok(pkg) => {
                    if name_vers.iter().any(|nv| pkg.is_for_name_ver(nv)) {
                        Some(pkg)
                    } else {
                        None
                    }
                }
                Err(_) => None
            }
        }).collect())
    }
}

fn read_manifest(path: &Path) -> CargoResult<Package> {
    let joined = path.join("Cargo.toml");
    ops::read_manifest(joined.as_str().unwrap())
}

fn display(paths: &[Path]) -> Vec<String> {
    paths.iter().map(|p| p.display().to_str()).collect()
}
