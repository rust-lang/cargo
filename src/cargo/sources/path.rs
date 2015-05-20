use std::cmp;
use std::fmt::{self, Debug, Formatter};
use std::fs;
use std::io::prelude::*;
use std::path::{Path, PathBuf};

use glob::Pattern;
use git2;

use core::{Package, PackageId, Summary, SourceId, Source, Dependency, Registry};
use ops;
use util::{self, CargoResult, internal, internal_error, human, ChainError};
use util::{MTime, Config};

pub struct PathSource<'cfg> {
    id: SourceId,
    path: PathBuf,
    updated: bool,
    packages: Vec<Package>,
    config: &'cfg Config,
}

// TODO: Figure out if packages should be discovered in new or self should be
// mut and packages are discovered in update
impl<'cfg> PathSource<'cfg> {
    pub fn for_path(path: &Path, config: &'cfg Config)
                    -> CargoResult<PathSource<'cfg>> {
        trace!("PathSource::for_path; path={}", path.display());
        Ok(PathSource::new(path, &try!(SourceId::for_path(path)), config))
    }

    /// Invoked with an absolute path to a directory that contains a Cargo.toml.
    /// The source will read the manifest and find any other packages contained
    /// in the directory structure reachable by the root manifest.
    pub fn new(path: &Path, id: &SourceId, config: &'cfg Config)
               -> PathSource<'cfg> {
        trace!("new; id={}", id);

        PathSource {
            id: id.clone(),
            path: path.to_path_buf(),
            updated: false,
            packages: Vec::new(),
            config: config,
        }
    }

    pub fn root_package(&self) -> CargoResult<Package> {
        trace!("root_package; source={:?}", self);

        if !self.updated {
            return Err(internal("source has not been updated"))
        }

        match self.packages.iter().find(|p| p.root() == &*self.path) {
            Some(pkg) => Ok(pkg.clone()),
            None => Err(internal("no package found in source"))
        }
    }

