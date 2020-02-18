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
//!   the commits for an index are quite small and easily applied/compressible.
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
//! The purpose of this layout is to hopefully cut down on `ls` sizes as well as
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

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use log::debug;
use semver::{Version, VersionReq};
use serde::Deserialize;
use tar::Archive;

use crate::core::dependency::{DepKind, Dependency};
use crate::core::source::MaybePackage;
use crate::core::{InternedString, Package, PackageId, Source, SourceId, Summary};
use crate::sources::PathSource;
use crate::util::errors::CargoResultExt;
use crate::util::hex;
use crate::util::into_url::IntoUrl;
use crate::util::{CargoResult, Config, Filesystem};

const PACKAGE_SOURCE_LOCK: &str = ".cargo-ok";
pub const CRATES_IO_INDEX: &str = "https://github.com/rust-lang/crates.io-index";
pub const CRATES_IO_REGISTRY: &str = "crates-io";
const CRATE_TEMPLATE: &str = "{crate}";
const VERSION_TEMPLATE: &str = "{version}";

pub struct RegistrySource<'cfg> {
    source_id: SourceId,
    src_path: Filesystem,
    config: &'cfg Config,
    updated: bool,
    ops: Box<dyn RegistryData + 'cfg>,
    index: index::RegistryIndex<'cfg>,
    yanked_whitelist: HashSet<PackageId>,
}

#[derive(Deserialize)]
pub struct RegistryConfig {
    /// Download endpoint for all crates.
    ///
    /// The string is a template which will generate the download URL for the
    /// tarball of a specific version of a crate. The substrings `{crate}` and
    /// `{version}` will be replaced with the crate's name and version
    /// respectively.
    ///
    /// For backwards compatibility, if the string does not contain `{crate}` or
    /// `{version}`, it will be extended with `/{crate}/{version}/download` to
    /// support registries like crates.io which were created before the
    /// templating setup was created.
    pub dl: String,

    /// API endpoint for the registry. This is what's actually hit to perform
    /// operations like yanks, owner modifications, publish new crates, etc.
    /// If this is None, the registry does not support API commands.
    pub api: Option<String>,
}

#[derive(Deserialize)]
pub struct RegistryPackage<'a> {
    name: InternedString,
    vers: Version,
    #[serde(borrow)]
    deps: Vec<RegistryDependency<'a>>,
    features: BTreeMap<InternedString, Vec<InternedString>>,
    cksum: String,
    yanked: Option<bool>,
    links: Option<InternedString>,
}

#[test]
fn escaped_char_in_json() {
    let _: RegistryPackage<'_> = serde_json::from_str(
        r#"{"name":"a","vers":"0.0.1","deps":[],"cksum":"bae3","features":{}}"#,
    )
    .unwrap();
    let _: RegistryPackage<'_> = serde_json::from_str(
        r#"{"name":"a","vers":"0.0.1","deps":[],"cksum":"bae3","features":{"test":["k","q"]},"links":"a-sys"}"#
    ).unwrap();

    // Now we add escaped cher all the places they can go
    // these are not valid, but it should error later than json parsing
    let _: RegistryPackage<'_> = serde_json::from_str(
        r#"{
        "name":"This name has a escaped cher in it \n\t\" ",
        "vers":"0.0.1",
        "deps":[{
            "name": " \n\t\" ",
            "req": " \n\t\" ",
            "features": [" \n\t\" "],
            "optional": true,
            "default_features": true,
            "target": " \n\t\" ",
            "kind": " \n\t\" ",
            "registry": " \n\t\" "
        }],
        "cksum":"bae3",
        "features":{"test \n\t\" ":["k \n\t\" ","q \n\t\" "]},
        "links":" \n\t\" "}"#,
    )
    .unwrap();
}

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "lowercase")]
enum Field {
    Name,
    Vers,
    Deps,
    Features,
    Cksum,
    Yanked,
    Links,
}

