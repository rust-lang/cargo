use std::fmt::{self, Debug, Formatter};
use std::path::{Path, PathBuf};
use std::task::Poll;

use crate::core::{Dependency, Package, PackageId, SourceId};
use crate::ops;
use crate::sources::source::MaybePackage;
use crate::sources::source::QueryKind;
use crate::sources::source::Source;
use crate::sources::IndexSummary;
use crate::util::{internal, CargoResult, GlobalContext};
use anyhow::Context as _;
use cargo_util::paths;
use filetime::FileTime;
use gix::bstr::{BString, ByteVec};
use gix::dir::entry::Status;
use gix::index::entry::Stage;
use ignore::gitignore::GitignoreBuilder;
use tracing::{debug, trace, warn};
use walkdir::WalkDir;

/// A source that represents a package gathered at the root
/// path on the filesystem.
///
/// It also provides convenient methods like [`PathSource::list_files`] to
/// list all files in a package, given its ability to walk the filesystem.
pub struct PathSource<'gctx> {
    /// The unique identifier of this source.
    source_id: SourceId,
    /// The root path of this source.
    path: PathBuf,
    /// Whether this source has updated all package information it may contain.
    updated: bool,
    /// Packages that this sources has discovered.
    package: Option<Package>,
    gctx: &'gctx GlobalContext,
}

impl<'gctx> PathSource<'gctx> {
    /// Invoked with an absolute path to a directory that contains a `Cargo.toml`.
    ///
    /// This source will only return the package at precisely the `path`
    /// specified, and it will be an error if there's not a package at `path`.
    pub fn new(path: &Path, source_id: SourceId, gctx: &'gctx GlobalContext) -> Self {
        Self {
            source_id,
            path: path.to_path_buf(),
            updated: false,
            package: None,
            gctx,
        }
    }

    /// Preloads a package for this source. The source is assumed that it has
    /// yet loaded any other packages.
    pub fn preload_with(pkg: Package, gctx: &'gctx GlobalContext) -> Self {
        let source_id = pkg.package_id().source_id();
        let path = pkg.root().to_owned();
        Self {
            source_id,
            path,
            updated: true,
            package: Some(pkg),
            gctx,
        }
    }

    /// Gets the package on the root path.
    pub fn root_package(&mut self) -> CargoResult<Package> {
        trace!("root_package; source={:?}", self);

        self.update()?;

        match &self.package {
            Some(pkg) => Ok(pkg.clone()),
            None => Err(internal(format!(
                "no package found in source {:?}",
                self.path
            ))),
        }
    }

    /// Returns the packages discovered by this source. It may walk the
    /// filesystem if package information haven't yet updated.
    pub fn read_packages(&self) -> CargoResult<Vec<Package>> {
        if self.updated {
            Ok(self.package.clone().into_iter().collect())
        } else {
            let pkg = self.read_package()?;
            Ok(vec![pkg])
        }
    }

    fn read_package(&self) -> CargoResult<Package> {
        let path = self.path.join("Cargo.toml");
        let pkg = ops::read_package(&path, self.source_id, self.gctx)?;
        Ok(pkg)
    }

    /// List all files relevant to building this package inside this source.
    ///
    /// This function will use the appropriate methods to determine the
    /// set of files underneath this source's directory which are relevant for
    /// building `pkg`.
    ///
    /// The basic assumption of this method is that all files in the directory
    /// are relevant for building this package, but it also contains logic to
    /// use other methods like `.gitignore`, `package.include`, or
    /// `package.exclude` to filter the list of files.
    pub fn list_files(&self, pkg: &Package) -> CargoResult<Vec<PathBuf>> {
        list_files(pkg, self.gctx)
    }

    /// Gets the last modified file in a package.
    pub fn last_modified_file(&self, pkg: &Package) -> CargoResult<(FileTime, PathBuf)> {
        if !self.updated {
            return Err(internal(format!(
                "BUG: source `{:?}` was not updated",
                self.path
            )));
        }
        last_modified_file(&self.path, pkg, self.gctx)
    }

