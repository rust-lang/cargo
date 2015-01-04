use std::cmp;
use std::fmt::{self, Show, Formatter};
use std::io::fs::{self, PathExtensions};
use glob::Pattern;
use git2;

use core::{Package, PackageId, Summary, SourceId, Source, Dependency, Registry};
use ops;
use util::{CargoResult, internal, internal_error};

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

        match self.packages.iter().find(|p| p.get_root() == self.path) {
            Some(pkg) => Ok(pkg.clone()),
            None => Err(internal("no package found in source"))
        }
    }

    pub fn read_packages(&self) -> CargoResult<Vec<Package>> {
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
        let root = pkg.get_manifest_path().dir_path();

        // Check whether the package itself is a git repository.
        let candidates = match git2::Repository::open(&root) {
            Ok(repo) => try!(self.list_files_git(pkg, repo)),

            // If not, check whether the package is in a sub-directory of the main repository.
            Err(..) if self.path.is_ancestor_of(&root) => {
                match git2::Repository::open(&self.path) {
                    Ok(repo) => try!(self.list_files_git(pkg, repo)),
                    _ => try!(self.list_files_walk(pkg))
                }
            }
            // If neither is true, fall back to walking the filesystem.
            _ => try!(self.list_files_walk(pkg))
        };

        let pats = pkg.get_manifest().get_exclude().iter().map(|p| {
            Pattern::new(p.as_slice())
        }).collect::<Vec<Pattern>>();

        Ok(candidates.into_iter().filter(|candidate| {
            let relative_path = candidate.path_relative_from(&root).unwrap();
            !pats.iter().any(|p| p.matches_path(&relative_path)) &&
                candidate.is_file()
        }).collect())
    }

    fn list_files_git(&self, pkg: &Package, repo: git2::Repository)
                      -> CargoResult<Vec<Path>> {
        warn!("list_files_git {}", pkg.get_package_id());
        let index = try!(repo.index());
        let root = match repo.workdir() {
            Some(dir) => dir,
            None => return Err(internal_error("Can't list files on a bare repository.", "")),
        };
        let pkg_path = pkg.get_manifest_path().dir_path();

        let mut ret = Vec::new();
        'outer: for i in range(0, index.len()) {
            let entry = match index.get(i) { Some(e) => e, None => continue };
            let fname = entry.path.as_bytes_no_nul();
            let file_path = root.join(fname);

            // Filter out files outside this package.
            if !pkg_path.is_ancestor_of(&file_path) { continue }

            // Filter out Cargo.lock and target always
            if fname == b"Cargo.lock" { continue }
            if fname == b"target" { continue }

            // Filter out sub-packages of this package
            for other_pkg in self.packages.iter().filter(|p| *p != pkg) {
                let other_path = other_pkg.get_manifest_path().dir_path();
                if pkg_path.is_ancestor_of(&other_path) && other_path.is_ancestor_of(&file_path) {
                    continue 'outer;
                }
            }

            // We found a file!
            warn!("  found {}", file_path.display());
            ret.push(file_path);
        }
        Ok(ret)
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
            self.packages.extend(packages.into_iter());
            self.updated = true;
        }

        Ok(())
    }

    fn download(&mut self, _: &[PackageId])  -> CargoResult<()>{
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
            warn!("{} {}", file.stat().map(|s| s.modified).unwrap_or(0), file.display());
            max = cmp::max(max, file.stat().map(|s| s.modified).unwrap_or(0));
        }
        log!(5, "fingerprint {}: {}", self.path.display(), max);
        Ok(max.to_string())
    }
}
