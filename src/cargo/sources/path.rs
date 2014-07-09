use std::cmp;
use std::fmt::{Show, Formatter};
use std::fmt;
use std::io::fs;

use core::{Package, PackageId, Summary, SourceId, Source};
use ops;
use util::{CargoResult, internal};

pub struct PathSource {
    id: SourceId,
    path: Path,
    updated: bool,
    packages: Vec<Package>
}

// TODO: Figure out if packages should be discovered in new or self should be
// mut and packages are discovered in update
impl PathSource {

    pub fn for_path(path: &Path) -> PathSource {
        PathSource::new(path, &SourceId::for_path(path))
    }

    /// Invoked with an absolute path to a directory that contains a Cargo.toml.
    /// The source will read the manifest and find any other packages contained
    /// in the directory structure reachable by the root manifest.
    pub fn new(path: &Path, id: &SourceId) -> PathSource {
        log!(5, "new; id={}", id);

        PathSource {
            id: id.clone(),
            path: path.clone(),
            updated: false,
            packages: Vec::new()
        }
    }

    pub fn get_root_package(&self) -> CargoResult<Package> {
        log!(5, "get_root_package; source={}", self);

        if !self.updated {
            return Err(internal("source has not been updated"))
        }

        match self.packages.as_slice().head() {
            Some(pkg) => Ok(pkg.clone()),
            None => Err(internal("no package found in source"))
        }
    }

    fn read_packages(&self) -> CargoResult<Vec<Package>> {
        if self.updated {
            Ok(self.packages.clone())
        } else {
            ops::read_packages(&self.path, &self.id)
        }
    }
}

impl Show for PathSource {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "the paths source")
    }
}

impl Source for PathSource {
    fn update(&mut self) -> CargoResult<()> {
        if !self.updated {
            let packages = try!(self.read_packages());
            self.packages.push_all_move(packages);
            self.updated = true;
        }

        Ok(())
    }

    fn list(&self) -> CargoResult<Vec<Summary>> {
        Ok(self.packages.iter()
           .map(|p| p.get_summary().clone())
           .collect())
    }

    fn download(&self, _: &[PackageId])  -> CargoResult<()>{
        // TODO: assert! that the PackageId is contained by the source
        Ok(())
    }

    fn get(&self, ids: &[PackageId]) -> CargoResult<Vec<Package>> {
        log!(5, "getting packages; ids={}", ids);

        Ok(self.packages.iter()
           .filter(|pkg| ids.iter().any(|id| pkg.get_package_id() == id))
           .map(|pkg| pkg.clone())
           .collect())
    }

    fn fingerprint(&self, pkg: &Package) -> CargoResult<String> {
        let packages = try!(self.read_packages());
        let mut max = 0;
        for pkg in packages.iter().filter(|p| *p == pkg) {
            let loc = pkg.get_manifest_path().dir_path();
            max = cmp::max(max, try!(walk(&loc, true)));
        }
        return Ok(max.to_string());

        fn walk(path: &Path, is_root: bool) -> CargoResult<u64> {
            if !path.is_dir() {
                // An fs::stat error here is either because path is a
                // broken symlink, a permissions error, or a race
                // condition where this path was rm'ed - either way,
                // we can ignore the error and treat the path's mtime
                // as 0.
                return Ok(fs::stat(path).map(|s| s.modified).unwrap_or(0))
            }
            // Don't recurse into any sub-packages that we have
            if !is_root && path.join("Cargo.toml").exists() { return Ok(0) }

            let mut max = 0;
            for dir in try!(fs::readdir(path)).iter() {
                if is_root && dir.filename_str() == Some("target") { continue }
                max = cmp::max(max, try!(walk(dir, false)));
            }
            return Ok(max)
        }
    }
}
