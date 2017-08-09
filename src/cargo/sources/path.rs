use std::fmt::{self, Debug, Formatter};
use std::fs;
use std::path::{Path, PathBuf};

use filetime::FileTime;
use git2;
use glob::Pattern;
use ignore::Match;
use ignore::gitignore::GitignoreBuilder;

use core::{Package, PackageId, Summary, SourceId, Source, Dependency, Registry};
use ops;
use util::{self, CargoError, CargoResult, internal};
use util::Config;

pub struct PathSource<'cfg> {
    source_id: SourceId,
    path: PathBuf,
    updated: bool,
    packages: Vec<Package>,
    config: &'cfg Config,
    recursive: bool,
}

impl<'cfg> PathSource<'cfg> {
    /// Invoked with an absolute path to a directory that contains a Cargo.toml.
    ///
    /// This source will only return the package at precisely the `path`
    /// specified, and it will be an error if there's not a package at `path`.
    pub fn new(path: &Path, id: &SourceId, config: &'cfg Config)
               -> PathSource<'cfg> {
        PathSource {
            source_id: id.clone(),
            path: path.to_path_buf(),
            updated: false,
            packages: Vec::new(),
            config: config,
            recursive: false,
        }
    }

    /// Creates a new source which is walked recursively to discover packages.
    ///
    /// This is similar to the `new` method except that instead of requiring a
    /// valid package to be present at `root` the folder is walked entirely to
    /// crawl for packages.
    ///
    /// Note that this should be used with care and likely shouldn't be chosen
    /// by default!
    pub fn new_recursive(root: &Path, id: &SourceId, config: &'cfg Config)
                         -> PathSource<'cfg> {
        PathSource {
            recursive: true,
            .. PathSource::new(root, id, config)
        }
    }

    pub fn root_package(&mut self) -> CargoResult<Package> {
        trace!("root_package; source={:?}", self);

        self.update()?;

        match self.packages.iter().find(|p| p.root() == &*self.path) {
            Some(pkg) => Ok(pkg.clone()),
            None => Err(internal("no package found in source"))
        }
    }

    pub fn read_packages(&self) -> CargoResult<Vec<Package>> {
        if self.updated {
            Ok(self.packages.clone())
        } else if self.recursive {
            ops::read_packages(&self.path, &self.source_id, self.config)
        } else {
            let path = self.path.join("Cargo.toml");
            let (pkg, _) = ops::read_package(&path, &self.source_id, self.config)?;
            Ok(vec![pkg])
        }
    }

