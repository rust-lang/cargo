use std::cmp;
use std::fmt::{Show, Formatter};
use std::fmt;
use std::io::fs;

use core::{Package, PackageId, Summary, SourceId, Source, Dependency, Registry};
use ops;
use util::{CargoResult, internal, internal_error, process};

pub struct PathSource {
    id: SourceId,
    path: Path,
    updated: bool,
    packages: Vec<Package>
}

// TODO: Figure out if packages should be discovered in new or self should be
// mut and packages are discovered in update
impl PathSource {

    pub fn for_path(path: &Path) -> CargoResult<PathSource> {
        log!(5, "PathSource::for_path; path={}", path.display());
        Ok(PathSource::new(path, &try!(SourceId::for_path(path))))
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

    /// List all files relevant to building this package inside this source.
    ///
    /// This function will use the appropriate methods to determine what is the
    /// set of files underneath this source's directory which are relevant for
    /// building `pkg`.
    ///
    /// The basic assumption of this method is that all files in the directory
    /// are relevant for building this package, but it also contains logic to
    /// use other methods like .gitignore to filter the list of files.
    pub fn list_files(&self, pkg: &Package) -> CargoResult<Vec<Path>> {
        // TODO: add an `excludes` section to the manifest which is another way
        // to filter files out of this set that is returned.
        return if self.path.join(".git").exists() {
            self.list_files_git(pkg)
        } else {
            self.list_files_walk(pkg)
        };

        fn list_files_git(&self, pkg: &Package) -> CargoResult<Vec<Path>> {
            let cwd = pkg.get_manifest_path().dir_path();
            let mut cmd = process("git").cwd(cwd.clone());
            cmd = cmd.arg("ls-files").arg("-z");

            // Filter out all other packages with a filter directive
            for pkg in self.packages.iter().filter(|p| *p != pkg) {
                if cwd.is_ancestor_of(pkg.get_manifest_path()) {
                    let filter = pkg.get_manifest_path().dir_path()
                                    .path_relative_from(&self.path).unwrap();
                    cmd = cmd.arg("-x").arg(filter);
                }
            }

            log!(5, "listing git files with: {}", cmd);
            let output = try!(cmd.arg(".").exec_with_output());
            let output = output.output.as_slice();
            Ok(output.split(|x| *x == 0).map(Path::new).collect())
        }

        fn list_files_walk(&self, pkg: &Package) -> CargoResult<Vec<Path>> {
            let mut ret = Vec::new();
            for pkg in self.packages.iter().filter(|p| *p == pkg) {
                let loc = pkg.get_manifest_path().dir_path();
                try!(walk(&loc, &mut ret, true));
            }
            return Ok(ret);

            fn walk(path: &Path, ret: &mut Vec<Path>,
                    is_root: bool) -> CargoResult<()> {
                if !path.is_dir() {
                    ret.push(path.clone());
                    return Ok(())
                }
                // Don't recurse into any sub-packages that we have
                if !is_root && path.join("Cargo.toml").exists() { return Ok(()) }
                for dir in try!(fs::readdir(path)).iter() {
                    match (is_root, dir.filename_str()) {
                        (_,    Some(".git")) |
                        (true, Some("target")) |
                        (true, Some("Cargo.lock")) => continue,
                        _ => {}
                    }
                    try!(walk(dir, ret, false));
                }
                return Ok(())
            }
        }
    }
}

impl Show for PathSource {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "the paths source")
    }
}

impl Registry for PathSource {
    fn query(&mut self, dep: &Dependency) -> CargoResult<Vec<Summary>> {
        let mut summaries: Vec<Summary> = self.packages.iter()
                                              .map(|p| p.get_summary().clone())
                                              .collect();
        summaries.query(dep)
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
        if !self.updated {
            return Err(internal_error("BUG: source was not updated", ""));
        }

        let mut max = 0;
        for file in try!(self.list_files(pkg)).iter() {
            // An fs::stat error here is either because path is a
            // broken symlink, a permissions error, or a race
            // condition where this path was rm'ed - either way,
            // we can ignore the error and treat the path's mtime
            // as 0.
            max = cmp::max(max, file.stat().map(|s| s.modified).unwrap_or(0));
        }
        Ok(max.to_string())
    }
}
