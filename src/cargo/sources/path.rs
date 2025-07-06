use std::collections::{HashMap, HashSet};
use std::fmt::{self, Debug, Formatter};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::task::Poll;

use crate::core::{Dependency, EitherManifest, Manifest, Package, PackageId, SourceId};
use crate::ops;
use crate::sources::IndexSummary;
use crate::sources::source::MaybePackage;
use crate::sources::source::QueryKind;
use crate::sources::source::Source;
use crate::util::GlobalContext;
use crate::util::errors::CargoResult;
use crate::util::important_paths::find_project_manifest_exact;
use crate::util::internal;
use crate::util::toml::read_manifest;
use anyhow::Context as _;
use cargo_util::paths;
use filetime::FileTime;
use gix::bstr::{BString, ByteVec};
use gix::dir::entry::Status;
use gix::index::entry::Stage;
use ignore::gitignore::GitignoreBuilder;
use tracing::{debug, info, trace, warn};
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
            package: Some(pkg),
            gctx,
        }
    }

    /// Gets the package on the root path.
    pub fn root_package(&mut self) -> CargoResult<Package> {
        trace!("root_package; source={:?}", self);

        self.load()?;

        match &self.package {
            Some(pkg) => Ok(pkg.clone()),
            None => Err(internal(format!(
                "no package found in source {:?}",
                self.path
            ))),
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
    #[tracing::instrument(skip_all)]
    pub fn list_files(&self, pkg: &Package) -> CargoResult<Vec<PathEntry>> {
        list_files(pkg, self.gctx)
    }

    /// Gets the last modified file in a package.
    fn last_modified_file(&self, pkg: &Package) -> CargoResult<(FileTime, PathBuf)> {
        if self.package.is_none() {
            return Err(internal(format!(
                "BUG: source `{:?}` was not loaded",
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
    pub fn load(&mut self) -> CargoResult<()> {
        if self.package.is_none() {
            self.package = Some(self.read_package()?);
        }

        Ok(())
    }

    fn read_package(&self) -> CargoResult<Package> {
        let path = self.path.join("Cargo.toml");
        let pkg = ops::read_package(&path, self.source_id, self.gctx)?;
        Ok(pkg)
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
        self.load()?;
        if let Some(s) = self.package.as_ref().map(|p| p.summary()) {
            let matched = match kind {
                QueryKind::Exact | QueryKind::RejectedVersions => dep.matches(s),
                QueryKind::AlternativeNames => true,
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
        self.load()?;
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
        self.load()
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
    /// Whether this source has loaded all package information it may contain.
    loaded: bool,
    /// Packages that this sources has discovered.
    ///
    /// Tracking all packages for a given ID to warn on-demand for unused packages
    packages: HashMap<PackageId, Vec<Package>>,
    /// Avoid redundant unused package warnings
    warned_duplicate: HashSet<PackageId>,
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
            loaded: false,
            packages: Default::default(),
            warned_duplicate: Default::default(),
            gctx,
        }
    }

    /// Returns the packages discovered by this source. It may walk the
    /// filesystem if package information haven't yet loaded.
    pub fn read_packages(&mut self) -> CargoResult<Vec<Package>> {
        self.load()?;
        Ok(self
            .packages
            .iter()
            .map(|(pkg_id, v)| {
                first_package(*pkg_id, v, &mut self.warned_duplicate, self.gctx).clone()
            })
            .collect())
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
    pub fn list_files(&self, pkg: &Package) -> CargoResult<Vec<PathEntry>> {
        list_files(pkg, self.gctx)
    }

    /// Gets the last modified file in a package.
    fn last_modified_file(&self, pkg: &Package) -> CargoResult<(FileTime, PathBuf)> {
        if !self.loaded {
            return Err(internal(format!(
                "BUG: source `{:?}` was not loaded",
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
    pub fn load(&mut self) -> CargoResult<()> {
        if !self.loaded {
            self.packages = read_packages(&self.path, self.source_id, self.gctx)?;
            self.loaded = true;
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
        self.load()?;
        for s in self
            .packages
            .iter()
            .filter(|(pkg_id, _)| pkg_id.name() == dep.package_name())
            .map(|(pkg_id, pkgs)| {
                first_package(*pkg_id, pkgs, &mut self.warned_duplicate, self.gctx)
            })
            .map(|p| p.summary())
        {
            let matched = match kind {
                QueryKind::Exact | QueryKind::RejectedVersions => dep.matches(s),
                QueryKind::AlternativeNames => true,
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
        self.load()?;
        let pkg = self.packages.get(&id);
        pkg.map(|pkgs| first_package(id, pkgs, &mut self.warned_duplicate, self.gctx).clone())
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
        self.load()
    }

    fn invalidate_cache(&mut self) {
        // Path source has no local cache.
    }

    fn set_quiet(&mut self, _quiet: bool) {
        // Path source does not display status
    }
}

/// Type that abstracts over [`gix::dir::entry::Kind`] and [`fs::FileType`].
#[derive(Debug, Clone, Copy)]
enum FileType {
    File { maybe_symlink: bool },
    Dir,
    Symlink,
    Other,
}

impl From<fs::FileType> for FileType {
    fn from(value: fs::FileType) -> Self {
        if value.is_file() {
            FileType::File {
                maybe_symlink: false,
            }
        } else if value.is_dir() {
            FileType::Dir
        } else if value.is_symlink() {
            FileType::Symlink
        } else {
            FileType::Other
        }
    }
}

impl From<gix::dir::entry::Kind> for FileType {
    fn from(value: gix::dir::entry::Kind) -> Self {
        use gix::dir::entry::Kind;
        match value {
            Kind::Untrackable => FileType::Other,
            Kind::File => FileType::File {
                maybe_symlink: false,
            },
            Kind::Symlink => FileType::Symlink,
            Kind::Directory | Kind::Repository => FileType::Dir,
        }
    }
}

/// [`PathBuf`] with extra metadata.
#[derive(Clone, Debug)]
pub struct PathEntry {
    path: PathBuf,
    ty: FileType,
    /// Whether this path was visited when traversing a symlink directory.
    under_symlink_dir: bool,
}

impl PathEntry {
    pub fn into_path_buf(self) -> PathBuf {
        self.path
    }

    /// Similar to [`std::path::Path::is_file`]
    /// but doesn't follow the symbolic link nor make any system call
    pub fn is_file(&self) -> bool {
        matches!(self.ty, FileType::File { .. })
    }

    /// Similar to [`std::path::Path::is_dir`]
    /// but doesn't follow the symbolic link nor make any system call
    pub fn is_dir(&self) -> bool {
        matches!(self.ty, FileType::Dir)
    }

    /// Similar to [`std::path::Path::is_symlink`]
    /// but doesn't follow the symbolic link nor make any system call
    ///
    /// If the path is not a symlink but under a symlink parent directory,
    /// this will return false.
    /// See [`PathEntry::is_symlink_or_under_symlink`] for an alternative.
    pub fn is_symlink(&self) -> bool {
        matches!(self.ty, FileType::Symlink)
    }

    /// Whether a path is a symlink or a path under a symlink directory.
    ///
    /// Use [`PathEntry::is_symlink`] to get the exact file type of the path only.
    pub fn is_symlink_or_under_symlink(&self) -> bool {
        self.is_symlink() || self.under_symlink_dir
    }

    /// Whether this path might be a plain text symlink.
    ///
    /// Git may check out symlinks as plain text files that contain the link texts,
    /// when either `core.symlinks` is `false`, or on Windows.
    pub fn maybe_plain_text_symlink(&self) -> bool {
        matches!(
            self.ty,
            FileType::File {
                maybe_symlink: true
            }
        )
    }
}

impl std::ops::Deref for PathEntry {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        self.path.as_path()
    }
}

impl AsRef<PathBuf> for PathEntry {
    fn as_ref(&self) -> &PathBuf {
        &self.path
    }
}

fn first_package<'p>(
    pkg_id: PackageId,
    pkgs: &'p Vec<Package>,
    warned_duplicate: &mut HashSet<PackageId>,
    gctx: &GlobalContext,
) -> &'p Package {
    if pkgs.len() != 1 && warned_duplicate.insert(pkg_id) {
        let ignored = pkgs[1..]
            .iter()
            // We can assume a package with publish = false isn't intended to be seen
            // by users so we can hide the warning about those since the user is unlikely
            // to care about those cases.
            .filter(|pkg| pkg.publish().is_none())
            .collect::<Vec<_>>();
        if !ignored.is_empty() {
            use std::fmt::Write as _;

            let plural = if ignored.len() == 1 { "" } else { "s" };
            let mut msg = String::new();
            let _ = writeln!(&mut msg, "skipping duplicate package{plural} `{pkg_id}`:");
            for ignored in ignored {
                let manifest_path = ignored.manifest_path().display();
                let _ = writeln!(&mut msg, "  {manifest_path}");
            }
            let manifest_path = pkgs[0].manifest_path().display();
            let _ = writeln!(&mut msg, "in favor of {manifest_path}");
            let _ = gctx.shell().warn(msg);
        }
    }
    &pkgs[0]
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
pub fn list_files(pkg: &Package, gctx: &GlobalContext) -> CargoResult<Vec<PathEntry>> {
    _list_files(pkg, gctx).with_context(|| {
        format!(
            "failed to determine list of files in {}",
            pkg.root().display()
        )
    })
}

/// See [`PathSource::list_files`].
fn _list_files(pkg: &Package, gctx: &GlobalContext) -> CargoResult<Vec<PathEntry>> {
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
        if rel == "Cargo.lock" || rel == "Cargo.toml" {
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
    let mut ret = Vec::new();
    list_files_walk(pkg.root(), &mut ret, true, &filter, gctx)?;
    Ok(ret)
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
    let repo_root = repo.workdir().ok_or_else(|| {
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
) -> CargoResult<Vec<PathEntry>> {
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
        .workdir()
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
        let mut include = BString::from(":(top)");
        include.push_str(package_prefix.as_ref());

        // Exclude the target directory.
        let mut exclude = BString::from(":!(exclude,top)");
        exclude.push_str(target_prefix.as_ref());

        vec![include, exclude]
    };

    let mut files = Vec::<PathEntry>::new();
    let mut subpackages_found = Vec::new();
    for item in repo
        .dirwalk_iter(index.clone(), pathspec, Default::default(), options)?
        .filter(|res| {
            // Don't include Cargo.lock if it is untracked. Packaging will
            // generate a new one as needed.
            // Also don't include untrackable directory entries, like FIFOs.
            res.as_ref().map_or(true, |item| {
                item.entry.disk_kind != Some(gix::dir::entry::Kind::Untrackable)
                    && !(item.entry.status == Status::Untracked
                        && item.entry.rela_path == "Cargo.lock")
            })
        })
        .map(|res| {
            res.map(|item| {
                // Assumption: if a file tracked as a symlink in Git index, and
                // the actual file type on disk is file, then it might be a
                // plain text file symlink.
                // There are exceptions like the file has changed from a symlink
                // to a real text file, but hasn't been committed to Git index.
                // Exceptions may be rare so we're okay with this now.
                let maybe_plain_text_symlink = item.entry.index_kind
                    == Some(gix::dir::entry::Kind::Symlink)
                    && item.entry.disk_kind == Some(gix::dir::entry::Kind::File);
                (
                    item.entry.rela_path,
                    item.entry.disk_kind,
                    maybe_plain_text_symlink,
                )
            })
        })
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
                        false,
                    )
                })
                .map(Ok),
        )
    {
        let (rela_path, kind, maybe_plain_text_symlink) = item?;
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
                    list_files_walk(&file_path, &mut files, false, filter, gctx)?;
                }
            }
        } else if (filter)(&file_path, is_dir) {
            assert!(!is_dir);
            trace!("  found {}", file_path.display());
            let ty = match kind.map(Into::into) {
                Some(FileType::File { .. }) => FileType::File {
                    maybe_symlink: maybe_plain_text_symlink,
                },
                Some(ty) => ty,
                None => FileType::Other,
            };
            files.push(PathEntry {
                path: file_path,
                ty,
                // Git index doesn't include files from symlink directory,
                // symlink dirs are handled in `list_files_walk`.
                under_symlink_dir: false,
            });
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
    path: &Path,
    ret: &mut Vec<PathEntry>,
    is_root: bool,
    filter: &dyn Fn(&Path, bool) -> bool,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let walkdir = WalkDir::new(path)
        .follow_links(true)
        // While this is the default, set it explicitly.
        // We need walkdir to visit the directory tree in depth-first order,
        // so we can ensure a path visited later be under a certain directory.
        .contents_first(false)
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

    let mut current_symlink_dir = None;
    for entry in walkdir {
        match entry {
            Ok(entry) => {
                let file_type = entry.file_type();

                match current_symlink_dir.as_ref() {
                    Some(dir) if entry.path().starts_with(dir) => {
                        // Still walk under the same parent symlink dir, so keep it
                    }
                    Some(_) | None => {
                        // Not under any parent symlink dir, update the current one.
                        current_symlink_dir = if file_type.is_dir() && entry.path_is_symlink() {
                            Some(entry.path().to_path_buf())
                        } else {
                            None
                        };
                    }
                }

                if file_type.is_file() || file_type.is_symlink() {
                    // We follow_links(true) here so check if entry was created from a symlink
                    let ty = if entry.path_is_symlink() {
                        FileType::Symlink
                    } else {
                        file_type.into()
                    };
                    ret.push(PathEntry {
                        path: entry.into_path(),
                        ty,
                        // This rely on contents_first(false), which walks in depth-first order
                        under_symlink_dir: current_symlink_dir.is_some(),
                    });
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
                Some(path) => ret.push(PathEntry {
                    path: path.to_path_buf(),
                    ty: FileType::Other,
                    under_symlink_dir: false,
                }),
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
            max_path = file.into_path_buf();
        }
    }
    trace!("last modified file {}: {}", path.display(), max);
    Ok((max, max_path))
}

fn read_packages(
    path: &Path,
    source_id: SourceId,
    gctx: &GlobalContext,
) -> CargoResult<HashMap<PackageId, Vec<Package>>> {
    let mut all_packages = HashMap::new();
    let mut visited = HashSet::<PathBuf>::new();
    let mut errors = Vec::<anyhow::Error>::new();

    trace!(
        "looking for root package: {}, source_id={}",
        path.display(),
        source_id
    );

    walk(path, &mut |dir| {
        trace!("looking for child package: {}", dir.display());

        // Don't recurse into hidden/dot directories unless we're at the toplevel
        if dir != path {
            let name = dir.file_name().and_then(|s| s.to_str());
            if name.map(|s| s.starts_with('.')) == Some(true) {
                return Ok(false);
            }

            // Don't automatically discover packages across git submodules
            if dir.join(".git").exists() {
                return Ok(false);
            }
        }

        // Don't ever look at target directories
        if dir.file_name().and_then(|s| s.to_str()) == Some("target")
            && has_manifest(dir.parent().unwrap())
        {
            return Ok(false);
        }

        if has_manifest(dir) {
            read_nested_packages(
                dir,
                &mut all_packages,
                source_id,
                gctx,
                &mut visited,
                &mut errors,
            )?;
        }
        Ok(true)
    })?;

    if all_packages.is_empty() {
        match errors.pop() {
            Some(err) => Err(err),
            None => {
                if find_project_manifest_exact(path, "cargo.toml").is_ok() {
                    Err(anyhow::format_err!(
                        "Could not find Cargo.toml in `{}`, but found cargo.toml please try to rename it to Cargo.toml",
                        path.display()
                    ))
                } else {
                    Err(anyhow::format_err!(
                        "Could not find Cargo.toml in `{}`",
                        path.display()
                    ))
                }
            }
        }
    } else {
        Ok(all_packages)
    }
}

fn nested_paths(manifest: &Manifest) -> Vec<PathBuf> {
    let mut nested_paths = Vec::new();
    let normalized = manifest.normalized_toml();
    let dependencies = normalized
        .dependencies
        .iter()
        .chain(normalized.build_dependencies())
        .chain(normalized.dev_dependencies())
        .chain(
            normalized
                .target
                .as_ref()
                .into_iter()
                .flat_map(|t| t.values())
                .flat_map(|t| {
                    t.dependencies
                        .iter()
                        .chain(t.build_dependencies())
                        .chain(t.dev_dependencies())
                }),
        );
    for dep_table in dependencies {
        for dep in dep_table.values() {
            let cargo_util_schemas::manifest::InheritableDependency::Value(dep) = dep else {
                continue;
            };
            let cargo_util_schemas::manifest::TomlDependency::Detailed(dep) = dep else {
                continue;
            };
            let Some(path) = dep.path.as_ref() else {
                continue;
            };
            nested_paths.push(PathBuf::from(path.as_str()));
        }
    }
    nested_paths
}

fn walk(path: &Path, callback: &mut dyn FnMut(&Path) -> CargoResult<bool>) -> CargoResult<()> {
    if !callback(path)? {
        trace!("not processing {}", path.display());
        return Ok(());
    }

    // Ignore any permission denied errors because temporary directories
    // can often have some weird permissions on them.
    let dirs = match fs::read_dir(path) {
        Ok(dirs) => dirs,
        Err(ref e) if e.kind() == io::ErrorKind::PermissionDenied => return Ok(()),
        Err(e) => {
            let cx = format!("failed to read directory `{}`", path.display());
            let e = anyhow::Error::from(e);
            return Err(e.context(cx));
        }
    };
    let mut dirs = dirs.collect::<Vec<_>>();
    dirs.sort_unstable_by_key(|d| d.as_ref().ok().map(|d| d.file_name()));
    for dir in dirs {
        let dir = dir?;
        if dir.file_type()?.is_dir() {
            walk(&dir.path(), callback)?;
        }
    }
    Ok(())
}

fn has_manifest(path: &Path) -> bool {
    find_project_manifest_exact(path, "Cargo.toml").is_ok()
}

fn read_nested_packages(
    path: &Path,
    all_packages: &mut HashMap<PackageId, Vec<Package>>,
    source_id: SourceId,
    gctx: &GlobalContext,
    visited: &mut HashSet<PathBuf>,
    errors: &mut Vec<anyhow::Error>,
) -> CargoResult<()> {
    if !visited.insert(path.to_path_buf()) {
        return Ok(());
    }

    let manifest_path = find_project_manifest_exact(path, "Cargo.toml")?;

    let manifest = match read_manifest(&manifest_path, source_id, gctx) {
        Err(err) => {
            // Ignore malformed manifests found on git repositories
            //
            // git source try to find and read all manifests from the repository
            // but since it's not possible to exclude folders from this search
            // it's safer to ignore malformed manifests to avoid
            //
            // TODO: Add a way to exclude folders?
            info!(
                "skipping malformed package found at `{}`",
                path.to_string_lossy()
            );
            errors.push(err.into());
            return Ok(());
        }
        Ok(tuple) => tuple,
    };

    let manifest = match manifest {
        EitherManifest::Real(manifest) => manifest,
        EitherManifest::Virtual(..) => return Ok(()),
    };
    let nested = nested_paths(&manifest);
    let pkg = Package::new(manifest, &manifest_path);

    let pkg_id = pkg.package_id();
    all_packages.entry(pkg_id).or_default().push(pkg);

    // Registry sources are not allowed to have `path=` dependencies because
    // they're all translated to actual registry dependencies.
    //
    // We normalize the path here ensure that we don't infinitely walk around
    // looking for crates. By normalizing we ensure that we visit this crate at
    // most once.
    //
    // TODO: filesystem/symlink implications?
    if !source_id.is_registry() {
        for p in nested.iter() {
            let path = paths::normalize_path(&path.join(p));
            let result =
                read_nested_packages(&path, all_packages, source_id, gctx, visited, errors);
            // Ignore broken manifests found on git repositories.
            //
            // A well formed manifest might still fail to load due to reasons
            // like referring to a "path" that requires an extra build step.
            //
            // See https://github.com/rust-lang/cargo/issues/6822.
            if let Err(err) = result {
                if source_id.is_git() {
                    info!(
                        "skipping nested package found at `{}`: {:?}",
                        path.display(),
                        &err,
                    );
                    errors.push(err);
                } else {
                    return Err(err);
                }
            }
        }
    }

    Ok(())
}
