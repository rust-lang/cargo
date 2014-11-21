//! A `Source` for registry-based packages.
//!
//! # What's a Registry?
//!
//! Registries are central locations where packages can be uploaded to,
//! discovered, and searched for. The purpose of a registry is to have a
//! location that serves as permanent storage for versions of a crate over time.
//!
//! Compared to git sources, a registry provides many packages as well as many
//! versions simultaneously. Git sources can also have commits deleted through
//! rebasings where registries cannot have their versions deleted.
//!
//! # The Index of a Registry
//!
//! One of the major difficulties with a registry is that hosting so many
//! packages may quickly run into performance problems when dealing with
//! dependency graphs. It's infeasible for cargo to download the entire contents
//! of the registry just to resolve one package's dependencies, for example. As
//! a result, cargo needs some efficient method of querying what packages are
//! available on a registry, what versions are available, and what the
//! dependencies for each version is.
//!
//! One method of doing so would be having the registry expose an HTTP endpoint
//! which can be queried with a list of packages and a response of their
//! dependencies and versions is returned. This is somewhat inefficient however
//! as we may have to hit the endpoint many times and we may have already
//! queried for much of the data locally already (for other packages, for
//! example). This also involves inventing a transport format between the
//! registry and Cargo itself, so this route was not taken.
//!
//! Instead, Cargo communicates with registries through a git repository
//! referred to as the Index. The Index of a registry is essentially an easily
//! query-able version of the registry's database for a list of versions of a
//! package as well as a list of dependencies for each version.
//!
//! Using git to host this index provides a number of benefits:
//!
//! * The entire index can be stored efficiently locally on disk. This means
//!   that all queries of a registry can happen locally and don't need to touch
//!   the network.
//!
//! * Updates of the index are quite efficient. Using git buys incremental
//!   updates, compressed transmission, etc for free. The index must be updated
//!   each time we need fresh information from a registry, but this is one
//!   update of a git repository that probably hasn't changed a whole lot so
//!   it shouldn't be too expensive.
//!
//!   Additionally, each modification to the index is just appending a line at
//!   the end of a file (the exact format is described later). This means that
//!   the commits for an index are quite small and easily applied/compressable.
//!
//! ## The format of the Index
//!
//! The index is a store for the list of versions for all packages known, so its
//! format on disk is optimized slightly to ensure that `ls registry` doesn't
//! produce a list of all packages ever known. The index also wants to ensure
//! that there's not a million files which may actually end up hitting
//! filesystem limits at some point. To this end, a few decisions were made
//! about the format of the registry:
//!
//! 1. Each crate will have one file corresponding to it. Each version for a
//!    crate will just be a line in this file.
//! 2. There will be two tiers of directories for crate names, under which
//!    crates corresponding to those tiers will be located.
//!
//! As an example, this is an example hierarchy of an index:
//!
//! ```notrust
//! .
//! ├── 3
//! │   └── u
//! │       └── url
//! ├── bz
//! │   └── ip
//! │       └── bzip2
//! ├── config.json
//! ├── en
//! │   └── co
//! │       └── encoding
//! └── li
//!     ├── bg
//!     │   └── libgit2
//!     └── nk
//!         └── link-config
//! ```
//!
//! The root of the index contains a `config.json` file with a few entries
//! corresponding to the registry (see `RegistryConfig` below).
//!
//! Otherwise, there are three numbered directories (1, 2, 3) for crates with
//! names 1, 2, and 3 characters in length. The 1/2 directories simply have the
//! crate files underneath them, while the 3 directory is sharded by the first
//! letter of the crate name.
//!
//! Otherwise the top-level directory contains many two-letter directory names,
//! each of which has many sub-folders with two letters. At the end of all these
//! are the actual crate files themselves.
//!
//! The purpose of this layou tis to hopefully cut down on `ls` sizes as well as
//! efficient lookup based on the crate name itself.
//!
//! ## Crate files
//!
//! Each file in the index is the history of one crate over time. Each line in
//! the file corresponds to one version of a crate, stored in JSON format (see
//! the `RegistryPackage` structure below).
//!
//! As new versions are published, new lines are appended to this file. The only
//! modifications to this file that should happen over time are yanks of a
//! particular version.
//!
//! # Downloading Packages
//!
//! The purpose of the Index was to provide an efficient method to resolve the
//! dependency graph for a package. So far we only required one network
//! interaction to update the registry's repository (yay!). After resolution has
//! been performed, however we need to download the contents of packages so we
//! can read the full manifest and build the source code.
//!
//! To accomplish this, this source's `download` method will make an HTTP
//! request per-package requested to download tarballs into a local cache. These
//! tarballs will then be unpacked into a destination folder.
//!
//! Note that because versions uploaded to the registry are frozen forever that
//! the HTTP download and unpacking can all be skipped if the version has
//! already been downloaded and unpacked. This caching allows us to only
//! download a package when absolutely necessary.
//!
//! # Filesystem Hierarchy
//!
//! Overall, the `$HOME/.cargo` looks like this when talking about the registry:
//!
//! ```notrust
//! # A folder under which all registry metadata is hosted (similar to
//! # $HOME/.cargo/git)
//! $HOME/.cargo/registry/
//!
//!     # For each registry that cargo knows about (keyed by hostname + hash)
//!     # there is a folder which is the checked out version of the index for
//!     # the registry in this location. Note that this is done so cargo can
//!     # support multiple registries simultaneously
//!     index/
//!         registry1-<hash>/
//!         registry2-<hash>/
//!         ...
//!
//!     # This folder is a cache for all downloaded tarballs from a registry.
//!     # Once downloaded and verified, a tarball never changes.
//!     cache/
//!         registry1-<hash>/<pkg>-<version>.crate
//!         ...
//!
//!     # Location in which all tarballs are unpacked. Each tarball is known to
//!     # be frozen after downloading, so transitively this folder is also
//!     # frozen once its unpacked (it's never unpacked again)
//!     src/
//!         registry1-<hash>/<pkg>-<version>/...
//!         ...
//! ```