    /// List all files relevant to building this package inside this source.
    ///
    /// This function will use the appropriate methods to determine the
    /// set of files underneath this source's directory which are relevant for
    /// building `pkg`.
    ///
    /// The basic assumption of this method is that all files in the directory
    /// are relevant for building this package, but it also contains logic to
    /// use other methods like .gitignore to filter the list of files.
    ///
    /// ## Pattern matching strategy
    ///
    /// Migrating from a glob-like pattern matching (using `glob` crate) to a
    /// gitignore-like pattern matching (using `ignore` crate). The migration
    /// stages are:
    ///
    /// 1) Only warn users about the future change iff their matching rules are
    ///    affected.  (CURRENT STAGE)
    ///
    /// 2) Switch to the new strategy and upate documents. Still keep warning
    ///    affected users.
    ///
    /// 3) Drop the old strategy and no mor warnings.
    ///
    /// See <https://github.com/rust-lang/cargo/issues/4268> for more info.
    pub fn list_files(&self, pkg: &Package) -> CargoResult<Vec<PathBuf>> {
        let root = pkg.root();
        let no_include_option = pkg.manifest().include().is_empty();

        // glob-like matching rules

        let glob_parse = |p: &String| {
            let pattern: &str = if p.starts_with('/') {
                &p[1..p.len()]
            } else {
                &p
            };
            Pattern::new(pattern).map_err(|e| {
                CargoError::from(format!("could not parse glob pattern `{}`: {}", p, e))
            })
        };

        let glob_exclude = pkg.manifest()
            .exclude()
            .iter()
            .map(|p| glob_parse(p))
            .collect::<Result<Vec<_>, _>>()?;

        let glob_include = pkg.manifest()
            .include()
            .iter()
            .map(|p| glob_parse(p))
            .collect::<Result<Vec<_>, _>>()?;

        let glob_should_package = |relative_path: &Path| -> bool {
            fn glob_match(patterns: &Vec<Pattern>, relative_path: &Path) -> bool {
                patterns.iter().any(|pattern| pattern.matches_path(relative_path))
            }

            // include and exclude options are mutually exclusive.
            if no_include_option {
                !glob_match(&glob_exclude, relative_path)
            } else {
                glob_match(&glob_include, relative_path)
            }
        };

        // ignore-like matching rules

        let mut exclude_builder = GitignoreBuilder::new(root);
        for rule in pkg.manifest().exclude() {
            exclude_builder.add_line(None, rule)?;
        }
        let ignore_exclude = exclude_builder.build()?;

        let mut include_builder = GitignoreBuilder::new(root);
        for rule in pkg.manifest().include() {
            include_builder.add_line(None, rule)?;
        }
        let ignore_include = include_builder.build()?;

        let ignore_should_package = |relative_path: &Path| -> CargoResult<bool> {
            // include and exclude options are mutually exclusive.
            if no_include_option {
                match ignore_exclude.matched_path_or_any_parents(
                    relative_path,
                    /* is_dir */ false,
                ) {
                    Match::None => Ok(true),
                    Match::Ignore(_) => Ok(false),
                    Match::Whitelist(pattern) => Err(CargoError::from(format!(
                        "exclude rules cannot start with `!`: {}",
                        pattern.original()
                    ))),
                }
            } else {
                match ignore_include.matched_path_or_any_parents(
                    relative_path,
                    /* is_dir */ false,
                ) {
                    Match::None => Ok(false),
                    Match::Ignore(_) => Ok(true),
                    Match::Whitelist(pattern) => Err(CargoError::from(format!(
                        "include rules cannot start with `!`: {}",
                        pattern.original()
                    ))),
                }
            }
        };

        // matching to paths

        let mut filter = |path: &Path| -> CargoResult<bool> {
            let relative_path = util::without_prefix(path, root).unwrap();
            let glob_should_package = glob_should_package(relative_path);
            let ignore_should_package = ignore_should_package(relative_path)?;

            if glob_should_package != ignore_should_package {
                if glob_should_package {
                    if no_include_option {
                        self.config
                            .shell()
                            .warn(format!(
                                "Pattern matching for Cargo's include/exclude fields is changing and \
                                file `{}` WILL be excluded in a future Cargo version.\n\
                                See https://github.com/rust-lang/cargo/issues/4268 for more info",
                                relative_path.display()
                            ))?;
                    } else {
                        self.config
                            .shell()
                            .warn(format!(
                                "Pattern matching for Cargo's include/exclude fields is changing and \
                                file `{}` WILL NOT be included in a future Cargo version.\n\
                                See https://github.com/rust-lang/cargo/issues/4268 for more info",
                                relative_path.display()
                            ))?;
                    }
                } else {
                    if no_include_option {
                        self.config
                            .shell()
                            .warn(format!(
                                "Pattern matching for Cargo's include/exclude fields is changing and \
                                file `{}` WILL NOT be excluded in a future Cargo version.\n\
                                See https://github.com/rust-lang/cargo/issues/4268 for more info",
                                relative_path.display()
                            ))?;
                    } else {
                        self.config
                            .shell()
                            .warn(format!(
                                "Pattern matching for Cargo's include/exclude fields is changing and \
                                file `{}` WILL be included in a future Cargo version.\n\
                                See https://github.com/rust-lang/cargo/issues/4268 for more info",
                                relative_path.display()
                            ))?;
                    }
                }
            }

            // Update to ignore_should_package for Stage 2
            Ok(glob_should_package)
        };

        // attempt git-prepopulate only if no `include` (rust-lang/cargo#4135)
        if no_include_option {
            if let Some(result) = self.discover_git_and_list_files(pkg, root, &mut filter) {
                return result;
            }
        }
        self.list_files_walk(pkg, &mut filter)
    }