    pub fn read_packages(&self) -> CargoResult<Vec<Package>> {
        if self.updated {
            Ok(self.packages.clone())
        } else if self.id.is_path() && self.id.precise().is_some() {
            // If our source id is a path and it's listed with a precise
            // version, then it means that we're not allowed to have nested
            // dependencies (they've been rewritten to crates.io dependencies)
            // In this case we specifically read just one package, not a list of
            // packages.
            let path = self.path.join("Cargo.toml");
            let (pkg, _) = try!(ops::read_package(&path, &self.id,
                                                  self.config));
            Ok(vec![pkg])
        } else {
            ops::read_packages(&self.path, &self.id, self.config)
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
    pub fn list_files(&self, pkg: &Package) -> CargoResult<Vec<PathBuf>> {
        let root = pkg.root();

        let parse = |p: &String| {
            Pattern::new(p).map_err(|e| {
                human(format!("could not parse pattern `{}`: {}", p, e))
            })
        };
        let exclude = try!(pkg.manifest().exclude().iter()
                              .map(|p| parse(p)).collect::<Result<Vec<_>, _>>());
        let include = try!(pkg.manifest().include().iter()
                              .map(|p| parse(p)).collect::<Result<Vec<_>, _>>());

        let mut filter = |p: &Path| {
            let relative_path = util::without_prefix(p, &root).unwrap();
            include.iter().any(|p| p.matches_path(&relative_path)) || {
                include.len() == 0 &&
                 !exclude.iter().any(|p| p.matches_path(&relative_path))
            }
        };

        // If this package is a git repository, then we really do want to query
        // the git repository as it takes into account items such as .gitignore.
        // We're not quite sure where the git repository is, however, so we do a
        // bit of a probe.
        //
        // We check all packages in this source that are ancestors of the
        // specified package (including the same package) to see if they're at
        // the root of the git repository. This isn't always true, but it'll get
        // us there most of the time!.
        let repo = self.packages.iter()
                       .map(|pkg| pkg.root())
                       .filter(|path| root.starts_with(path))
                       .filter_map(|path| git2::Repository::open(&path).ok())
                       .next();
        match repo {
            Some(repo) => self.list_files_git(pkg, repo, &mut filter),
            None => self.list_files_walk(pkg, &mut filter),
        }
    }

    fn list_files_git(&self, pkg: &Package, repo: git2::Repository,
                      filter: &mut FnMut(&Path) -> bool)
                      -> CargoResult<Vec<PathBuf>> {
        warn!("list_files_git {}", pkg.package_id());
        let index = try!(repo.index());
        let root = try!(repo.workdir().chain_error(|| {
            internal_error("Can't list files on a bare repository.", "")
        }));
        let pkg_path = pkg.root();

        let mut ret = Vec::new();

        // We use information from the git repository to guide use in traversing
        // its tree. The primary purpose of this is to take advantage of the
        // .gitignore and auto-ignore files that don't matter.
        //
        // Here we're also careful to look at both tracked an untracked files as
        // the untracked files are often part of a build and may become relevant
        // as part of a future commit.
        let index_files = index.iter().map(|entry| join(&root, &entry.path));
        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true);
        let statuses = try!(repo.statuses(Some(&mut opts)));
        let untracked = statuses.iter().map(|entry| {
            join(&root, entry.path_bytes())
        });

        'outer: for file_path in index_files.chain(untracked) {
            let file_path = try!(file_path);

            // Filter out files outside this package.
            if !file_path.starts_with(pkg_path) { continue }

            // Filter out Cargo.lock and target always
            {
                let fname = file_path.file_name().and_then(|s| s.to_str());
                if fname == Some("Cargo.lock") { continue }
                if fname == Some("target") { continue }
            }

            // Filter out sub-packages of this package
            for other_pkg in self.packages.iter().filter(|p| *p != pkg) {
                let other_path = other_pkg.root();
                if other_path.starts_with(pkg_path) &&
                   file_path.starts_with(other_path) {
                    continue 'outer;
                }
            }

            // TODO: the `entry` has a mode we should be able to look at instead
            //       of just calling stat() again
            if fs::metadata(&file_path).map(|m| m.is_dir()).unwrap_or(false) {
                warn!("  found submodule {}", file_path.display());
                let rel = util::without_prefix(&file_path, &root).unwrap();
                let rel = try!(rel.to_str().chain_error(|| {
                    human(format!("invalid utf-8 filename: {}", rel.display()))
                }));
                // Git submodules are currently only named through `/` path
                // separators, explicitly not `\` which windows uses. Who knew?
                let rel = rel.replace(r"\", "/");
                match repo.find_submodule(&rel).and_then(|s| s.open()) {
                    Ok(repo) => {
                        let files = try!(self.list_files_git(pkg, repo, filter));
                        ret.extend(files.into_iter());
                    }
                    Err(..) => {
                        try!(PathSource::walk(&file_path, &mut ret, false,
                                              filter));
                    }
                }
            } else if (*filter)(&file_path) {
                // We found a file!
                warn!("  found {}", file_path.display());
                ret.push(file_path);
            }
        }
        return Ok(ret);

        #[cfg(unix)]
        fn join(path: &Path, data: &[u8]) -> CargoResult<PathBuf> {
            use std::os::unix::prelude::*;
            use std::ffi::OsStr;
            Ok(path.join(<OsStr as OsStrExt>::from_bytes(data)))
        }
        #[cfg(windows)]
        fn join(path: &Path, data: &[u8]) -> CargoResult<PathBuf> {
            use std::str;
            match str::from_utf8(data) {
                Ok(s) => Ok(path.join(s)),
                Err(..) => Err(internal("cannot process path in git with a non \
                                         unicode filename")),
            }
        }
    }

    fn list_files_walk(&self, pkg: &Package, filter: &mut FnMut(&Path) -> bool)
                       -> CargoResult<Vec<PathBuf>> {
        let mut ret = Vec::new();
        for pkg in self.packages.iter().filter(|p| *p == pkg) {
            let loc = pkg.root();
            try!(PathSource::walk(loc, &mut ret, true, filter));
        }
        return Ok(ret);
    }

    fn walk(path: &Path, ret: &mut Vec<PathBuf>,
            is_root: bool, filter: &mut FnMut(&Path) -> bool) -> CargoResult<()>
    {
        if !fs::metadata(&path).map(|m| m.is_dir()).unwrap_or(false) {
            if (*filter)(path) {
                ret.push(path.to_path_buf());
            }
            return Ok(())
        }
        // Don't recurse into any sub-packages that we have
        if !is_root && fs::metadata(&path.join("Cargo.toml")).is_ok() {
            return Ok(())
        }
        for dir in try!(fs::read_dir(path)) {
            let dir = try!(dir).path();
            match (is_root, dir.file_name().and_then(|s| s.to_str())) {
                (_,    Some(".git")) |
                (true, Some("target")) |
                (true, Some("Cargo.lock")) => continue,
                _ => {}
            }
            try!(PathSource::walk(&dir, ret, false, filter));
        }
        return Ok(())
    }
}

impl<'cfg> Debug for PathSource<'cfg> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "the paths source")
    }
}

impl<'cfg> Registry for PathSource<'cfg> {
    fn query(&mut self, dep: &Dependency) -> CargoResult<Vec<Summary>> {
        let mut summaries: Vec<Summary> = self.packages.iter()
                                              .map(|p| p.summary().clone())
                                              .collect();
        summaries.query(dep)
    }
}

impl<'cfg> Source for PathSource<'cfg> {
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
        trace!("getting packages; ids={:?}", ids);

        Ok(self.packages.iter()
           .filter(|pkg| ids.iter().any(|id| pkg.package_id() == id))
           .map(|pkg| pkg.clone())
           .collect())
    }

    fn fingerprint(&self, pkg: &Package) -> CargoResult<String> {
        if !self.updated {
            return Err(internal_error("BUG: source was not updated", ""));
        }

        let mut max = MTime::zero();
        for file in try!(self.list_files(pkg)).iter() {
            // An fs::stat error here is either because path is a
            // broken symlink, a permissions error, or a race
            // condition where this path was rm'ed - either way,
            // we can ignore the error and treat the path's mtime
            // as 0.
            let mtime = MTime::of(&file).unwrap_or(MTime::zero());
            warn!("{} {}", mtime, file.display());
            max = cmp::max(max, mtime);
        }
        trace!("fingerprint {}: {}", self.path.display(), max);
        Ok(max.to_string())
    }
}
