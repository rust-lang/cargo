use std::fs;
use std::path::{PathBuf, Path};

use curl::http;
use git2;
use rustc_serialize::json;
use rustc_serialize::hex::ToHex;
use url::Url;

use core::{PackageId, SourceId};
use ops;
use sources::git;
use sources::registry::{RegistryData, RegistryConfig};
use util::paths;
use util::{Config, CargoResult, ChainError, human, internal, Sha256, ToUrl};

pub struct RemoteRegistry<'cfg> {
    index_path: PathBuf,
    cache_path: PathBuf,
    source_id: SourceId,
    config: &'cfg Config,
    handle: Option<http::Handle>,
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

    fn download(&mut self, url: &Url) -> CargoResult<Vec<u8>> {
        let handle = match self.handle {
            Some(ref mut handle) => handle,
            None => {
                self.handle = Some(try!(ops::http_handle(self.config)));
                self.handle.as_mut().unwrap()
            }
        };
        // TODO: don't download into memory (curl-rust doesn't expose it)
        let resp = try!(handle.get(url.to_string()).follow_redirects(true).exec());
        if resp.get_code() != 200 && resp.get_code() != 0 {
            Err(internal(format!("failed to get 200 response from {}\n{}",
                                 url, resp)))
        } else {
            Ok(resp.move_body())
        }
    }
}

impl<'cfg> RegistryData for RemoteRegistry<'cfg> {
    fn index_path(&self) -> &Path {
        &self.index_path
    }

    fn config(&self) -> CargoResult<Option<RegistryConfig>> {
        let contents = try!(paths::read(&self.index_path.join("config.json")));
        let config = try!(json::decode(&contents));
        Ok(Some(config))
    }

    fn update_index(&mut self) -> CargoResult<()> {
        let msg = format!("registry `{}`", self.source_id.url());
        try!(self.config.shell().status("Updating", msg));

        let repo = match git2::Repository::open(&self.index_path) {
            Ok(repo) => repo,
            Err(..) => {
                try!(fs::create_dir_all(&self.index_path));
                let _ = fs::remove_dir_all(&self.index_path);
                try!(git2::Repository::init(&self.index_path))
            }
        };

        // git fetch origin
        let url = self.source_id.url().to_string();
        let refspec = "refs/heads/*:refs/remotes/origin/*";
        try!(git::fetch(&repo, &url, refspec).chain_error(|| {
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
                -> CargoResult<PathBuf> {
        // TODO: should discover filename from the S3 redirect
        let filename = format!("{}-{}.crate", pkg.name(), pkg.version());
        let dst = self.cache_path.join(&filename);
        if fs::metadata(&dst).is_ok() {
            return Ok(dst)
        }

        try!(fs::create_dir_all(dst.parent().unwrap()));

        let config = try!(self.config()).unwrap();
        let mut url = try!(config.dl.to_url().map_err(internal));
        url.path_mut().unwrap().push(pkg.name().to_string());
        url.path_mut().unwrap().push(pkg.version().to_string());
        url.path_mut().unwrap().push("download".to_string());

        try!(self.config.shell().status("Downloading", pkg));
        let data = try!(self.download(&url).chain_error(|| {
            human(format!("failed to download package `{}` from {}", pkg, url))
        }));

        // Verify what we just downloaded
        let actual = {
            let mut state = Sha256::new();
            state.update(&data);
            state.finish()
        };
        if actual.to_hex() != checksum {
            bail!("failed to verify the checksum of `{}`", pkg)
        }

        try!(paths::write(&dst, &data));
        Ok(dst)
    }
}
