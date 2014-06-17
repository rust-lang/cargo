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
pub struct PathSource {
    path: Path
}

impl PathSource {

    /**
     * Invoked with an absolute path to a directory that contains a Cargo.toml.
     * The source will read the manifest and find any other packages contained
     * in the directory structure reachable by the root manifest.
     */
    pub fn new(path: &Path) -> PathSource {
        log!(5, "new; path={}", path.display());
        PathSource { path: path.clone() }
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
    fn update(&self) -> CargoResult<()> {
        Ok(())
    }

    fn list(&self) -> CargoResult<Vec<Summary>> {
        // TODO: Recursively find manifests

        match PathSource::read_package(&self.path.join("Cargo.toml")) {
            Ok(ref pkg) => Ok(vec!(pkg.get_summary().clone())),
            Err(e) => {
                debug!("failed to read manifest; path={}; err={}", self.path.display(), e);
                Err(e)
            }
        }
    }

    fn download(&self, _: &[PackageId])  -> CargoResult<()>{
        // TODO: assert! that the PackageId is contained by the source
        Ok(())
    }

    fn get(&self, ids: &[PackageId]) -> CargoResult<Vec<Package>> {
        log!(5, "getting packages; ids={}", ids);

        PathSource::read_package(&self.path.join("Cargo.toml")).and_then(|pkg| {
            log!(5, "comparing; pkg={}", pkg);

            if ids.iter().any(|pkg_id| pkg.get_package_id() == pkg_id) {
                Ok(vec!(pkg))
            } else {
                // TODO: Be smarter
                // Err(simple_human(format!("Couldn't find `{}` in path source", ids)))
                Ok(vec!())
            }
        })
    }
}

fn namespace(path: &Path) -> CargoResult<url::Url> {
    let real = try!(realpath(path).map_err(io_error));
    url::from_str(format!("file://{}", real.display()).as_slice()).map_err(|err|
        simple_human(err.as_slice()))
}
