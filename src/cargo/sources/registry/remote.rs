use std::cell::{Cell, Ref, RefCell};
use std::fmt::Write as FmtWrite;
use std::io::prelude::*;
use std::io::SeekFrom;
use std::mem;
use std::path::Path;
use std::str;

use lazycell::LazyCell;
use log::{debug, trace};

use crate::core::{PackageId, SourceId};
use crate::sources::git;
use crate::sources::registry::MaybeLock;
use crate::sources::registry::{
    RegistryConfig, RegistryData, CRATE_TEMPLATE, INDEX_LOCK, VERSION_TEMPLATE,
};
use crate::util::errors::{CargoResult, CargoResultExt};
use crate::util::{Config, Sha256};
use crate::util::{FileLock, Filesystem};

pub struct RemoteRegistry<'cfg> {
    index_path: Filesystem,
    cache_path: Filesystem,
    source_id: SourceId,
    config: &'cfg Config,
    tree: RefCell<Option<git2::Tree<'static>>>,
    repo: LazyCell<git2::Repository>,
    head: Cell<Option<git2::Oid>>,
}

impl<'cfg> RemoteRegistry<'cfg> {
    pub fn new(source_id: SourceId, config: &'cfg Config, name: &str) -> RemoteRegistry<'cfg> {
        RemoteRegistry {
            index_path: config.registry_index_path().join(name),
            cache_path: config.registry_cache_path().join(name),
            source_id,
            config,
            tree: RefCell::new(None),
            repo: LazyCell::new(),
            head: Cell::new(None),
        }
    }

    fn repo(&self) -> CargoResult<&git2::Repository> {
        self.repo.try_borrow_with(|| {
            let path = self.index_path.clone().into_path_unlocked();

            // Fast path without a lock
            if let Ok(repo) = git2::Repository::open(&path) {
                trace!("opened a repo without a lock");
                return Ok(repo);
            }

            // Ok, now we need to lock and try the whole thing over again.
            trace!("acquiring registry index lock");
            let lock = self.index_path.open_rw(
                Path::new(INDEX_LOCK),
                self.config,
                "the registry index",
            )?;
            match git2::Repository::open(&path) {
                Ok(repo) => Ok(repo),
                Err(_) => {
                    let _ = lock.remove_siblings();

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
                    Ok(git2::Repository::init_opts(&path, &opts)
                        .chain_err(|| "failed to initialized index git repository")?)
                }
            }
        })
    }

    fn head(&self) -> CargoResult<git2::Oid> {
        if self.head.get().is_none() {
            let oid = self.repo()?.refname_to_id("refs/remotes/origin/master")?;
            self.head.set(Some(oid));
        }
        Ok(self.head.get().unwrap())
    }

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

    fn filename(&self, pkg: PackageId) -> String {
        format!("{}-{}.crate", pkg.name(), pkg.version())
    }
}

impl<'cfg> RegistryData for RemoteRegistry<'cfg> {
    fn prepare(&self) -> CargoResult<()> {
        self.repo()?; // create intermediate dirs and initialize the repo
        Ok(())
    }

    fn index_path(&self) -> &Filesystem {
        &self.index_path
    }

    fn load(
        &self,
        _root: &Path,
        path: &Path,
        data: &mut dyn FnMut(&[u8]) -> CargoResult<()>,
    ) -> CargoResult<()> {
        // Note that the index calls this method and the filesystem is locked
        // in the index, so we don't need to worry about an `update_index`
        // happening in a different process.
        let repo = self.repo()?;
        let tree = self.tree()?;
        let entry = tree.get_path(path)?;
        let object = entry.to_object(repo)?;
        let blob = match object.as_blob() {
            Some(blob) => blob,
            None => failure::bail!("path `{}` is not a blob in the git repo", path.display()),
        };
        data(blob.content())
    }

    fn config(&mut self) -> CargoResult<Option<RegistryConfig>> {
        debug!("loading config");
        self.prepare()?;
        let _lock =
            self.index_path
                .open_ro(Path::new(INDEX_LOCK), self.config, "the registry index")?;
        let mut config = None;
        self.load(Path::new(""), Path::new("config.json"), &mut |json| {
            config = Some(serde_json::from_slice(json)?);
            Ok(())
        })?;
        trace!("config loaded");
        Ok(config)
    }

    fn update_index(&mut self) -> CargoResult<()> {
        if self.config.cli_unstable().offline {
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
        let _lock =
            self.index_path
                .open_rw(Path::new(INDEX_LOCK), self.config, "the registry index")?;
        self.config
            .shell()
            .status("Updating", self.source_id.display_registry())?;

        // git fetch origin master
        let url = self.source_id.url();
        let refspec = "refs/heads/master:refs/remotes/origin/master";
        let repo = self.repo.borrow_mut().unwrap();
        git::fetch(repo, url, refspec, self.config)
            .chain_err(|| format!("failed to fetch `{}`", url))?;
        Ok(())
    }

    fn download(&mut self, pkg: PackageId, _checksum: &str) -> CargoResult<MaybeLock> {
        let filename = self.filename(pkg);

        // Attempt to open an read-only copy first to avoid an exclusive write
        // lock and also work with read-only filesystems. Note that we check the
        // length of the file like below to handle interrupted downloads.
        //
        // If this fails then we fall through to the exclusive path where we may
        // have to redownload the file.
        if let Ok(dst) = self.cache_path.open_ro(&filename, self.config, &filename) {
            let meta = dst.file().metadata()?;
            if meta.len() > 0 {
                return Ok(MaybeLock::Ready(dst));
            }
        }

        let config = self.config()?.unwrap();
        let mut url = config.dl;
        if !url.contains(CRATE_TEMPLATE) && !url.contains(VERSION_TEMPLATE) {
            write!(url, "/{}/{}/download", CRATE_TEMPLATE, VERSION_TEMPLATE).unwrap();
        }
        let url = url
            .replace(CRATE_TEMPLATE, &*pkg.name())
            .replace(VERSION_TEMPLATE, &pkg.version().to_string());

        Ok(MaybeLock::Download {
            url,
            descriptor: pkg.to_string(),
        })
    }

    fn finish_download(
        &mut self,
        pkg: PackageId,
        checksum: &str,
        data: &[u8],
    ) -> CargoResult<FileLock> {
        // Verify what we just downloaded
        let mut state = Sha256::new();
        state.update(data);
        if hex::encode(state.finish()) != checksum {
            failure::bail!("failed to verify the checksum of `{}`", pkg)
        }

        let filename = self.filename(pkg);
        let mut dst = self.cache_path.open_rw(&filename, self.config, &filename)?;
        let meta = dst.file().metadata()?;
        if meta.len() > 0 {
            return Ok(dst);
        }

        dst.write_all(data)?;
        dst.seek(SeekFrom::Start(0))?;
        Ok(dst)
    }

    fn is_crate_downloaded(&self, pkg: PackageId) -> bool {
        let filename = format!("{}-{}.crate", pkg.name(), pkg.version());
        let path = Path::new(&filename);

        if let Ok(dst) = self.cache_path.open_ro(path, self.config, &filename) {
            if let Ok(meta) = dst.file().metadata() {
                return meta.len() > 0;
            }
        }
        false
    }
}

impl<'cfg> Drop for RemoteRegistry<'cfg> {
    fn drop(&mut self) {
        // Just be sure to drop this before our other fields
        self.tree.borrow_mut().take();
    }
}
