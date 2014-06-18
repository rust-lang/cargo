use std::fmt;
use std::fmt::{Show,Formatter};
use core::{Package,PackageId,Summary,SourceId,Source};
use ops;
use util::{CargoResult,simple_human};

pub struct PathSource {
    id: SourceId,
    path: Path,
}

/**
 * TODO: Figure out if packages should be discovered in new or self should be
 * mut and packages are discovered in update
 */
impl PathSource {

    /**
     * Invoked with an absolute path to a directory that contains a Cargo.toml.
     * The source will read the manifest and find any other packages contained
     * in the directory structure reachable by the root manifest.
     */
    pub fn new(id: &SourceId) -> PathSource {
        log!(5, "new; id={}", id);
        assert!(id.is_path(), "does not represent a path source; id={}", id);

        let path = Path::new(id.get_url().path.as_slice());

        PathSource {
            id: id.clone(),
            path: path
        }
    }

    pub fn get_root_package(&self) -> CargoResult<Package> {
        log!(5, "get_root_package; source={}", self);

        match (try!(self.packages())).as_slice().head() {
            Some(pkg) => Ok(pkg.clone()),
            None => Err(simple_human("no package found in source"))
        }
    }

    fn packages(&self) -> CargoResult<Vec<Package>> {
        ops::read_packages(&self.path, &self.id)
    }
}

impl Show for PathSource {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "the paths source")
    }
}

impl Source for PathSource {
    fn update(&mut self) -> CargoResult<()> {
        Ok(())
    }

    fn list(&self) -> CargoResult<Vec<Summary>> {
        let pkgs = try!(self.packages());
        Ok(pkgs.iter().map(|p| p.get_summary().clone()).collect())
    }

    fn download(&self, _: &[PackageId])  -> CargoResult<()>{
        // TODO: assert! that the PackageId is contained by the source
        Ok(())
    }

    fn get(&self, ids: &[PackageId]) -> CargoResult<Vec<Package>> {
        log!(5, "getting packages; ids={}", ids);

        let pkgs = try!(self.packages());

        Ok(pkgs.iter()
           .filter(|pkg| ids.iter().any(|id| pkg.get_package_id() == id))
           .map(|pkg| pkg.clone())
           .collect())
    }
}
