use std::collections::HashSet;
use std::fmt::{self, Debug, Formatter};
use std::fs;
use std::path::{Path, PathBuf};

use crate::core::source::MaybePackage;
use crate::core::{Dependency, Package, PackageId, Source, SourceId, Summary};
use crate::ops;
use crate::util::{internal, CargoResult, Config};
use anyhow::Context as _;
use cargo_util::paths;
use filetime::FileTime;
use ignore::gitignore::GitignoreBuilder;
use ignore::Match;
use log::{trace, warn};

pub struct PathSource<'cfg> {
    source_id: SourceId,
    path: PathBuf,
    updated: bool,
    packages: Vec<Package>,
    config: &'cfg Config,
    recursive: bool,
}

impl<'cfg> PathSource<'cfg> {
    /// Invoked with an absolute path to a directory that contains a `Cargo.toml`.
    ///
    /// This source will only return the package at precisely the `path`
    /// specified, and it will be an error if there's not a package at `path`.
    pub fn new(path: &Path, source_id: SourceId, config: &'cfg Config) -> PathSource<'cfg> {
        PathSource {
            source_id,
            path: path.to_path_buf(),
            updated: false,
            packages: Vec::new(),
            config,
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
    pub fn new_recursive(root: &Path, id: SourceId, config: &'cfg Config) -> PathSource<'cfg> {
        PathSource {
            recursive: true,
            ..PathSource::new(root, id, config)
        }
    }

    pub fn preload_with(&mut self, pkg: Package) {
        assert!(!self.updated);
        assert!(!self.recursive);
        assert!(self.packages.is_empty());
        self.updated = true;
        self.packages.push(pkg);
    }

    pub fn root_package(&mut self) -> CargoResult<Package> {
        trace!("root_package; source={:?}", self);

        self.update()?;

        match self.packages.iter().find(|p| p.root() == &*self.path) {
            Some(pkg) => Ok(pkg.clone()),
            None => Err(internal(format!(
                "no package found in source {:?}",
                self.path
            ))),
        }
    }

    pub fn read_packages(&self) -> CargoResult<Vec<Package>> {
        if self.updated {
            Ok(self.packages.clone())
        } else if self.recursive {
            ops::read_packages(&self.path, self.source_id, self.config)
        } else {
            let path = self.path.join("Cargo.toml");
            let (pkg, _) = ops::read_package(&path, self.source_id, self.config)?;
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
    pub fn list_files(&self, pkg: &Package) -> CargoResult<Vec<PathBuf>> {
        self._list_files(pkg).with_context(|| {
            format!(
                "failed to determine list of files in {}",
                pkg.root().display()
            )
        })
    }

    fn _list_files(&self, pkg: &Package) -> CargoResult<Vec<PathBuf>> {
        let root = pkg.root();
        let no_include_option = pkg.manifest().include().is_empty();
        let git_repo = if no_include_option {
            self.discover_git_repo(root)?
        } else {
            None
        };

        let mut exclude_builder = GitignoreBuilder::new(root);
        if no_include_option && git_repo.is_none() {
            // no include option and not git repo discovered (see rust-lang/cargo#7183).
            exclude_builder.add_line(None, ".*")?;
        }
        for rule in pkg.manifest().exclude() {
            exclude_builder.add_line(None, rule)?;
        }
        let ignore_exclude = exclude_builder.build()?;

        let mut include_builder = GitignoreBuilder::new(root);
        for rule in pkg.manifest().include() {
            include_builder.add_line(None, rule)?;
        }
        let ignore_include = include_builder.build()?;

        let ignore_should_package = |relative_path: &Path, is_dir: bool| -> CargoResult<bool> {
            // "Include" and "exclude" options are mutually exclusive.
            if no_include_option {
                match ignore_exclude.matched_path_or_any_parents(relative_path, is_dir) {
                    Match::None => Ok(true),
                    Match::Ignore(_) => Ok(false),
                    Match::Whitelist(_) => Ok(true),
                }
            } else {
                if is_dir {
                    // Generally, include directives don't list every
                    // directory (nor should they!). Just skip all directory
                    // checks, and only check files.
                    return Ok(true);
                }
                match ignore_include
                    .matched_path_or_any_parents(relative_path, /* is_dir */ false)
                {
                    Match::None => Ok(false),
                    Match::Ignore(_) => Ok(true),
                    Match::Whitelist(_) => Ok(false),
                }
            }
        };

        let mut filter = |path: &Path, is_dir: bool| -> CargoResult<bool> {
            let relative_path = path.strip_prefix(root)?;

            let rel = relative_path.as_os_str();
            if rel == "Cargo.lock" {
                return Ok(pkg.include_lockfile());
            } else if rel == "Cargo.toml" {
                return Ok(true);
            }

            ignore_should_package(relative_path, is_dir)
        };

        // Attempt Git-prepopulate only if no `include` (see rust-lang/cargo#4135).
        if no_include_option {
            if let Some(repo) = git_repo {
                return self.list_files_git(pkg, &repo, &mut filter);
            }
        }
        self.list_files_walk(pkg, &mut filter)
    }

    /// Returns `Some(git2::Repository)` if found sibling `Cargo.toml` and `.git`
    /// directory; otherwise, caller should fall back on full file list.
    fn discover_git_repo(&self, root: &Path) -> CargoResult<Option<git2::Repository>> {
        let repo = match git2::Repository::discover(root) {
            Ok(repo) => repo,
            Err(e) => {
                log::debug!(
                    "could not discover git repo at or above {}: {}",
                    root.display(),
                    e
                );
                return Ok(None);
            }
        };
        let index = repo
            .index()
            .with_context(|| format!("failed to open git index at {}", repo.path().display()))?;
        let repo_root = repo.workdir().ok_or_else(|| {
            anyhow::format_err!(
                "did not expect repo at {} to be bare",
                repo.path().display()
            )
        })?;
        let repo_relative_path = match paths::strip_prefix_canonical(root, repo_root) {
            Ok(p) => p,
            Err(e) => {
                log::warn!(
                    "cannot determine if path `{:?}` is in git repo `{:?}`: {:?}",
                    root,
                    repo_root,
                    e
                );
                return Ok(None);
            }
        };
        let manifest_path = repo_relative_path.join("Cargo.toml");
        if index.get_path(&manifest_path, 0).is_some() {
            return Ok(Some(repo));
        }
        // Package Cargo.toml is not in git, don't use git to guide our selection.
        Ok(None)
    }

    fn list_files_git(
        &self,
        pkg: &Package,
        repo: &git2::Repository,
        filter: &mut dyn FnMut(&Path, bool) -> CargoResult<bool>,
    ) -> CargoResult<Vec<PathBuf>> {
        warn!("list_files_git {}", pkg.package_id());
        let index = repo.index()?;
        let root = repo
            .workdir()
            .ok_or_else(|| anyhow::format_err!("can't list files on a bare repository"))?;
        let pkg_path = pkg.root();

        let mut ret = Vec::<PathBuf>::new();

        // We use information from the Git repository to guide us in traversing
        // its tree. The primary purpose of this is to take advantage of the
        // `.gitignore` and auto-ignore files that don't matter.
        //
        // Here we're also careful to look at both tracked and untracked files as
        // the untracked files are often part of a build and may become relevant
        // as part of a future commit.
        let index_files = index.iter().map(|entry| {
            use libgit2_sys::{GIT_FILEMODE_COMMIT, GIT_FILEMODE_LINK};
            // ``is_dir`` is an optimization to avoid calling
            // ``fs::metadata`` on every file.
            let is_dir = if entry.mode == GIT_FILEMODE_LINK as u32 {
                // Let the code below figure out if this symbolic link points
                // to a directory or not.
                None
            } else {
                Some(entry.mode == GIT_FILEMODE_COMMIT as u32)
            };
            (join(root, &entry.path), is_dir)
        });
        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true);
        if let Ok(suffix) = pkg_path.strip_prefix(root) {
            opts.pathspec(suffix);
        }
        let statuses = repo.statuses(Some(&mut opts))?;
        let mut skip_paths = HashSet::new();
        let untracked: Vec<_> = statuses
            .iter()
            .filter_map(|entry| {
                match entry.status() {
                    // Don't include Cargo.lock if it is untracked. Packaging will
                    // generate a new one as needed.
                    git2::Status::WT_NEW if entry.path() != Some("Cargo.lock") => {
                        Some(Ok((join(root, entry.path_bytes()), None)))
                    }
                    git2::Status::WT_DELETED => {
                        let path = match join(root, entry.path_bytes()) {
                            Ok(p) => p,
                            Err(e) => return Some(Err(e)),
                        };
                        skip_paths.insert(path);
                        None
                    }
                    _ => None,
                }
            })
            .collect::<CargoResult<_>>()?;

        let mut subpackages_found = Vec::new();

        for (file_path, is_dir) in index_files.chain(untracked) {
            let file_path = file_path?;
            if skip_paths.contains(&file_path) {
                continue;
            }

            // Filter out files blatantly outside this package. This is helped a
            // bit above via the `pathspec` function call, but we need to filter
            // the entries in the index as well.
            if !file_path.starts_with(pkg_path) {
                continue;
            }

            match file_path.file_name().and_then(|s| s.to_str()) {
                // The `target` directory is never included.
                Some("target") => continue,

                // Keep track of all sub-packages found and also strip out all
                // matches we've found so far. Note, though, that if we find
                // our own `Cargo.toml`, we keep going.
                Some("Cargo.toml") => {
                    let path = file_path.parent().unwrap();
                    if path != pkg_path {
                        warn!("subpackage found: {}", path.display());
                        ret.retain(|p| !p.starts_with(path));
                        subpackages_found.push(path.to_path_buf());
                        continue;
                    }
                }

                _ => {}
            }

            // If this file is part of any other sub-package we've found so far,
            // skip it.
            if subpackages_found.iter().any(|p| file_path.starts_with(p)) {
                continue;
            }

            // `is_dir` is None for symlinks. The `unwrap` checks if the
            // symlink points to a directory.
            let is_dir = is_dir.unwrap_or_else(|| file_path.is_dir());
            if is_dir {
                warn!("  found submodule {}", file_path.display());
                let rel = file_path.strip_prefix(root)?;
                let rel = rel.to_str().ok_or_else(|| {
                    anyhow::format_err!("invalid utf-8 filename: {}", rel.display())
                })?;
                // Git submodules are currently only named through `/` path
                // separators, explicitly not `\` which windows uses. Who knew?
                let rel = rel.replace(r"\", "/");
                match repo.find_submodule(&rel).and_then(|s| s.open()) {
                    Ok(repo) => {
                        let files = self.list_files_git(pkg, &repo, filter)?;
                        ret.extend(files.into_iter());
                    }
                    Err(..) => {
                        PathSource::walk(&file_path, &mut ret, false, filter)?;
                    }
                }
            } else if (*filter)(&file_path, is_dir)? {
                assert!(!is_dir);
                // We found a file!
                warn!("  found {}", file_path.display());
                ret.push(file_path);
            }
        }
        return Ok(ret);

        #[cfg(unix)]
        fn join(path: &Path, data: &[u8]) -> CargoResult<PathBuf> {
            use std::ffi::OsStr;
            use std::os::unix::prelude::*;
            Ok(path.join(<OsStr as OsStrExt>::from_bytes(data)))
        }
        #[cfg(windows)]
        fn join(path: &Path, data: &[u8]) -> CargoResult<PathBuf> {
            use std::str;
            match str::from_utf8(data) {
                Ok(s) => Ok(path.join(s)),
                Err(e) => Err(anyhow::format_err!(
                    "cannot process path in git with a non utf8 filename: {}\n{:?}",
                    e,
                    data
                )),
            }
        }
    }

    fn list_files_walk(
        &self,
        pkg: &Package,
        filter: &mut dyn FnMut(&Path, bool) -> CargoResult<bool>,
    ) -> CargoResult<Vec<PathBuf>> {
        let mut ret = Vec::new();
        PathSource::walk(pkg.root(), &mut ret, true, filter)?;
        Ok(ret)
    }

    fn walk(
        path: &Path,
        ret: &mut Vec<PathBuf>,
        is_root: bool,
        filter: &mut dyn FnMut(&Path, bool) -> CargoResult<bool>,
    ) -> CargoResult<()> {
        let is_dir = path.is_dir();
        if !is_root && !(*filter)(path, is_dir)? {
            return Ok(());
        }
        if !is_dir {
            ret.push(path.to_path_buf());
            return Ok(());
        }
        // Don't recurse into any sub-packages that we have.
        if !is_root && path.join("Cargo.toml").exists() {
            return Ok(());
        }

        // For package integration tests, we need to sort the paths in a deterministic order to
        // be able to match stdout warnings in the same order.
        //
        // TODO: drop `collect` and sort after transition period and dropping warning tests.
        // See rust-lang/cargo#4268 and rust-lang/cargo#4270.
        let mut entries: Vec<PathBuf> = fs::read_dir(path)
            .with_context(|| format!("cannot read {:?}", path))?
            .map(|e| e.unwrap().path())
            .collect();
        entries.sort_unstable_by(|a, b| a.as_os_str().cmp(b.as_os_str()));
        for path in entries {
            let name = path.file_name().and_then(|s| s.to_str());
            if is_root && name == Some("target") {
                // Skip Cargo artifacts.
                continue;
            }
            PathSource::walk(&path, ret, false, filter)?;
        }
        Ok(())
    }

    pub fn last_modified_file(&self, pkg: &Package) -> CargoResult<(FileTime, PathBuf)> {
        if !self.updated {
            return Err(internal(format!(
                "BUG: source `{:?}` was not updated",
                self.path
            )));
        }

        let mut max = FileTime::zero();
        let mut max_path = PathBuf::new();
        for file in self.list_files(pkg).with_context(|| {
            format!(
                "failed to determine the most recently modified file in {}",
                pkg.root().display()
            )
        })? {
            // An `fs::stat` error here is either because path is a
            // broken symlink, a permissions error, or a race
            // condition where this path was `rm`-ed -- either way,
            // we can ignore the error and treat the path's `mtime`
            // as `0`.
            let mtime = paths::mtime(&file).unwrap_or_else(|_| FileTime::zero());
            if mtime > max {
                max = mtime;
                max_path = file;
            }
        }
        trace!("last modified file {}: {}", self.path.display(), max);
        Ok((max, max_path))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl<'cfg> Debug for PathSource<'cfg> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "the paths source")
    }
}

impl<'cfg> Source for PathSource<'cfg> {
    fn query(&mut self, dep: &Dependency, f: &mut dyn FnMut(Summary)) -> CargoResult<()> {
        for s in self.packages.iter().map(|p| p.summary()) {
            if dep.matches(s) {
                f(s.clone())
            }
        }
        Ok(())
    }

    fn fuzzy_query(&mut self, _dep: &Dependency, f: &mut dyn FnMut(Summary)) -> CargoResult<()> {
        for s in self.packages.iter().map(|p| p.summary()) {
            f(s.clone())
        }
        Ok(())
    }

    fn supports_checksums(&self) -> bool {
        false
    }

    fn requires_precise(&self) -> bool {
        false
    }

    fn source_id(&self) -> SourceId {
        self.source_id
    }

    fn update(&mut self) -> CargoResult<()> {
        if !self.updated {
            let packages = self.read_packages()?;
            self.packages.extend(packages.into_iter());
            self.updated = true;
        }

        Ok(())
    }

    fn download(&mut self, id: PackageId) -> CargoResult<MaybePackage> {
        trace!("getting packages; id={}", id);

        let pkg = self.packages.iter().find(|pkg| pkg.package_id() == id);
        pkg.cloned()
            .map(MaybePackage::Ready)
            .ok_or_else(|| internal(format!("failed to find {} in path source", id)))
    }

    fn finish_download(&mut self, _id: PackageId, _data: Vec<u8>) -> CargoResult<Package> {
        panic!("no download should have started")
    }

    fn fingerprint(&self, pkg: &Package) -> CargoResult<String> {
        let (max, max_path) = self.last_modified_file(pkg)?;
        // Note that we try to strip the prefix of this package to get a
        // relative path to ensure that the fingerprint remains consistent
        // across entire project directory renames.
        let max_path = max_path.strip_prefix(&self.path).unwrap_or(&max_path);
        Ok(format!("{} ({})", max, max_path.display()))
    }

    fn describe(&self) -> String {
        match self.source_id.url().to_file_path() {
            Ok(path) => path.display().to_string(),
            Err(_) => self.source_id.to_string(),
        }
    }

    fn add_to_yanked_whitelist(&mut self, _pkgs: &[PackageId]) {}

    fn is_yanked(&mut self, _pkg: PackageId) -> CargoResult<bool> {
        Ok(false)
    }
}