use std::io::{mod, fs, File};
use std::io::fs::PathExtensions;
use std::collections::HashMap;

use curl::http;
use git2;
use flate2::reader::GzDecoder;
use serialize::json;
use serialize::hex::ToHex;
use tar::Archive;
use url::Url;

use core::{Source, SourceId, PackageId, Package, Summary, Registry};
use core::dependency::{Dependency, Kind};
use sources::{PathSource, git};
use util::{CargoResult, Config, internal, ChainError, ToUrl, human};
use util::{hex, Require, Sha256};
use ops;

static DEFAULT: &'static str = "https://github.com/rust-lang/crates.io-index";

pub struct RegistrySource<'a, 'b:'a> {
    source_id: SourceId,
    checkout_path: Path,
    cache_path: Path,
    src_path: Path,
    config: &'a Config<'b>,
    handle: Option<http::Handle>,
    sources: Vec<PathSource>,
    hashes: HashMap<(String, String), String>, // (name, vers) => cksum
    cache: HashMap<String, Vec<(Summary, bool)>>,
    updated: bool,
}

#[deriving(Decodable)]
pub struct RegistryConfig {
    /// Download endpoint for all crates. This will be appended with
    /// `/<crate>/<version>/download` and then will be hit with an HTTP GET
    /// request to download the tarball for a crate.
    pub dl: String,

    /// API endpoint for the registry. This is what's actually hit to perform
    /// operations like yanks, owner modifications, publish new crates, etc.
    pub api: String,
}

#[deriving(Decodable)]
struct RegistryPackage {
    name: String,
    vers: String,
    deps: Vec<RegistryDependency>,
    features: HashMap<String, Vec<String>>,
    cksum: String,
    yanked: Option<bool>,
}

#[deriving(Decodable)]
struct RegistryDependency {
    name: String,
    req: String,
    features: Vec<String>,
    optional: bool,
    default_features: bool,
    target: Option<String>,
    kind: Option<String>,
}

impl<'a, 'b> RegistrySource<'a, 'b> {
    pub fn new(source_id: &SourceId,
               config: &'a Config<'b>) -> RegistrySource<'a, 'b> {
        let hash = hex::short_hash(source_id);
        let ident = source_id.get_url().host().unwrap().to_string();
        let part = format!("{}-{}", ident, hash);
        RegistrySource {
            checkout_path: config.registry_index_path().join(part.as_slice()),
            cache_path: config.registry_cache_path().join(part.as_slice()),
            src_path: config.registry_source_path().join(part.as_slice()),
            config: config,
            source_id: source_id.clone(),
            handle: None,
            sources: Vec::new(),
            hashes: HashMap::new(),
            cache: HashMap::new(),
            updated: false,
        }
    }

