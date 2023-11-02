//! Access to a Git index based registry. See [`RemoteRegistry`] for details.

use crate::core::{GitReference, PackageId, SourceId};
use crate::sources::git;
use crate::sources::git::fetch::RemoteKind;
use crate::sources::registry::download;
use crate::sources::registry::MaybeLock;
use crate::sources::registry::{LoadResponse, RegistryConfig, RegistryData};
use crate::util::cache_lock::CacheLockMode;
use crate::util::errors::CargoResult;
use crate::util::interning::InternedString;
use crate::util::{Config, Filesystem};
use anyhow::Context as _;
use cargo_util::paths;
use lazycell::LazyCell;
use std::cell::{Cell, Ref, RefCell};
use std::fs::File;
use std::mem;
use std::path::Path;
use std::str;
use std::task::{ready, Poll};
use tracing::{debug, trace};

/// A remote registry is a registry that lives at a remote URL (such as
/// crates.io). The git index is cloned locally, and `.crate` files are
/// downloaded as needed and cached locally.
///
/// This type is primarily accessed through the [`RegistryData`] trait.
///
/// See the [module-level documentation](super) for the index format and layout.
///
/// ## History of Git-based index registry
///
/// Using Git to host this index used to be quite efficient. The full index can
/// be stored efficiently locally on disk, and once it is downloaded, all
/// queries of a registry can happen locally and needn't touch the network.
/// Git-based index was a reasonable design choice at the time when HTTP/2
/// was just introduced.
///
/// However, the full index keeps growing as crates.io grows. It becomes
/// relatively big and slows down the first use of Cargo. Git (specifically
/// libgit2) is not efficient at handling huge amounts of small files either.
/// On the other hand, newer protocols like HTTP/2 are prevalent and capable to
/// serve a bunch of tiny files. Today, it is encouraged to use [`HttpRegistry`],
/// which is the default from 1.70.0. That being said, Cargo will continue
/// supporting Git-based index for a pretty long while.
///
/// [`HttpRegistry`]: super::http_remote::HttpRegistry
pub struct RemoteRegistry<'cfg> {
    /// Path to the registry index (`$CARGO_HOME/registry/index/$REG-HASH`).
    index_path: Filesystem,
    /// Path to the cache of `.crate` files (`$CARGO_HOME/registry/cache/$REG-HASH`).
    cache_path: Filesystem,
    /// The unique identifier of this registry source.
    source_id: SourceId,
    /// This reference is stored so that when a registry needs update, it knows
    /// where to fetch from.
    index_git_ref: GitReference,
    config: &'cfg Config,
    /// A Git [tree object] to help this registry find crate metadata from the
    /// underlying Git repository.
    ///
    /// This is stored here to prevent Git from repeatedly creating a tree object
    /// during each call into `load()`.
    ///
    /// [tree object]: https://git-scm.com/book/en/v2/Git-Internals-Git-Objects#_tree_objects
    tree: RefCell<Option<git2::Tree<'static>>>,
    /// A Git repository that contains the actual index we want.
    repo: LazyCell<git2::Repository>,
    /// The current HEAD commit of the underlying Git repository.
    head: Cell<Option<git2::Oid>>,
    /// This stores sha value of the current HEAD commit for convenience.
    current_sha: Cell<Option<InternedString>>,
    /// Whether this registry needs to update package information.
    ///
    /// See [`RemoteRegistry::mark_updated`] on how to make sure a registry
    /// index is updated only once per session.
    needs_update: bool,
    /// Disables status messages.
    quiet: bool,
}