    // Returns Some(_) if found sibling Cargo.toml and .git folder;
    // otherwise caller should fall back on full file list.
    fn discover_git_and_list_files(&self,
                                   pkg: &Package,
                                   root: &Path,
                                   filter: &mut FnMut(&Path) -> CargoResult<bool>)
                                   -> Option<CargoResult<Vec<PathBuf>>> {
        // If this package is in a git repository, then we really do want to
        // query the git repository as it takes into account items such as
        // .gitignore. We're not quite sure where the git repository is,
        // however, so we do a bit of a probe.
        //
        // We walk this package's path upwards and look for a sibling
        // Cargo.toml and .git folder. If we find one then we assume that we're
        // part of that repository.
        let mut cur = root;
        loop {
            if cur.join("Cargo.toml").is_file() {
                // If we find a git repository next to this Cargo.toml, we still
                // check to see if we are indeed part of the index. If not, then
                // this is likely an unrelated git repo, so keep going.
                if let Ok(repo) = git2::Repository::open(cur) {
                    let index = match repo.index() {
                        Ok(index) => index,
                        Err(err) => return Some(Err(err.into())),
                    };
                    let path = util::without_prefix(root, cur)
                                    .unwrap().join("Cargo.toml");
                    if index.get_path(&path, 0).is_some() {
                        return Some(self.list_files_git(pkg, repo, filter));
                    }
                }
            }
            // don't cross submodule boundaries
            if cur.join(".git").is_dir() {
                break
            }
            match cur.parent() {
                Some(parent) => cur = parent,
                None => break,
            }
        }
        return None;
    }

    fn list_files_git(&self, pkg: &Package, repo: git2::Repository,
                      filter: &mut FnMut(&Path) -> CargoResult<bool>)
                      -> CargoResult<Vec<PathBuf>> {
        warn!("list_files_git {}", pkg.package_id());
        let index = repo.index()?;
        let root = repo.workdir().ok_or_else(|| {
            internal("Can't list files on a bare repository.")
        })?;
        let pkg_path = pkg.root();

        let mut ret = Vec::<PathBuf>::new();

        // We use information from the git repository to guide us in traversing
        // its tree. The primary purpose of this is to take advantage of the
        // .gitignore and auto-ignore files that don't matter.
        //
        // Here we're also careful to look at both tracked and untracked files as
        // the untracked files are often part of a build and may become relevant
        // as part of a future commit.
        let index_files = index.iter().map(|entry| {
            use libgit2_sys::GIT_FILEMODE_COMMIT;
            let is_dir = entry.mode == GIT_FILEMODE_COMMIT as u32;
            (join(root, &entry.path), Some(is_dir))
        });
        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true);
        if let Some(suffix) = util::without_prefix(pkg_path, root) {
            opts.pathspec(suffix);
        }
        let statuses = repo.statuses(Some(&mut opts))?;
        let untracked = statuses.iter().filter_map(|entry| {
            match entry.status() {
                git2::STATUS_WT_NEW => Some((join(root, entry.path_bytes()), None)),
                _ => None,
            }
        });

        let mut subpackages_found = Vec::new();