    /// Get the configured default registry URL.
    ///
    /// This is the main cargo registry by default, but it can be overridden in
    /// a .cargo/config
    pub fn url() -> CargoResult<Url> {
        let config = try!(ops::registry_configuration());
        let url = config.index.unwrap_or(DEFAULT.to_string());
        url.as_slice().to_url().map_err(human)
    }

    /// Get the default url for the registry
    pub fn default_url() -> String {
        DEFAULT.to_string()
    }

    /// Decode the configuration stored within the registry.
    ///
    /// This requires that the index has been at least checked out.
    pub fn config(&self) -> CargoResult<RegistryConfig> {
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

        try!(fs::mkdir_recursive(&self.checkout_path, io::USER_DIR));
        let _ = fs::rmdir_recursive(&self.checkout_path);
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
    fn download_package(&mut self, pkg: &PackageId, url: &Url)
                        -> CargoResult<Path> {
        // TODO: should discover from the S3 redirect
        let filename = format!("{}-{}.crate", pkg.get_name(), pkg.get_version());
        let dst = self.cache_path.join(filename);
        if dst.exists() { return Ok(dst) }
        try!(self.config.shell().status("Downloading", pkg));

        try!(fs::mkdir_recursive(&dst.dir_path(), io::USER_DIR));
        let handle = match self.handle {
            Some(ref mut handle) => handle,
            None => {
                self.handle = Some(try!(ops::http_handle()));
                self.handle.as_mut().unwrap()
            }
        };
        // TODO: don't download into memory (curl-rust doesn't expose it)
        let resp = try!(handle.get(url.to_string()).follow_redirects(true).exec());
        if resp.get_code() != 200 && resp.get_code() != 0 {
            return Err(internal(format!("Failed to get 200 reponse from {}\n{}",
                                        url, resp)))
        }

        // Verify what we just downloaded
        let expected = self.hashes.get(&(pkg.get_name().to_string(),
                                         pkg.get_version().to_string()));
        let expected = try!(expected.require(|| {
            internal(format!("no hash listed for {}", pkg))
        }));
        let actual = {
            let mut state = Sha256::new();
            state.update(resp.get_body());
            state.finish()
        };
        if actual.as_slice().to_hex() != *expected {
            return Err(human(format!("Failed to verify the checksum of `{}`",
                                     pkg)))
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

        try!(fs::mkdir_recursive(&dst.dir_path(), io::USER_DIR));
        let f = try!(File::open(&tarball));
        let gz = try!(GzDecoder::new(f));
        let mut tar = Archive::new(gz);
        try!(tar.unpack(&dst.dir_path()));
        try!(File::create(&dst.join(".cargo-ok")));
        Ok(dst)
    }

    /// Parse the on-disk metadata for the package provided
    fn summaries(&mut self, name: &str) -> CargoResult<&Vec<(Summary, bool)>> {
        if self.cache.contains_key(name) {
            return Ok(self.cache.get(name).unwrap());
        }
        // see module comment for why this is structured the way it is
        let path = self.checkout_path.clone();
        let path = match name.len() {
            1 => path.join("1").join(name),
            2 => path.join("2").join(name),
            3 => path.join("3").join(name.slice_to(1)).join(name),
            _ => path.join(name.slice(0, 2))
                     .join(name.slice(2, 4))
                     .join(name),
        };
        let summaries = match File::open(&path) {
            Ok(mut f) => {
                let contents = try!(f.read_to_string());
                let ret: CargoResult<Vec<(Summary, bool)>>;
                ret = contents.as_slice().lines().filter(|l| l.trim().len() > 0)
                              .map(|l| self.parse_registry_package(l))
                              .collect();
                try!(ret.chain_error(|| {
                    internal(format!("Failed to parse registry's information \
                                      for: {}", name))
                }))
            }
            Err(..) => Vec::new(),
        };
        self.cache.insert(name.to_string(), summaries);
        Ok(self.cache.get(name).unwrap())
    }

    /// Parse a line from the registry's index file into a Summary for a
    /// package.
    ///
    /// The returned boolean is whether or not the summary has been yanked.
    fn parse_registry_package(&mut self, line: &str)
                              -> CargoResult<(Summary, bool)> {
        let RegistryPackage {
            name, vers, cksum, deps, features, yanked
        } = try!(json::decode::<RegistryPackage>(line));
        let pkgid = try!(PackageId::new(name.as_slice(),
                                        vers.as_slice(),
                                        &self.source_id));
        let deps: CargoResult<Vec<Dependency>> = deps.into_iter().map(|dep| {
            self.parse_registry_dependency(dep)
        }).collect();
        let deps = try!(deps);
        self.hashes.insert((name, vers), cksum);
        Ok((try!(Summary::new(pkgid, deps, features)), yanked.unwrap_or(false)))
    }

    /// Converts an encoded dependency in the registry to a cargo dependency
    fn parse_registry_dependency(&self, dep: RegistryDependency)
                                 -> CargoResult<Dependency> {
        let RegistryDependency {
            name, req, features, optional, default_features, target, kind
        } = dep;

        let dep = try!(Dependency::parse(name.as_slice(), Some(req.as_slice()),
                                         &self.source_id));
        let kind = match kind.as_ref().map(|s| s.as_slice()).unwrap_or("") {
            "dev" => Kind::Development,
            "build" => Kind::Build,
            _ => Kind::Normal,
        };

        Ok(dep.optional(optional)
              .default_features(default_features)
              .features(features)
              .only_for_platform(target)
              .kind(kind))
    }

    /// Actually perform network operations to update the registry
    fn do_update(&mut self) -> CargoResult<()> {
        if self.updated { return Ok(()) }

        try!(self.config.shell().status("Updating",
             format!("registry `{}`", self.source_id.get_url())));
        let repo = try!(self.open());

        // git fetch origin
        let url = self.source_id.get_url().to_string();
        let refspec = "refs/heads/*:refs/remotes/origin/*";
        try!(git::fetch(&repo, url.as_slice(), refspec).chain_error(|| {
            internal(format!("failed to fetch `{}`", url))
        }));

        // git reset --hard origin/master
        let reference = "refs/remotes/origin/master";
        let oid = try!(repo.refname_to_id(reference));
        log!(5, "[{}] updating to rev {}", self.source_id, oid);
        let object = try!(repo.find_object(oid, None));
        try!(repo.reset(&object, git2::Hard, None, None));
        self.updated = true;
        self.cache.clear();
        Ok(())
    }
}

impl<'a, 'b> Registry for RegistrySource<'a, 'b> {
    fn query(&mut self, dep: &Dependency) -> CargoResult<Vec<Summary>> {
        // If this is a precise dependency, then it came from a lockfile and in
        // theory the registry is known to contain this version. If, however, we
        // come back with no summaries, then our registry may need to be
        // updated, so we fall back to performing a lazy update.
        if dep.get_source_id().get_precise().is_some() &&
           try!(self.summaries(dep.get_name())).len() == 0 {
            try!(self.do_update());
        }

        let summaries = try!(self.summaries(dep.get_name()));
        let mut summaries = summaries.iter().filter(|&&(_, yanked)| {
            dep.get_source_id().get_precise().is_some() || !yanked
        }).map(|&(ref s, _)| s.clone()).collect::<Vec<_>>();
        summaries.query(dep)
    }
}

impl<'a, 'b> Source for RegistrySource<'a, 'b> {
    fn update(&mut self) -> CargoResult<()> {
        // If we have an imprecise version then we don't know what we're going
        // to look for, so we always atempt to perform an update here.
        //
        // If we have a precise version, then we'll update lazily during the
        // querying phase.
        if self.source_id.get_precise().is_none() {
            try!(self.do_update());
        }
        Ok(())
    }

    fn download(&mut self, packages: &[PackageId]) -> CargoResult<()> {
        let config = try!(self.config());
        let url = try!(config.dl.as_slice().to_url().map_err(internal));
        for package in packages.iter() {
            if self.source_id != *package.get_source_id() { continue }

            let mut url = url.clone();
            url.path_mut().unwrap().push(package.get_name().to_string());
            url.path_mut().unwrap().push(package.get_version().to_string());
            url.path_mut().unwrap().push("download".to_string());
            let path = try!(self.download_package(package, &url).chain_error(|| {
                internal(format!("Failed to download package `{}` from {}",
                                 package, url))
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
            ret.extend(try!(src.get(packages)).into_iter());
        }
        return Ok(ret);
    }

    fn fingerprint(&self, pkg: &Package) -> CargoResult<String> {
        Ok(pkg.get_package_id().get_version().to_string())
    }
}
