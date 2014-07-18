#![allow(unused)]
use std::io::{mod, fs, File, MemReader};
use curl::http;
use git2;
use semver::Version;
use flate2::reader::GzDecoder;
use serialize::json;
use tar::Archive;
use url::Url;

use core::{Source, SourceId, PackageId, Package, Summary, Registry};
use core::Dependency;
use sources::PathSource;
use util::{CargoResult, Config, internal, ChainError, ToUrl, human};
use util::{hex, Require};
use ops;

static CENTRAL: &'static str = "https://example.com";

pub struct RegistrySource<'a, 'b:'a> {
    source_id: SourceId,
    checkout_path: Path,
    cache_path: Path,
    src_path: Path,
    config: &'a mut Config<'b>,
    handle: http::Handle,
    sources: Vec<PathSource>,
}

#[deriving(Decodable)]
struct RegistryConfig {
    dl_url: String,
}

impl<'a, 'b> RegistrySource<'a, 'b> {
    pub fn new(source_id: &SourceId,
               config: &'a mut Config<'b>) -> RegistrySource<'a, 'b> {
        let hash = hex::short_hash(source_id);
        let ident = source_id.get_url().host().unwrap().to_string();
        let part = format!("{}-{}", ident, hash);
        RegistrySource {
            checkout_path: config.registry_index_path().join(part.as_slice()),
            cache_path: config.registry_cache_path().join(part.as_slice()),
            src_path: config.registry_source_path().join(part.as_slice()),
            config: config,
            source_id: source_id.clone(),
            handle: http::Handle::new(),
            sources: Vec::new(),
        }
    }

    /// Get the configured default registry URL.
    ///
    /// This is the main cargo registry by default, but it can be overridden in
    /// a .cargo/config
    pub fn url() -> CargoResult<Url> {
        let config = try!(ops::upload_configuration());
        let url = config.host.unwrap_or(CENTRAL.to_string());
        url.as_slice().to_url().map_err(human)
    }

    /// Translates the HTTP url of the registry to the git URL
    fn git_url(&self) -> Url {
        let mut url = self.source_id.get_url().clone();
        url.path_mut().unwrap().push("git".to_string());
        url.path_mut().unwrap().push("index".to_string());
        url
    }

    /// Decode the configuration stored within the registry.
    ///
    /// This requires that the index has been at least checked out.
    fn config(&self) -> CargoResult<RegistryConfig> {
        let mut f = try!(File::open(&self.checkout_path.join("config.json")));
        let contents = try!(f.read_to_string());
        let config = try!(json::decode(contents.as_slice()));
        Ok(config)
    }

    /// Open the git repository for the index of the registry.
    ///
    /// This will attempt to open an existing checkout, and failing that it will
    /// initialize a fresh new directory and git checkout. No remotes will be
    /// configured by default.
    fn open(&self) -> CargoResult<git2::Repository> {
        match git2::Repository::open(&self.checkout_path) {
            Ok(repo) => return Ok(repo),
            Err(..) => {}
        }

        try!(fs::mkdir_recursive(&self.checkout_path, io::UserDir));
        let _ = fs::rmdir_recursive(&self.checkout_path);
        let url = self.git_url().to_string();
        let repo = try!(git2::Repository::init(&self.checkout_path));
        Ok(repo)
    }

    /// Download the given package from the given url into the local cache.
    ///
    /// This will perform the HTTP request to fetch the package. This function
    /// will only succeed if the HTTP download was successful and the file is
    /// then ready for inspection.
    ///
    /// No action is taken if the package is already downloaded.
    fn download_package(&mut self, pkg: &PackageId, url: Url)
                        -> CargoResult<Path> {
        let dst = self.cache_path.join(url.path().unwrap().last().unwrap()
                                          .as_slice());
        if dst.exists() { return Ok(dst) }
        try!(self.config.shell().status("Downloading", pkg));

        try!(fs::mkdir_recursive(&dst.dir_path(), io::UserDir));
        // TODO: don't download into memory
        let resp = try!(self.handle.get(url.to_string()).exec());
        if resp.get_code() != 200 {
            return Err(internal(format!("Failed to get 200 reponse from {}\n{}",
                                        url, resp)))
        }
        try!(File::create(&dst).write(resp.get_body()));
        Ok(dst)
    }

    /// Unpacks a downloaded package into a location where it's ready to be
    /// compiled.
    ///
    /// No action is taken if the source looks like it's already unpacked.
    fn unpack_package(&self, pkg: &PackageId, tarball: Path)
                      -> CargoResult<Path> {
        let dst = self.src_path.join(format!("{}-{}", pkg.get_name(),
                                             pkg.get_version()));
        if dst.join(".cargo-ok").exists() { return Ok(dst) }

        try!(fs::mkdir_recursive(&dst.dir_path(), io::UserDir));
        let f = try!(File::open(&tarball));
        let mut gz = try!(GzDecoder::new(f));
        // TODO: don't read into memory
        let mem = try!(gz.read_to_end());
        let tar = Archive::new(MemReader::new(mem));
        for file in try!(tar.files()) {
            let mut file = try!(file);
            let dst = dst.dir_path().join(file.filename_bytes());
            try!(fs::mkdir_recursive(&dst.dir_path(), io::UserDir));
            let mut dst = try!(File::create(&dst));
            try!(io::util::copy(&mut file, &mut dst));
        }
        try!(File::create(&dst.join(".cargo-ok")));
        Ok(dst)
    }
}