#[derive(Deserialize)]
struct RegistryDependency<'a> {
    name: InternedString,
    #[serde(borrow)]
    req: Cow<'a, str>,
    features: Vec<InternedString>,
    optional: bool,
    default_features: bool,
    target: Option<Cow<'a, str>>,
    kind: Option<Cow<'a, str>>,
    registry: Option<Cow<'a, str>>,
    package: Option<InternedString>,
    public: Option<bool>,
}

impl<'a> RegistryDependency<'a> {
    /// Converts an encoded dependency in the registry to a cargo dependency
    pub fn into_dep(self, default: SourceId) -> CargoResult<Dependency> {
        let RegistryDependency {
            name,
            req,
            mut features,
            optional,
            default_features,
            target,
            kind,
            registry,
            package,
            public,
        } = self;

        let id = if let Some(registry) = &registry {
            SourceId::for_registry(&registry.into_url()?)?
        } else {
            default
        };

        let mut dep = Dependency::parse_no_deprecated(package.unwrap_or(name), Some(&req), id)?;
        if package.is_some() {
            dep.set_explicit_name_in_toml(name);
        }
        let kind = match kind.as_ref().map(|s| &s[..]).unwrap_or("") {
            "dev" => DepKind::Development,
            "build" => DepKind::Build,
            _ => DepKind::Normal,
        };

        let platform = match target {
            Some(target) => Some(target.parse()?),
            None => None,
        };

        // All dependencies are private by default
        let public = public.unwrap_or(false);

        // Unfortunately older versions of cargo and/or the registry ended up
        // publishing lots of entries where the features array contained the
        // empty feature, "", inside. This confuses the resolution process much
        // later on and these features aren't actually valid, so filter them all
        // out here.
        features.retain(|s| !s.is_empty());

        // In index, "registry" is null if it is from the same index.
        // In Cargo.toml, "registry" is None if it is from the default
        if !id.is_default_registry() {
            dep.set_registry_id(id);
        }

        dep.set_optional(optional)
            .set_default_features(default_features)
            .set_features(features)
            .set_platform(platform)
            .set_kind(kind)
            .set_public(public);

        Ok(dep)
    }
}

pub trait RegistryData {
    fn prepare(&self) -> CargoResult<()>;
    fn index_path(&self) -> &Filesystem;
    fn load(
        &self,
        root: &Path,
        path: &Path,
        data: &mut dyn FnMut(&[u8]) -> CargoResult<()>,
    ) -> CargoResult<()>;
    fn config(&mut self) -> CargoResult<Option<RegistryConfig>>;
    fn update_index(&mut self) -> CargoResult<()>;
    fn download(&mut self, pkg: PackageId, checksum: &str) -> CargoResult<MaybeLock>;
    fn finish_download(&mut self, pkg: PackageId, checksum: &str, data: &[u8])
        -> CargoResult<File>;

    fn is_crate_downloaded(&self, _pkg: PackageId) -> bool {
        true
    }
    fn assert_index_locked<'a>(&self, path: &'a Filesystem) -> &'a Path;
    fn current_version(&self) -> Option<InternedString>;
}

pub enum MaybeLock {
    Ready(File),
    Download { url: String, descriptor: String },
}

mod index;
mod local;
mod remote;

fn short_name(id: SourceId) -> String {
    let hash = hex::short_hash(&id);
    let ident = id.url().host_str().unwrap_or("").to_string();
    format!("{}-{}", ident, hash)
}

