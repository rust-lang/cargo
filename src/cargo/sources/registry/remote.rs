use std::cell::{RefCell, Ref, Cell};
use std::io::SeekFrom;
use std::io::prelude::*;
use std::mem;
use std::path::Path;

use git2;
use hex::ToHex;
use serde_json;

use core::{PackageId, SourceId};
use ops;
use sources::git;
use sources::registry::{RegistryData, RegistryConfig, INDEX_LOCK};
use util::network;
use util::{FileLock, Filesystem, LazyCell};
use util::{Config, Sha256, ToUrl};
use util::errors::{CargoErrorKind, CargoResult, CargoResultExt};

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
    pub fn new(source_id: &SourceId, config: &'cfg Config, name: &str)
               -> RemoteRegistry<'cfg> {
        RemoteRegistry {
            index_path: config.registry_index_path().join(name),
            cache_path: config.registry_cache_path().join(name),
            source_id: source_id.clone(),
            config: config,
            tree: RefCell::new(None),
            repo: LazyCell::new(),
            head: Cell::new(None),
        }
    }

    fn repo(&self) -> CargoResult<&git2::Repository> {
        self.repo.get_or_try_init(|| {
            let path = self.index_path.clone().into_path_unlocked();

            // Fast path without a lock
            if let Ok(repo) = git2::Repository::open(&path) {
                return Ok(repo)
            }

            // Ok, now we need to lock and try the whole thing over again.
            let lock = self.index_path.open_rw(Path::new(INDEX_LOCK),
                                               self.config,
                                               "the registry index")?;
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
                    Ok(git2::Repository::init(&path)?)
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

    fn tree(&self) -> CargoResult<Ref<git2::Tree>> {
        {
            let tree = self.tree.borrow();
            if tree.is_some() {
                return Ok(Ref::map(tree, |s| s.as_ref().unwrap()))
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
        let tree = unsafe {
            mem::transmute::<git2::Tree, git2::Tree<'static>>(tree)
        };
        *self.tree.borrow_mut() = Some(tree);
        Ok(Ref::map(self.tree.borrow(), |s| s.as_ref().unwrap()))
    }
}

impl<'cfg> RegistryData for RemoteRegistry<'cfg> {
    fn index_path(&self) -> &Filesystem {
        &self.index_path
    }

    fn load(&self,
            _root: &Path,
            path: &Path,
            data: &mut FnMut(&[u8]) -> CargoResult<()>) -> CargoResult<()> {
        // Note that the index calls this method and the filesystem is locked
        // in the index, so we don't need to worry about an `update_index`
        // happening in a different process.
        let repo = self.repo()?;
        let tree = self.tree()?;
        let entry = tree.get_path(path)?;
        let object = entry.to_object(&repo)?;
        let blob = match object.as_blob() {
            Some(blob) => blob,
            None => bail!("path `{}` is not a blob in the git repo", path.display()),
        };
        data(blob.content())
    }

    fn config(&mut self) -> CargoResult<Option<RegistryConfig>> {
        self.repo()?; // create intermediate dirs and initialize the repo
        let _lock = self.index_path.open_ro(Path::new(INDEX_LOCK),
                                            self.config,
                                            "the registry index")?;
        let mut config = None;
        self.load(Path::new(""), Path::new("config.json"), &mut |json| {
            config = Some(serde_json::from_slice(&json)?);
            Ok(())
        })?;
        Ok(config)
    }

    fn update_index(&mut self) -> CargoResult<()> {
        // Ensure that we'll actually be able to acquire an HTTP handle later on
        // once we start trying to download crates. This will weed out any
        // problems with `.cargo/config` configuration related to HTTP.
        //
        // This way if there's a problem the error gets printed before we even
        // hit the index, which may not actually read this configuration.
        ops::http_handle(self.config)?;

        self.repo()?;
        self.head.set(None);
        *self.tree.borrow_mut() = None;
        let _lock = self.index_path.open_rw(Path::new(INDEX_LOCK),
                                            self.config,
                                            "the registry index")?;
        self.config.shell().status("Updating",
             format!("registry `{}`", self.source_id.url()))?;

        // git fetch origin master
        let url = self.source_id.url();
        let refspec = "refs/heads/master:refs/remotes/origin/master";
        let repo = self.repo.borrow_mut().unwrap();
        git::fetch(repo, url, refspec, self.config).chain_err(|| {
            format!("failed to fetch `{}`", url)
        })?;
        Ok(())
    }

    fn download(&mut self, pkg: &PackageId, checksum: &str)
                -> CargoResult<FileLock> {
        let filename = format!("{}-{}.crate", pkg.name(), pkg.version());
        let path = Path::new(&filename);

        // Attempt to open an read-only copy first to avoid an exclusive write
        // lock and also work with read-only filesystems. Note that we check the
        // length of the file like below to handle interrupted downloads.
        //
        // If this fails then we fall through to the exclusive path where we may
        // have to redownload the file.
        if let Ok(dst) = self.cache_path.open_ro(path, self.config, &filename) {
            let meta = dst.file().metadata()?;
            if meta.len() > 0 {
                return Ok(dst)
            }
        }
        let mut dst = self.cache_path.open_rw(path, self.config, &filename)?;
        let meta = dst.file().metadata()?;
        if meta.len() > 0 {
            return Ok(dst)
        }
        self.config.shell().status("Downloading", pkg)?;

        let config = self.config()?.unwrap();
        let mut url = config.dl.to_url()?;
        url.path_segments_mut().unwrap()
            .push(pkg.name())
            .push(&pkg.version().to_string())
            .push("download");

        // TODO: don't download into memory, but ensure that if we ctrl-c a
        //       download we should resume either from the start or the middle
        //       on the next time
        let url = url.to_string();
        let mut handle = self.config.http()?.borrow_mut();
        handle.get(true)?;
        handle.url(&url)?;
        handle.follow_location(true)?;
        let mut state = Sha256::new();
        let mut body = Vec::new();
        network::with_retry(self.config, || {
            state = Sha256::new();
            body = Vec::new();
            {
                let mut handle = handle.transfer();
                handle.write_function(|buf| {
                    state.update(buf);
                    body.extend_from_slice(buf);
                    Ok(buf.len())
                })?;
                handle.perform()?;
            }
            let code = handle.response_code()?;
            if code != 200 && code != 0 {
                let url = handle.effective_url()?.unwrap_or(&url);
                Err(CargoErrorKind::HttpNot200(code, url.to_string()).into())
            } else {
                Ok(())
            }
        })?;

        // Verify what we just downloaded
        if state.finish().to_hex() != checksum {
            bail!("failed to verify the checksum of `{}`", pkg)
        }

        dst.write_all(&body)?;
        dst.seek(SeekFrom::Start(0))?;
        Ok(dst)
    }
}

impl<'cfg> Drop for RemoteRegistry<'cfg> {
    fn drop(&mut self) {
        // Just be sure to drop this before our other fields
        self.tree.borrow_mut().take();
    }
}
