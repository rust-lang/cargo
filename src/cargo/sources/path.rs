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
        assert!(id.is_path(), "does not represent a path source; id={}", id);

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
}

impl Show for PathSource {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "the paths source")
    }
}

impl Source for PathSource {
    fn update(&mut self) -> CargoResult<()> {
        if !self.updated {
          let pkgs = try!(ops::read_packages(&self.path, &self.id));
          self.packages.push_all_move(pkgs);
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

    fn fingerprint(&self) -> CargoResult<String> {
        let mut max = None;
        let target_dir = self.path.join("target");
        for child in try!(fs::walk_dir(&self.path)) {
            if target_dir.is_ancestor_of(&child) { continue }
            let stat = try!(fs::stat(&child));
            max = cmp::max(max, Some(stat.modified));
        }
        match max {
            None => Ok(String::new()),
            Some(time) => Ok(time.to_str()),
        }
    }
}
