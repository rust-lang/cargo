use std::io::SeekFrom;
use std::io::prelude::*;
use std::path::Path;

use curl::easy::Easy;
use git2;
use rustc_serialize::json;
use rustc_serialize::hex::ToHex;

use core::{PackageId, SourceId};
use ops;
use sources::git;
use sources::registry::{RegistryData, RegistryConfig, INDEX_LOCK};
use util::network;
use util::paths;
use util::{FileLock, Filesystem};
use util::{Config, CargoResult, ChainError, human, Sha256, ToUrl};

pub struct RemoteRegistry<'cfg> {
    index_path: Filesystem,
    cache_path: Filesystem,
    source_id: SourceId,
    config: &'cfg Config,
    handle: Option<Easy>,
}

impl<'cfg> RemoteRegistry<'cfg> {
    pub fn new(source_id: &SourceId, config: &'cfg Config, name: &str)
               -> RemoteRegistry<'cfg> {
        RemoteRegistry {
            index_path: config.registry_index_path().join(name),
            cache_path: config.registry_cache_path().join(name),
            source_id: source_id.clone(),
            config: config,
            handle: None,
        }
    }
}

impl<'cfg> RegistryData for RemoteRegistry<'cfg> {
    fn index_path(&self) -> &Filesystem {
        &self.index_path
    }

    fn config(&self) -> CargoResult<Option<RegistryConfig>> {
        let lock = try!(self.index_path.open_ro(Path::new(INDEX_LOCK),
                                                self.config,
                                                "the registry index"));
        let path = lock.path().parent().unwrap();
        let contents = try!(paths::read(&path.join("config.json")));
        let config = try!(json::decode(&contents));
        Ok(Some(config))
    }

    fn update_index(&mut self) -> CargoResult<()> {
        // Ensure that we'll actually be able to acquire an HTTP handle later on
        // once we start trying to download crates. This will weed out any
        // problems with `.cargo/config` configuration related to HTTP.
        //
        // This way if there's a problem the error gets printed before we even
        // hit the index, which may not actually read this configuration.
        try!(ops::http_handle(self.config));

        // Then we actually update the index
        try!(self.index_path.create_dir());
        let lock = try!(self.index_path.open_rw(Path::new(INDEX_LOCK),
                                                self.config,
                                                "the registry index"));
        let path = lock.path().parent().unwrap();

        try!(self.config.shell().status("Updating",
             format!("registry `{}`", self.source_id.url())));
        let repo = try!(git2::Repository::open(path).or_else(|_| {
            let _ = lock.remove_siblings();
            git2::Repository::init(path)
        }));

        // git fetch origin
        let url = self.source_id.url().to_string();
        let refspec = "refs/heads/*:refs/remotes/origin/*";

        try!(git::fetch(&repo, &url, refspec, &self.config).chain_error(|| {
            human(format!("failed to fetch `{}`", url))
        }));

        // git reset --hard origin/master
        let reference = "refs/remotes/origin/master";
        let oid = try!(repo.refname_to_id(reference));
        trace!("[{}] updating to rev {}", self.source_id, oid);
        let object = try!(repo.find_object(oid, None));
        try!(repo.reset(&object, git2::ResetType::Hard, None));
        Ok(())
    }

    fn download(&mut self, pkg: &PackageId, checksum: &str)
                -> CargoResult<FileLock> {
        let filename = format!("{}-{}.crate", pkg.name(), pkg.version());
        let path = Path::new(&filename);
        let mut dst = try!(self.cache_path.open_rw(path, self.config, &filename));
        let meta = try!(dst.file().metadata());
        if meta.len() > 0 {
            return Ok(dst)
        }
        try!(self.config.shell().status("Downloading", pkg));

        let config = try!(self.config()).unwrap();
        let mut url = try!(config.dl.to_url());
        url.path_segments_mut().unwrap()
            .push(pkg.name())
            .push(&pkg.version().to_string())
            .push("download");

        let handle = match self.handle {
            Some(ref mut handle) => handle,
            None => {
                self.handle = Some(try!(ops::http_handle(self.config)));
                self.handle.as_mut().unwrap()
            }
        };
        // TODO: don't download into memory, but ensure that if we ctrl-c a
        //       download we should resume either from the start or the middle
        //       on the next time
        try!(handle.get(true));
        try!(handle.url(&url.to_string()));
        try!(handle.follow_location(true));
        let mut state = Sha256::new();
        let mut body = Vec::new();
        {
            let mut handle = handle.transfer();
            try!(handle.write_function(|buf| {
                state.update(buf);
                body.extend_from_slice(buf);
                Ok(buf.len())
            }));
            try!(network::with_retry(self.config, || {
                handle.perform()
            }))
        }
        let code = try!(handle.response_code());
        if code != 200 && code != 0 {
            bail!("failed to get 200 response from `{}`, got {}", url, code)
        }

        // Verify what we just downloaded
        if state.finish().to_hex() != checksum {
            bail!("failed to verify the checksum of `{}`", pkg)
        }

        try!(dst.write_all(&body));
        try!(dst.seek(SeekFrom::Start(0)));
        Ok(dst)
    }
}