impl<'cfg> RemoteRegistry<'cfg> {
    /// Creates a Git-rebased remote registry for `source_id`.
    ///
    /// * `name` --- Name of a path segment where `.crate` tarballs and the
    ///   registry index are stored. Expect to be unique.
    pub fn new(source_id: SourceId, config: &'cfg Config, name: &str) -> RemoteRegistry<'cfg> {
        RemoteRegistry {
            index_path: config.registry_index_path().join(name),
            cache_path: config.registry_cache_path().join(name),
            source_id,
            config,
            index_git_ref: GitReference::DefaultBranch,
            tree: RefCell::new(None),
            repo: LazyCell::new(),
            head: Cell::new(None),
            current_sha: Cell::new(None),
            needs_update: false,
            quiet: false,
        }
    }

    /// Creates intermediate dirs and initialize the repository.
    fn repo(&self) -> CargoResult<&git2::Repository> {
        self.repo.try_borrow_with(|| {
            trace!("acquiring registry index lock");
            let path = self
                .config
                .assert_package_cache_locked(CacheLockMode::DownloadExclusive, &self.index_path);

            match git2::Repository::open(&path) {
                Ok(repo) => Ok(repo),
                Err(_) => {
                    drop(paths::remove_dir_all(&path));
                    paths::create_dir_all(&path)?;

                    // Note that we'd actually prefer to use a bare repository
                    // here as we're not actually going to check anything out.
                    // All versions of Cargo, though, share the same CARGO_HOME,
                    // so for compatibility with older Cargo which *does* do
                    // checkouts we make sure to initialize a new full
                    // repository (not a bare one).
                    //
                    // We should change this to `init_bare` whenever we feel
                    // like enough time has passed or if we change the directory
                    // that the folder is located in, such as by changing the
                    // hash at the end of the directory.
                    //
                    // Note that in the meantime we also skip `init.templatedir`
                    // as it can be misconfigured sometimes or otherwise add
                    // things that we don't want.
                    let mut opts = git2::RepositoryInitOptions::new();
                    opts.external_template(false);
                    Ok(git2::Repository::init_opts(&path, &opts).with_context(|| {
                        format!("failed to initialize index git repository (in {:?})", path)
                    })?)
                }
            }
        })
    }

    /// Get the object ID of the HEAD commit from the underlying Git repository.
    fn head(&self) -> CargoResult<git2::Oid> {
        if self.head.get().is_none() {
            let repo = self.repo()?;
            let oid = self.index_git_ref.resolve(repo)?;
            self.head.set(Some(oid));
        }
        Ok(self.head.get().unwrap())
    }

    /// Returns a [`git2::Tree`] object of the current HEAD commit of the
    /// underlying Git repository.
    fn tree(&self) -> CargoResult<Ref<'_, git2::Tree<'_>>> {
        {
            let tree = self.tree.borrow();
            if tree.is_some() {
                return Ok(Ref::map(tree, |s| s.as_ref().unwrap()));
            }
        }
        let repo = self.repo()?;
        let commit = repo.find_commit(self.head()?)?;
        let tree = commit.tree()?;

        // SAFETY:
        // Unfortunately in libgit2 the tree objects look like they've got a
        // reference to the repository object which means that a tree cannot
        // outlive the repository that it came from. Here we want to cache this
        // tree, though, so to accomplish this we transmute it to a static
        // lifetime.
        //
        // Note that we don't actually hand out the static lifetime, instead we
        // only return a scoped one from this function. Additionally the repo
        // we loaded from (above) lives as long as this object
        // (`RemoteRegistry`) so we then just need to ensure that the tree is
        // destroyed first in the destructor, hence the destructor on
        // `RemoteRegistry` below.
        let tree = unsafe { mem::transmute::<git2::Tree<'_>, git2::Tree<'static>>(tree) };
        *self.tree.borrow_mut() = Some(tree);
        Ok(Ref::map(self.tree.borrow(), |s| s.as_ref().unwrap()))
    }

    /// Gets the current version of the registry index.
    ///
    /// It is usually sha of the HEAD commit from the underlying Git repository.
    fn current_version(&self) -> Option<InternedString> {
        if let Some(sha) = self.current_sha.get() {
            return Some(sha);
        }
        let sha = InternedString::new(&self.head().ok()?.to_string());
        self.current_sha.set(Some(sha));
        Some(sha)
    }

    /// Whether the registry is up-to-date. See [`Self::mark_updated`] for more.
    fn is_updated(&self) -> bool {
        self.config.updated_sources().contains(&self.source_id)
    }

    /// Marks this registry as up-to-date.
    ///
    /// This makes sure the index is only updated once per session since it is
    /// an expensive operation. This generally only happens when the resolver
    /// is run multiple times, such as during `cargo publish`.
    fn mark_updated(&self) {
        self.config.updated_sources().insert(self.source_id);
    }
}

impl<'cfg> RegistryData for RemoteRegistry<'cfg> {
    fn prepare(&self) -> CargoResult<()> {
        self.repo()?;
        Ok(())
    }

    fn index_path(&self) -> &Filesystem {
        &self.index_path
    }