impl<'cfg> RegistrySource<'cfg> {
    pub fn remote(
        source_id: SourceId,
        yanked_whitelist: &HashSet<PackageId>,
        config: &'cfg Config,
    ) -> RegistrySource<'cfg> {
        let name = short_name(source_id);
        let ops = remote::RemoteRegistry::new(source_id, config, &name);
        RegistrySource::new(source_id, config, &name, Box::new(ops), yanked_whitelist)
    }

    pub fn local(
        source_id: SourceId,
        path: &Path,
        yanked_whitelist: &HashSet<PackageId>,
        config: &'cfg Config,
    ) -> RegistrySource<'cfg> {
        let name = short_name(source_id);
        let ops = local::LocalRegistry::new(path, config, &name);
        RegistrySource::new(source_id, config, &name, Box::new(ops), yanked_whitelist)
    }

    fn new(
        source_id: SourceId,
        config: &'cfg Config,
        name: &str,
        ops: Box<dyn RegistryData + 'cfg>,
        yanked_whitelist: &HashSet<PackageId>,
    ) -> RegistrySource<'cfg> {
        RegistrySource {
            src_path: config.registry_source_path().join(name),
            config,
            source_id,
            updated: false,
            index: index::RegistryIndex::new(source_id, ops.index_path(), config),
            yanked_whitelist: yanked_whitelist.clone(),
            ops,
        }
    }

    /// Decode the configuration stored within the registry.
    ///
    /// This requires that the index has been at least checked out.
    pub fn config(&mut self) -> CargoResult<Option<RegistryConfig>> {
        self.ops.config()
    }

    /// Unpacks a downloaded package into a location where it's ready to be
    /// compiled.
    ///
    /// No action is taken if the source looks like it's already unpacked.
    fn unpack_package(&self, pkg: PackageId, tarball: &File) -> CargoResult<PathBuf> {
        // The `.cargo-ok` file is used to track if the source is already
        // unpacked.
        let package_dir = format!("{}-{}", pkg.name(), pkg.version());
        let dst = self.src_path.join(&package_dir);
        dst.create_dir()?;
        let path = dst.join(PACKAGE_SOURCE_LOCK);
        let path = self.config.assert_package_cache_locked(&path);
        let unpack_dir = path.parent().unwrap();
        if let Ok(meta) = path.metadata() {
            if meta.len() > 0 {
                return Ok(unpack_dir.to_path_buf());
            }
        }
        let mut ok = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&path)?;

        let gz = GzDecoder::new(tarball);
        let mut tar = Archive::new(gz);
        tar.set_preserve_mtime(false);
        let prefix = unpack_dir.file_name().unwrap();
        let parent = unpack_dir.parent().unwrap();
        for entry in tar.entries()? {
            let mut entry = entry.chain_err(|| "failed to iterate over archive")?;
            let entry_path = entry
                .path()
                .chain_err(|| "failed to read entry path")?
                .into_owned();

            // We're going to unpack this tarball into the global source
            // directory, but we want to make sure that it doesn't accidentally
            // (or maliciously) overwrite source code from other crates. Cargo
            // itself should never generate a tarball that hits this error, and
            // crates.io should also block uploads with these sorts of tarballs,
            // but be extra sure by adding a check here as well.
            if !entry_path.starts_with(prefix) {
                anyhow::bail!(
                    "invalid tarball downloaded, contains \
                     a file at {:?} which isn't under {:?}",
                    entry_path,
                    prefix
                )
            }

            // Once that's verified, unpack the entry as usual.
            entry
                .unpack_in(parent)
                .chain_err(|| format!("failed to unpack entry at `{}`", entry_path.display()))?;
        }

        // Write to the lock file to indicate that unpacking was successful.
        write!(ok, "ok")?;

        Ok(unpack_dir.to_path_buf())
    }

    fn do_update(&mut self) -> CargoResult<()> {
        self.ops.update_index()?;
        let path = self.ops.index_path();
        self.index = index::RegistryIndex::new(self.source_id, path, self.config);
        self.updated = true;
        Ok(())
    }

    fn get_pkg(&mut self, package: PackageId, path: &File) -> CargoResult<Package> {
        let path = self
            .unpack_package(package, path)
            .chain_err(|| format!("failed to unpack package `{}`", package))?;
        let mut src = PathSource::new(&path, self.source_id, self.config);
        src.update()?;
        let mut pkg = match src.download(package)? {
            MaybePackage::Ready(pkg) => pkg,
            MaybePackage::Download { .. } => unreachable!(),
        };

        // After we've loaded the package configure its summary's `checksum`
        // field with the checksum we know for this `PackageId`.
        let req = VersionReq::exact(package.version());
        let summary_with_cksum = self
            .index
            .summaries(package.name(), &req, &mut *self.ops)?
            .map(|s| s.summary.clone())
            .next()
            .expect("summary not found");
        if let Some(cksum) = summary_with_cksum.checksum() {
            pkg.manifest_mut()
                .summary_mut()
                .set_checksum(cksum.to_string());
        }

        Ok(pkg)
    }
}