    /// Returns the root path of this source.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Discovers packages inside this source if it hasn't yet done.
    pub fn update(&mut self) -> CargoResult<()> {
        if !self.updated {
            self.package = Some(self.read_package()?);
            self.updated = true;
        }

        Ok(())
    }
}

impl<'gctx> Debug for PathSource<'gctx> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "the paths source")
    }
}

impl<'gctx> Source for PathSource<'gctx> {
    fn query(
        &mut self,
        dep: &Dependency,
        kind: QueryKind,
        f: &mut dyn FnMut(IndexSummary),
    ) -> Poll<CargoResult<()>> {
        self.update()?;
        if let Some(s) = self.package.as_ref().map(|p| p.summary()) {
            let matched = match kind {
                QueryKind::Exact => dep.matches(s),
                QueryKind::Alternatives => true,
                QueryKind::Normalized => dep.matches(s),
            };
            if matched {
                f(IndexSummary::Candidate(s.clone()))
            }
        }
        Poll::Ready(Ok(()))
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

    fn download(&mut self, id: PackageId) -> CargoResult<MaybePackage> {
        trace!("getting packages; id={}", id);
        self.update()?;
        let pkg = self.package.iter().find(|pkg| pkg.package_id() == id);
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

    fn is_yanked(&mut self, _pkg: PackageId) -> Poll<CargoResult<bool>> {
        Poll::Ready(Ok(false))
    }

    fn block_until_ready(&mut self) -> CargoResult<()> {
        self.update()
    }

    fn invalidate_cache(&mut self) {
        // Path source has no local cache.
    }

    fn set_quiet(&mut self, _quiet: bool) {
        // Path source does not display status
    }
}

/// A source that represents one or multiple packages gathered from a given root
/// path on the filesystem.
pub struct RecursivePathSource<'gctx> {
    /// The unique identifier of this source.
    source_id: SourceId,
    /// The root path of this source.
    path: PathBuf,
    /// Whether this source has updated all package information it may contain.
    updated: bool,
    /// Packages that this sources has discovered.
    packages: Vec<Package>,
    gctx: &'gctx GlobalContext,
}

impl<'gctx> RecursivePathSource<'gctx> {
    /// Creates a new source which is walked recursively to discover packages.
    ///
    /// This is similar to the [`PathSource::new`] method except that instead
    /// of requiring a valid package to be present at `root` the folder is
    /// walked entirely to crawl for packages.
    ///
    /// Note that this should be used with care and likely shouldn't be chosen
    /// by default!
    pub fn new(root: &Path, source_id: SourceId, gctx: &'gctx GlobalContext) -> Self {
        Self {
            source_id,
            path: root.to_path_buf(),
            updated: false,
            packages: Vec::new(),
            gctx,
        }
    }

    /// Returns the packages discovered by this source. It may walk the
    /// filesystem if package information haven't yet updated.
    pub fn read_packages(&self) -> CargoResult<Vec<Package>> {
        if self.updated {
            Ok(self.packages.clone())
        } else {
            ops::read_packages(&self.path, self.source_id, self.gctx)
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
    /// use other methods like `.gitignore`, `package.include`, or
    /// `package.exclude` to filter the list of files.
    pub fn list_files(&self, pkg: &Package) -> CargoResult<Vec<PathBuf>> {
        list_files(pkg, self.gctx)
    }

    /// Gets the last modified file in a package.
    pub fn last_modified_file(&self, pkg: &Package) -> CargoResult<(FileTime, PathBuf)> {
        if !self.updated {
            return Err(internal(format!(
                "BUG: source `{:?}` was not updated",
                self.path
            )));
        }
        last_modified_file(&self.path, pkg, self.gctx)
    }

    /// Returns the root path of this source.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Discovers packages inside this source if it hasn't yet done.
    pub fn update(&mut self) -> CargoResult<()> {
        if !self.updated {
            let packages = self.read_packages()?;
            self.packages.extend(packages.into_iter());
            self.updated = true;
        }

        Ok(())
    }
}

impl<'gctx> Debug for RecursivePathSource<'gctx> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "the paths source")
    }
}