impl<'a, 'b> Registry for RegistrySource<'a, 'b> {
    fn query(&mut self, dep: &Dependency) -> CargoResult<Vec<Summary>> {
        let path = &self.checkout_path;
        let mut chars = dep.get_name().chars();
        let path = path.join(format!("{}{}", chars.next().unwrap_or('X'),
                                     chars.next().unwrap_or('X')));
        let path = path.join(format!("{}{}", chars.next().unwrap_or('X'),
                                     chars.next().unwrap_or('X')));
        let path = path.join(dep.get_name());
        let contents = match File::open(&path) {
            Ok(mut f) => try!(f.read_to_string()),
            Err(..) => return Ok(Vec::new()),
        };

        let ret: CargoResult<Vec<Summary>>;
        ret = contents.as_slice().lines().filter(|l| l.trim().len() > 0)
                      .map(|l| {
            #[deriving(Decodable)]
            struct Package { name: String, vers: String, deps: Vec<String> }

            let pkg = try!(json::decode::<Package>(l));
            let pkgid = try!(PackageId::new(pkg.name.as_slice(),
                                            pkg.vers.as_slice(),
                                            &self.source_id));
            let deps: CargoResult<Vec<Dependency>> = pkg.deps.iter().map(|dep| {
                let mut parts = dep.as_slice().splitn(1, '|');
                let name = parts.next().unwrap();
                let vers = try!(parts.next().require(|| {
                    human(format!("malformed dependency in registry: {}", dep))
                }));
                Dependency::parse(name, Some(vers), &self.source_id)
            }).collect();
            let deps = try!(deps);
            Ok(Summary::new(&pkgid, deps.as_slice()))
        }).collect();
        let mut summaries = try!(ret.chain_error(|| {
            internal(format!("Failed to parse registry's information for: {}",
                             dep.get_name()))
        }));
        summaries.query(dep)
    }
}

impl<'a, 'b> Source for RegistrySource<'a, 'b> {
    fn update(&mut self) -> CargoResult<()> {
        try!(self.config.shell().status("Updating",
             format!("registry `{}`", self.source_id.get_url())));
        let repo = try!(self.open());

        // git fetch origin
        let url = self.git_url().to_string();
        let refspec = "refs/heads/*:refs/remotes/origin/*";
        let mut remote = try!(repo.remote_create_anonymous(url.as_slice(),
                                                           refspec));
        log!(5, "[{}] fetching {}", self.source_id, url);
        try!(remote.fetch(None, None).chain_error(|| {
            internal(format!("failed to fetch `{}`", url))
        }));

        // git reset --hard origin/master
        let reference = "refs/remotes/origin/master";
        let oid = try!(git2::Reference::name_to_id(&repo, reference));
        log!(5, "[{}] updating to rev {}", self.source_id, oid);
        let object = try!(git2::Object::lookup(&repo, oid, None));
        try!(repo.reset(&object, git2::Hard, None, None));
        Ok(())
    }

    fn download(&mut self, packages: &[PackageId]) -> CargoResult<()> {
        let config = try!(self.config());
        let url = try!(config.dl_url.as_slice().to_url().map_err(internal));
        for package in packages.iter() {
            if self.source_id != *package.get_source_id() { continue }

            let mut url = url.clone();
            url.path_mut().unwrap().push("pkg".to_string());
            url.path_mut().unwrap().push(package.get_name().to_string());
            url.path_mut().unwrap().push(format!("{}-{}.tar.gz",
                                                 package.get_name(),
                                                 package.get_version()));
            let path = try!(self.download_package(package, url).chain_error(|| {
                internal(format!("Failed to download package `{}`", package))
            }));
            let path = try!(self.unpack_package(package, path).chain_error(|| {
                internal(format!("Failed to unpack package `{}`", package))
            }));
            let mut src = PathSource::new(&path, &self.source_id);
            try!(src.update());
            self.sources.push(src);
        }
        Ok(())
    }

    fn get(&self, packages: &[PackageId]) -> CargoResult<Vec<Package>> {
        let mut ret = Vec::new();
        for src in self.sources.iter() {
            ret.extend(try!(src.get(packages)).move_iter());
        }
        return Ok(ret);
    }

    fn fingerprint(&self, pkg: &Package) -> CargoResult<String> {
        Ok(pkg.get_package_id().get_version().to_string())
    }
}