impl<'cfg> Source for RegistrySource<'cfg> {
    fn query(&mut self, dep: &Dependency, f: &mut dyn FnMut(Summary)) -> CargoResult<()> {
        // If this is a precise dependency, then it came from a lock file and in
        // theory the registry is known to contain this version. If, however, we
        // come back with no summaries, then our registry may need to be
        // updated, so we fall back to performing a lazy update.
        if dep.source_id().precise().is_some() && !self.updated {
            debug!("attempting query without update");
            let mut called = false;
            self.index
                .query_inner(dep, &mut *self.ops, &self.yanked_whitelist, &mut |s| {
                    if dep.matches(&s) {
                        called = true;
                        f(s);
                    }
                })?;
            if called {
                return Ok(());
            } else {
                debug!("falling back to an update");
                self.do_update()?;
            }
        }

        self.index
            .query_inner(dep, &mut *self.ops, &self.yanked_whitelist, &mut |s| {
                if dep.matches(&s) {
                    f(s);
                }
            })
    }

    fn fuzzy_query(&mut self, dep: &Dependency, f: &mut dyn FnMut(Summary)) -> CargoResult<()> {
        self.index
            .query_inner(dep, &mut *self.ops, &self.yanked_whitelist, f)
    }

    fn supports_checksums(&self) -> bool {
        true
    }

    fn requires_precise(&self) -> bool {
        false
    }

    fn source_id(&self) -> SourceId {
        self.source_id
    }

    fn update(&mut self) -> CargoResult<()> {
        // If we have an imprecise version then we don't know what we're going
        // to look for, so we always attempt to perform an update here.
        //
        // If we have a precise version, then we'll update lazily during the
        // querying phase. Note that precise in this case is only
        // `Some("locked")` as other `Some` values indicate a `cargo update
        // --precise` request
        if self.source_id.precise() != Some("locked") {
            self.do_update()?;
        } else {
            debug!("skipping update due to locked registry");
        }
        Ok(())
    }

    fn download(&mut self, package: PackageId) -> CargoResult<MaybePackage> {
        let hash = self.index.hash(package, &mut *self.ops)?;
        match self.ops.download(package, hash)? {
            MaybeLock::Ready(file) => self.get_pkg(package, &file).map(MaybePackage::Ready),
            MaybeLock::Download { url, descriptor } => {
                Ok(MaybePackage::Download { url, descriptor })
            }
        }
    }

    fn finish_download(&mut self, package: PackageId, data: Vec<u8>) -> CargoResult<Package> {
        let hash = self.index.hash(package, &mut *self.ops)?;
        let file = self.ops.finish_download(package, hash, &data)?;
        self.get_pkg(package, &file)
    }

    fn fingerprint(&self, pkg: &Package) -> CargoResult<String> {
        Ok(pkg.package_id().version().to_string())
    }

    fn describe(&self) -> String {
        self.source_id.display_index()
    }

    fn add_to_yanked_whitelist(&mut self, pkgs: &[PackageId]) {
        self.yanked_whitelist.extend(pkgs);
    }

    fn is_yanked(&mut self, pkg: PackageId) -> CargoResult<bool> {
        if !self.updated {
            self.do_update()?;
        }
        self.index.is_yanked(pkg, &mut *self.ops)
    }
}