    fn assert_index_locked<'a>(&self, path: &'a Filesystem) -> &'a Path {
        self.config
            .assert_package_cache_locked(CacheLockMode::DownloadExclusive, path)
    }

    /// Read the general concept for `load()` on [`RegistryData::load`].
    ///
    /// `index_version` is a string representing the version of the file used
    /// to construct the cached copy.
    ///
    /// Older versions of Cargo used the single value of the hash of the HEAD
    /// commit as a `index_version`. This is technically correct but a little
    /// too conservative. If a new commit is fetched all cached files need to
    /// be regenerated even if a particular file was not changed.
    ///
    /// However if an old cargo has written such a file we still know how to
    /// read it, as long as we check for that hash value.
    ///
    /// Cargo now uses a hash of the file's contents as provided by git.
    fn load(
        &mut self,
        _root: &Path,
        path: &Path,
        index_version: Option<&str>,
    ) -> Poll<CargoResult<LoadResponse>> {
        if self.needs_update {
            return Poll::Pending;
        }
        // Check if the cache is valid.
        let git_commit_hash = self.current_version();
        if index_version.is_some() && index_version == git_commit_hash.as_deref() {
            // This file was written by an old version of cargo, but it is
            // still up-to-date.
            return Poll::Ready(Ok(LoadResponse::CacheValid));
        }
        // Note that the index calls this method and the filesystem is locked
        // in the index, so we don't need to worry about an `update_index`
        // happening in a different process.
        fn load_helper(
            registry: &RemoteRegistry<'_>,
            path: &Path,
            index_version: Option<&str>,
        ) -> CargoResult<LoadResponse> {
            let repo = registry.repo()?;
            let tree = registry.tree()?;
            let entry = tree.get_path(path);
            let entry = entry?;
            let git_file_hash = Some(entry.id().to_string());

            // Check if the cache is valid.
            if index_version.is_some() && index_version == git_file_hash.as_deref() {
                return Ok(LoadResponse::CacheValid);
            }

            let object = entry.to_object(repo)?;
            let Some(blob) = object.as_blob() else {
                anyhow::bail!("path `{}` is not a blob in the git repo", path.display())
            };

            Ok(LoadResponse::Data {
                raw_data: blob.content().to_vec(),
                index_version: git_file_hash,
            })
        }

        match load_helper(&self, path, index_version) {
            Ok(result) => Poll::Ready(Ok(result)),
            Err(_) if !self.is_updated() => {
                // If git returns an error and we haven't updated the repo,
                // return pending to allow an update to try again.
                self.needs_update = true;
                Poll::Pending
            }
            Err(e)
                if e.downcast_ref::<git2::Error>()
                    .map(|e| e.code() == git2::ErrorCode::NotFound)
                    .unwrap_or_default() =>
            {
                // The repo has been updated and the file does not exist.
                Poll::Ready(Ok(LoadResponse::NotFound))
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }

    fn config(&mut self) -> Poll<CargoResult<Option<RegistryConfig>>> {
        debug!("loading config");
        self.prepare()?;
        self.config
            .assert_package_cache_locked(CacheLockMode::DownloadExclusive, &self.index_path);
        match ready!(self.load(Path::new(""), Path::new(RegistryConfig::NAME), None)?) {
            LoadResponse::Data { raw_data, .. } => {
                trace!("config loaded");
                let cfg: RegistryConfig = serde_json::from_slice(&raw_data)?;
                Poll::Ready(Ok(Some(cfg)))
            }
            _ => Poll::Ready(Ok(None)),
        }
    }

    fn block_until_ready(&mut self) -> CargoResult<()> {
        if !self.needs_update {
            return Ok(());
        }

        self.needs_update = false;

        if self.is_updated() {
            return Ok(());
        }
        self.mark_updated();

        if self.config.offline() {
            return Ok(());
        }
        if self.config.cli_unstable().no_index_update {
            return Ok(());
        }

        debug!("updating the index");

        // Ensure that we'll actually be able to acquire an HTTP handle later on
        // once we start trying to download crates. This will weed out any
        // problems with `.cargo/config` configuration related to HTTP.
        //
        // This way if there's a problem the error gets printed before we even
        // hit the index, which may not actually read this configuration.
        self.config.http()?;

        self.prepare()?;
        self.head.set(None);
        *self.tree.borrow_mut() = None;
        self.current_sha.set(None);
        let _path = self
            .config
            .assert_package_cache_locked(CacheLockMode::DownloadExclusive, &self.index_path);
        if !self.quiet {
            self.config
                .shell()
                .status("Updating", self.source_id.display_index())?;
        }

        // Fetch the latest version of our `index_git_ref` into the index
        // checkout.
        let url = self.source_id.url();
        let repo = self.repo.borrow_mut().unwrap();
        git::fetch(
            repo,
            url.as_str(),
            &self.index_git_ref,
            self.config,
            RemoteKind::Registry,
        )
        .with_context(|| format!("failed to fetch `{}`", url))?;

        Ok(())
    }

    /// Read the general concept for `invalidate_cache()` on
    /// [`RegistryData::invalidate_cache`].
    ///
    /// To fully invalidate, undo [`RemoteRegistry::mark_updated`]'s work.
    fn invalidate_cache(&mut self) {
        self.needs_update = true;
    }

    fn set_quiet(&mut self, quiet: bool) {
        self.quiet = quiet;
    }

    fn is_updated(&self) -> bool {
        self.is_updated()
    }

    fn download(&mut self, pkg: PackageId, checksum: &str) -> CargoResult<MaybeLock> {
        let registry_config = loop {
            match self.config()? {
                Poll::Pending => self.block_until_ready()?,
                Poll::Ready(cfg) => break cfg.unwrap(),
            }
        };

        download::download(
            &self.cache_path,
            &self.config,
            pkg,
            checksum,
            registry_config,
        )
    }

    fn finish_download(
        &mut self,
        pkg: PackageId,
        checksum: &str,
        data: &[u8],
    ) -> CargoResult<File> {
        download::finish_download(&self.cache_path, &self.config, pkg, checksum, data)
    }

    fn is_crate_downloaded(&self, pkg: PackageId) -> bool {
        download::is_crate_downloaded(&self.cache_path, &self.config, pkg)
    }
}

/// Implemented to just be sure to drop `tree` field before our other fields.
/// See SAFETY inside [`RemoteRegistry::tree()`] for more.
impl<'cfg> Drop for RemoteRegistry<'cfg> {
    fn drop(&mut self) {
        self.tree.borrow_mut().take();
    }
}