impl<'gctx> Source for RecursivePathSource<'gctx> {
    fn query(
        &mut self,
        dep: &Dependency,
        kind: QueryKind,
        f: &mut dyn FnMut(IndexSummary),
    ) -> Poll<CargoResult<()>> {
        self.update()?;
        for s in self.packages.iter().map(|p| p.summary()) {
            let matched = match kind {
                QueryKind::Exact => dep.matches(s),
                QueryKind::Alternatives => true,
                QueryKind::Normalized => dep.matches(s),
            };
            if matched {
                f(IndexSummary::Candidate(s.clone()))
            }
        }
        Poll::Ready(Ok(()))
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

    fn download(&mut self, id: PackageId) -> CargoResult<MaybePackage> {
        trace!("getting packages; id={}", id);
        self.update()?;
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

    fn is_yanked(&mut self, _pkg: PackageId) -> Poll<CargoResult<bool>> {
        Poll::Ready(Ok(false))
    }

    fn block_until_ready(&mut self) -> CargoResult<()> {
        self.update()
    }

    fn invalidate_cache(&mut self) {
        // Path source has no local cache.
    }

    fn set_quiet(&mut self, _quiet: bool) {
        // Path source does not display status
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
/// use other methods like `.gitignore`, `package.include`, or
/// `package.exclude` to filter the list of files.
pub fn list_files(pkg: &Package, gctx: &GlobalContext) -> CargoResult<Vec<PathBuf>> {
    _list_files(pkg, gctx).with_context(|| {
        format!(
            "failed to determine list of files in {}",
            pkg.root().display()
        )
    })
}

/// See [`PathSource::list_files`].
fn _list_files(pkg: &Package, gctx: &GlobalContext) -> CargoResult<Vec<PathBuf>> {
    let root = pkg.root();
    let no_include_option = pkg.manifest().include().is_empty();
    let git_repo = if no_include_option {
        discover_gix_repo(root)?
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

    let ignore_should_package = |relative_path: &Path, is_dir: bool| {
        // "Include" and "exclude" options are mutually exclusive.
        if no_include_option {
            !ignore_exclude
                .matched_path_or_any_parents(relative_path, is_dir)
                .is_ignore()
        } else {
            if is_dir {
                // Generally, include directives don't list every
                // directory (nor should they!). Just skip all directory
                // checks, and only check files.
                return true;
            }
            ignore_include
                .matched_path_or_any_parents(relative_path, /* is_dir */ false)
                .is_ignore()
        }
    };

    let filter = |path: &Path, is_dir: bool| {
        let Ok(relative_path) = path.strip_prefix(root) else {
            return false;
        };

        let rel = relative_path.as_os_str();
        if rel == "Cargo.lock" {
            return pkg.include_lockfile();
        } else if rel == "Cargo.toml" {
            return true;
        }

        ignore_should_package(relative_path, is_dir)
    };

    // Attempt Git-prepopulate only if no `include` (see rust-lang/cargo#4135).
    if no_include_option {
        if let Some(repo) = git_repo {
            return list_files_gix(pkg, &repo, &filter, gctx);
        }
    }
    list_files_walk(pkg, &filter, gctx)
}

/// Returns [`Some(gix::Repository)`](gix::Repository) if the discovered repository
/// (searched upwards from `root`) contains a tracked `<root>/Cargo.toml`.
/// Otherwise, the caller should fall back on full file list.
fn discover_gix_repo(root: &Path) -> CargoResult<Option<gix::Repository>> {
    let repo = match gix::ThreadSafeRepository::discover(root) {
        Ok(repo) => repo.to_thread_local(),
        Err(e) => {
            tracing::debug!(
                "could not discover git repo at or above {}: {}",
                root.display(),
                e
            );
            return Ok(None);
        }
    };
    let index = repo
        .index_or_empty()
        .with_context(|| format!("failed to open git index at {}", repo.path().display()))?;
    let repo_root = repo.work_dir().ok_or_else(|| {
        anyhow::format_err!(
            "did not expect repo at {} to be bare",
            repo.path().display()
        )
    })?;
    let repo_relative_path = match paths::strip_prefix_canonical(root, repo_root) {
        Ok(p) => p,
        Err(e) => {
            warn!(
                "cannot determine if path `{:?}` is in git repo `{:?}`: {:?}",
                root, repo_root, e
            );
            return Ok(None);
        }
    };
    let manifest_path = gix::path::join_bstr_unix_pathsep(
        gix::path::to_unix_separators_on_windows(gix::path::into_bstr(repo_relative_path)),
        "Cargo.toml",
    );
    if index.entry_index_by_path(&manifest_path).is_ok() {
        return Ok(Some(repo));
    }
    // Package Cargo.toml is not in git, don't use git to guide our selection.
    Ok(None)
}

/// Lists files relevant to building this package inside this source by
/// traversing the git working tree, while avoiding ignored files.
///
/// This looks into Git sub-repositories as well, resolving them to individual files.
/// Symlinks to directories will also be resolved, but walked as repositories if they
/// point to one to avoid picking up `.git` directories.
fn list_files_gix(
    pkg: &Package,
    repo: &gix::Repository,
    filter: &dyn Fn(&Path, bool) -> bool,
    gctx: &GlobalContext,
) -> CargoResult<Vec<PathBuf>> {
    debug!("list_files_gix {}", pkg.package_id());
    let options = repo
        .dirwalk_options()?
        .emit_untracked(gix::dir::walk::EmissionMode::Matching)
        .emit_ignored(None)
        .emit_tracked(true)
        .recurse_repositories(false)
        .symlinks_to_directories_are_ignored_like_directories(true)
        .emit_empty_directories(false);
    let index = repo.index_or_empty()?;
    let root = repo
        .work_dir()
        .ok_or_else(|| anyhow::format_err!("can't list files on a bare repository"))?;
    assert!(
        root.is_absolute(),
        "BUG: paths used internally are absolute, and the repo inherits that"
    );

    let pkg_path = pkg.root();
    let repo_relative_pkg_path = pkg_path.strip_prefix(root).unwrap_or(Path::new(""));
    let target_prefix = gix::path::to_unix_separators_on_windows(gix::path::into_bstr(
        repo_relative_pkg_path.join("target/"),
    ));
    let package_prefix =
        gix::path::to_unix_separators_on_windows(gix::path::into_bstr(repo_relative_pkg_path));

    let pathspec = {
        // Include the package root.
        let mut include = BString::from(":/");
        include.push_str(package_prefix.as_ref());

        // Exclude the target directory.
        let mut exclude = BString::from(":!/");
        exclude.push_str(target_prefix.as_ref());

        vec![include, exclude]
    };

    let mut files = Vec::<PathBuf>::new();
    let mut subpackages_found = Vec::new();
    for item in repo
        .dirwalk_iter(index.clone(), pathspec, Default::default(), options)?
        .filter(|res| {
            // Don't include Cargo.lock if it is untracked. Packaging will
            // generate a new one as needed.
            res.as_ref().map_or(true, |item| {
                !(item.entry.status == Status::Untracked && item.entry.rela_path == "Cargo.lock")
            })
        })
        .map(|res| res.map(|item| (item.entry.rela_path, item.entry.disk_kind)))
        .chain(
            // Append entries that might be tracked in `<pkg_root>/target/`.
            index
                .prefixed_entries(target_prefix.as_ref())
                .unwrap_or_default()
                .iter()
                .filter(|entry| {
                    // probably not needed as conflicts prevent this to run, but let's be explicit.
                    entry.stage() == Stage::Unconflicted
                })
                .map(|entry| {
                    (
                        entry.path(&index).to_owned(),
                        // Do not trust what's recorded in the index, enforce checking the disk.
                        // This traversal is not part of a `status()`, and tracking things in `target/`
                        // is rare.
                        None,
                    )
                })
                .map(Ok),
        )
    {
        let (rela_path, kind) = item?;
        let file_path = root.join(gix::path::from_bstr(rela_path));
        if file_path.file_name().and_then(|name| name.to_str()) == Some("Cargo.toml") {
            // Keep track of all sub-packages found and also strip out all
            // matches we've found so far. Note, though, that if we find
            // our own `Cargo.toml`, we keep going.
            let path = file_path.parent().unwrap();
            if path != pkg_path {
                debug!("subpackage found: {}", path.display());
                files.retain(|p| !p.starts_with(path));
                subpackages_found.push(path.to_path_buf());
                continue;
            }
        }

        // If this file is part of any other sub-package we've found so far,
        // skip it.
        if subpackages_found.iter().any(|p| file_path.starts_with(p)) {
            continue;
        }

        let is_dir = kind.map_or(false, |kind| {
            if kind == gix::dir::entry::Kind::Symlink {
                // Symlinks must be checked to see if they point to a directory
                // we should traverse.
                file_path.is_dir()
            } else {
                kind.is_dir()
            }
        });
        if is_dir {
            // This could be a submodule, or a sub-repository. In any case, we prefer to walk
            // it with git-support to leverage ignored files and to avoid pulling in entire
            // .git repositories.
            match gix::open(&file_path) {
                Ok(sub_repo) => {
                    files.extend(list_files_gix(pkg, &sub_repo, filter, gctx)?);
                }
                Err(_) => {
                    walk(&file_path, &mut files, false, filter, gctx)?;
                }
            }
        } else if (filter)(&file_path, is_dir) {
            assert!(!is_dir);
            trace!("  found {}", file_path.display());
            files.push(file_path);
        }
    }

    return Ok(files);
}

/// Lists files relevant to building this package inside this source by
/// walking the filesystem from the package root path.
///
/// This is a fallback for [`list_files_gix`] when the package
/// is not tracked under a Git repository.
fn list_files_walk(
    pkg: &Package,
    filter: &dyn Fn(&Path, bool) -> bool,
    gctx: &GlobalContext,
) -> CargoResult<Vec<PathBuf>> {
    let mut ret = Vec::new();
    walk(pkg.root(), &mut ret, true, filter, gctx)?;
    Ok(ret)
}

/// Helper recursive function for [`list_files_walk`].
fn walk(
    path: &Path,
    ret: &mut Vec<PathBuf>,
    is_root: bool,
    filter: &dyn Fn(&Path, bool) -> bool,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let walkdir = WalkDir::new(path)
        .follow_links(true)
        .into_iter()
        .filter_entry(|entry| {
            let path = entry.path();
            let at_root = is_root && entry.depth() == 0;
            let is_dir = entry.file_type().is_dir();

            if !at_root && !filter(path, is_dir) {
                return false;
            }

            if !is_dir {
                return true;
            }

            // Don't recurse into any sub-packages that we have.
            if !at_root && path.join("Cargo.toml").exists() {
                return false;
            }

            // Skip root Cargo artifacts.
            if is_root
                && entry.depth() == 1
                && path.file_name().and_then(|s| s.to_str()) == Some("target")
            {
                return false;
            }

            true
        });
    for entry in walkdir {
        match entry {
            Ok(entry) => {
                if !entry.file_type().is_dir() {
                    ret.push(entry.into_path());
                }
            }
            Err(err) if err.loop_ancestor().is_some() => {
                gctx.shell().warn(err)?;
            }
            Err(err) => match err.path() {
                // If an error occurs with a path, filter it again.
                // If it is excluded, Just ignore it in this case.
                // See issue rust-lang/cargo#10917
                Some(path) if !filter(path, path.is_dir()) => {}
                // Otherwise, simply recover from it.
                // Don't worry about error skipping here, the callers would
                // still hit the IO error if they do access it thereafter.
                Some(path) => ret.push(path.to_path_buf()),
                None => return Err(err.into()),
            },
        }
    }

    Ok(())
}

/// Gets the last modified file in a package.
fn last_modified_file(
    path: &Path,
    pkg: &Package,
    gctx: &GlobalContext,
) -> CargoResult<(FileTime, PathBuf)> {
    let mut max = FileTime::zero();
    let mut max_path = PathBuf::new();
    for file in list_files(pkg, gctx).with_context(|| {
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
    trace!("last modified file {}: {}", path.display(), max);
    Ok((max, max_path))
}
