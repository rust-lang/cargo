use std::fmt;
use std::fmt::{Show,Formatter};
use core::{Package,PackageId,Summary};
use core::source::Source;
use ops;
use url;
use util::{CargoResult,simple_human,io_error,realpath};

/* 
 * TODO: Consider whether it may be more appropriate for a PathSource to only
 * take in a single path vs. a vec of paths. The pros / cons are unknown at
 * this point.
 */
pub struct PathSource { paths: Vec<Path> }

impl PathSource {
    pub fn new(paths: Vec<Path>) -> PathSource {
        log!(5, "new; paths={}", display(paths.as_slice()));
        PathSource { paths: paths }
    }

    pub fn read_package(path: &Path) -> CargoResult<Package> {
        log!(5, "read_package; path={}", path.display());

        // TODO: Use a realpath fn
        let dir = path.dir_path();
        let namespace = try!(namespace(&dir));

        ops::read_package(path, &namespace)
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
            match PathSource::read_package(&path.join("Cargo.toml")) {
                Ok(ref pkg) => Some(pkg.get_summary().clone()),
                Err(e) => {
                    debug!("failed to read manifest; path={}; err={}", path.display(), e);
                    None
                }
            }
        }).collect())
    }

    fn download(&self, _: &[PackageId])  -> CargoResult<()>{
        Ok(())
    }

    fn get(&self, ids: &[PackageId]) -> CargoResult<Vec<Package>> {
        log!(5, "getting packages; ids={}", ids);

        Ok(self.paths.iter().filter_map(|path| {
            match PathSource::read_package(&path.join("Cargo.toml")) {
                Ok(pkg) => {
                    log!(5, "comparing; pkg={}", pkg);

                    if ids.iter().any(|pkg_id| pkg.get_package_id() == pkg_id) {
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

fn display(paths: &[Path]) -> Vec<String> {
    paths.iter().map(|p| p.display().to_str()).collect()
}

fn namespace(path: &Path) -> CargoResult<url::Url> {
    let real = try!(realpath(path).map_err(io_error));
    url::from_str(format!("file://{}", real.display()).as_slice()).map_err(|err|
        simple_human(err.as_slice()))
}