        for (file_path, is_dir) in index_files.chain(untracked) {
            let file_path = file_path?;

            // Filter out files blatantly outside this package. This is helped a
            // bit obove via the `pathspec` function call, but we need to filter
            // the entries in the index as well.
            if !file_path.starts_with(pkg_path) {
                continue
            }

            match file_path.file_name().and_then(|s| s.to_str()) {
                // Filter out Cargo.lock and target always, we don't want to
                // package a lock file no one will ever read and we also avoid
                // build artifacts
                Some("Cargo.lock") |
                Some("target") => continue,

                // Keep track of all sub-packages found and also strip out all
                // matches we've found so far. Note, though, that if we find
                // our own `Cargo.toml` we keep going.
                Some("Cargo.toml") => {
                    let path = file_path.parent().unwrap();
                    if path != pkg_path {
                        warn!("subpackage found: {}", path.display());
                        ret.retain(|p| !p.starts_with(path));
                        subpackages_found.push(path.to_path_buf());
                        continue
                    }
                }

                _ => {}
            }

            // If this file is part of any other sub-package we've found so far,
            // skip it.
            if subpackages_found.iter().any(|p| file_path.starts_with(p)) {
                continue
            }

            if is_dir.unwrap_or_else(|| file_path.is_dir()) {
                warn!("  found submodule {}", file_path.display());
                let rel = util::without_prefix(&file_path, root).unwrap();
                let rel = rel.to_str().ok_or_else(|| {
                    CargoError::from(format!("invalid utf-8 filename: {}", rel.display()))
                })?;
                // Git submodules are currently only named through `/` path
                // separators, explicitly not `\` which windows uses. Who knew?
                let rel = rel.replace(r"\", "/");
                match repo.find_submodule(&rel).and_then(|s| s.open()) {
                    Ok(repo) => {
                        let files = self.list_files_git(pkg, repo, filter)?;
                        ret.extend(files.into_iter());
                    }
                    Err(..) => {
                        PathSource::walk(&file_path, &mut ret, false, filter)?;
                    }
                }
            } else if (*filter)(&file_path)? {
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

    fn list_files_walk(&self, pkg: &Package, filter: &mut FnMut(&Path) -> CargoResult<bool>)
                       -> CargoResult<Vec<PathBuf>> {
        let mut ret = Vec::new();
        PathSource::walk(pkg.root(), &mut ret, true, filter)?;
        Ok(ret)
    }

    fn walk(path: &Path, ret: &mut Vec<PathBuf>,
            is_root: bool, filter: &mut FnMut(&Path) -> CargoResult<bool>)
            -> CargoResult<()>
    {
        if !fs::metadata(&path).map(|m| m.is_dir()).unwrap_or(false) {
            if (*filter)(path)? {
                ret.push(path.to_path_buf());
            }
            return Ok(())
        }
        // Don't recurse into any sub-packages that we have
        if !is_root && fs::metadata(&path.join("Cargo.toml")).is_ok() {
            return Ok(())
        }

        // For package integration tests, we need to sort the paths in a deterministic order to
        // be able to match stdout warnings in the same order.
        //
        // TODO: Drop collect and sort after transition period and dropping wraning tests.
        // See <https://github.com/rust-lang/cargo/issues/4268>
        // and <https://github.com/rust-lang/cargo/pull/4270>
        let mut entries: Vec<fs::DirEntry> = fs::read_dir(path)?.map(|e| e.unwrap()).collect();
        entries.sort_by(|a, b| a.path().as_os_str().cmp(b.path().as_os_str()));
        for entry in entries {
            let path = entry.path();
            let name = path.file_name().and_then(|s| s.to_str());
            // Skip dotfile directories
            if name.map(|s| s.starts_with('.')) == Some(true) {
                continue
            } else if is_root {
                // Skip cargo artifacts
                match name {
                    Some("target") | Some("Cargo.lock") => continue,
                    _ => {}
                }
            }
            PathSource::walk(&path, ret, false, filter)?;
        }
        Ok(())
    }
}

impl<'cfg> Debug for PathSource<'cfg> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "the paths source")
    }
}

impl<'cfg> Registry for PathSource<'cfg> {
    fn query(&mut self,
             dep: &Dependency,
             f: &mut FnMut(Summary)) -> CargoResult<()> {
        for s in self.packages.iter().map(|p| p.summary()) {
            if dep.matches(s) {
                f(s.clone())
            }
        }
        Ok(())
    }
}

impl<'cfg> Source for PathSource<'cfg> {
    fn source_id(&self) -> &SourceId {
        &self.source_id
    }

    fn update(&mut self) -> CargoResult<()> {
        if !self.updated {
            let packages = self.read_packages()?;
            self.packages.extend(packages.into_iter());
            self.updated = true;
        }

        Ok(())
    }

    fn download(&mut self, id: &PackageId) -> CargoResult<Package> {
        trace!("getting packages; id={}", id);

        let pkg = self.packages.iter().find(|pkg| pkg.package_id() == id);
        pkg.cloned().ok_or_else(|| {
            internal(format!("failed to find {} in path source", id))
        })
    }

    fn fingerprint(&self, pkg: &Package) -> CargoResult<String> {
        if !self.updated {
            return Err(internal("BUG: source was not updated"));
        }

        let mut max = FileTime::zero();
        let mut max_path = PathBuf::from("");
        for file in self.list_files(pkg)? {
            // An fs::stat error here is either because path is a
            // broken symlink, a permissions error, or a race
            // condition where this path was rm'ed - either way,
            // we can ignore the error and treat the path's mtime
            // as 0.
            let mtime = fs::metadata(&file).map(|meta| {
                FileTime::from_last_modification_time(&meta)
            }).unwrap_or(FileTime::zero());
            warn!("{} {}", mtime, file.display());
            if mtime > max {
                max = mtime;
                max_path = file;
            }
        }
        trace!("fingerprint {}: {}", self.path.display(), max);
        Ok(format!("{} ({})", max, max_path.display()))
    }
}
