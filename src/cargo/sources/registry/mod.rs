//! A `Source` for registry-based packages.
//!
//! # What's a Registry?
//!
//! [Registries] are central locations where packages can be uploaded to,
//! discovered, and searched for. The purpose of a registry is to have a
//! location that serves as permanent storage for versions of a crate over time.
//!
//! Compared to git sources (see [`GitSource`]), a registry provides many
//! packages as well as many versions simultaneously. Git sources can also
//! have commits deleted through rebasings where registries cannot have their
//! versions deleted.
//!
//! In Cargo, [`RegistryData`] is an abstraction over each kind of actual
//! registry, and [`RegistrySource`] connects those implementations to
//! [`Source`] trait. Two prominent features these abstractions provide are
//!
//! * A way to query the metadata of a package from a registry. The metadata
//!   comes from the index.
//! * A way to download package contents (a.k.a source files) that are required
//!   when building the package itself.
//!
//! We'll cover each functionality later.
//!
//! [Registries]: https://doc.rust-lang.org/nightly/cargo/reference/registries.html
//! [`GitSource`]: super::GitSource
//!
//! # Different Kinds of Registries
//!
//! Cargo provides multiple kinds of registries. Each of them serves the index
//! and package contents in a slightly different way. Namely,
//!
//! * [`LocalRegistry`] --- Serves the index and package contents entirely on
//!   a local filesystem.
//! * [`RemoteRegistry`] --- Serves the index ahead of time from a Git
//!   repository, and package contents are downloaded as needed.
//! * [`HttpRegistry`] --- Serves both the index and package contents on demand
//!   over a HTTP-based registry API. This is the default starting from 1.70.0.
//!
//! Each registry has its own [`RegistryData`] implementation, and can be
//! created from either [`RegistrySource::local`] or [`RegistrySource::remote`].
//!
//! [`LocalRegistry`]: local::LocalRegistry
//! [`RemoteRegistry`]: remote::RemoteRegistry
//! [`HttpRegistry`]: http_remote::HttpRegistry
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
//! To solve the problem, a registry must provide an index of package metadata.
//! The index of a registry is essentially an easily query-able version of the
//! registry's database for a list of versions of a package as well as a list
//! of dependencies for each version. The exact format of the index is
//! described later.
//!
//! See the [`index`] module for topics about the management, parsing, caching,
//! and versioning for the on-disk index.
//!
//! ## The Format of The Index
//!
//! The index is a store for the list of versions for all packages known, so its
//! format on disk is optimized slightly to ensure that `ls registry` doesn't
//! produce a list of all packages ever known. The index also wants to ensure
//! that there's not a million files which may actually end up hitting
//! filesystem limits at some point. To this end, a few decisions were made
//! about the format of the registry:
//!
//! 1. Each crate will have one file corresponding to it. Each version for a
//!    crate will just be a line in this file (see [`IndexPackage`] for its
//!    representation).
//! 2. There will be two tiers of directories for crate names, under which
//!    crates corresponding to those tiers will be located.
//!    (See [`cargo_util::registry::make_dep_path`] for the implementation of
//!    this layout hierarchy.)
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
//! corresponding to the registry (see [`RegistryConfig`] below).
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
//! See [The Cargo Book: Registry Index][registry-index] for the public
//! interface on the index format.
//!
//! [registry-index]: https://doc.rust-lang.org/nightly/cargo/reference/registry-index.html
//!
//! ## The Index Files
//!
//! Each file in the index is the history of one crate over time. Each line in
//! the file corresponds to one version of a crate, stored in JSON format (see
//! the [`IndexPackage`] structure).
//!
//! As new versions are published, new lines are appended to this file. **The
//! only modifications to this file that should happen over time are yanks of a
//! particular version.**
//!
//! # Downloading Packages
//!
//! The purpose of the index was to provide an efficient method to resolve the
//! dependency graph for a package. After resolution has been performed, we need
//! to download the contents of packages so we can read the full manifest and
//! build the source code.
//!
//! To accomplish this, [`RegistryData::download`] will "make" an HTTP request
//! per-package requested to download tarballs into a local cache. These
//! tarballs will then be unpacked into a destination folder.
//!
//! Note that because versions uploaded to the registry are frozen forever that
//! the HTTP download and unpacking can all be skipped if the version has
//! already been downloaded and unpacked. This caching allows us to only
//! download a package when absolutely necessary.
//!
//! # Filesystem Hierarchy
//!
//! Overall, the `$HOME/.cargo` looks like this when talking about the registry
//! (remote registries, specifically):
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
//!     # This folder is a cache for all downloaded tarballs (`.crate` file)
//!     # from a registry. Once downloaded and verified, a tarball never changes.
//!     cache/
//!         registry1-<hash>/<pkg>-<version>.crate
//!         ...
//!
//!     # Location in which all tarballs are unpacked. Each tarball is known to
//!     # be frozen after downloading, so transitively this folder is also
//!     # frozen once its unpacked (it's never unpacked again)
//!     # CAVEAT: They are not read-only. See rust-lang/cargo#9455.
//!     src/
//!         registry1-<hash>/<pkg>-<version>/...
//!         ...
//! ```
//!
//! [`IndexPackage`]: index::IndexPackage

use std::collections::HashSet;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io;
use std::io::Read;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::task::{ready, Poll};

use anyhow::Context as _;
use cargo_util::paths::{self, exclude_from_backups_and_indexing};
use flate2::read::GzDecoder;
use serde::Deserialize;
use serde::Serialize;
use tar::Archive;
use tracing::debug;

use crate::core::dependency::Dependency;
use crate::core::{Package, PackageId, SourceId, Summary};
use crate::sources::source::MaybePackage;
use crate::sources::source::QueryKind;
use crate::sources::source::Source;
use crate::sources::PathSource;
use crate::util::cache_lock::CacheLockMode;
use crate::util::hex;
use crate::util::interning::InternedString;
use crate::util::network::PollExt;
use crate::util::{restricted_names, CargoResult, Config, Filesystem, LimitErrorReader};

/// The `.cargo-ok` file is used to track if the source is already unpacked.
/// See [`RegistrySource::unpack_package`] for more.
///
/// Not to be confused with `.cargo-ok` file in git sources.
const PACKAGE_SOURCE_LOCK: &str = ".cargo-ok";

pub const CRATES_IO_INDEX: &str = "https://github.com/rust-lang/crates.io-index";
pub const CRATES_IO_HTTP_INDEX: &str = "sparse+https://index.crates.io/";
pub const CRATES_IO_REGISTRY: &str = "crates-io";
pub const CRATES_IO_DOMAIN: &str = "crates.io";

/// The content inside `.cargo-ok`.
/// See [`RegistrySource::unpack_package`] for more.
#[derive(Deserialize, Serialize)]
struct LockMetadata {
    /// The version of `.cargo-ok` file
    v: u32,
}

/// A [`Source`] implementation for a local or a remote registry.
///
/// This contains common functionality that is shared between each registry
/// kind, with the registry-specific logic implemented as part of the
/// [`RegistryData`] trait referenced via the `ops` field.
///
/// For general concepts of registries, see the [module-level documentation](crate::sources::registry).
pub struct RegistrySource<'cfg> {
    /// The unique identifier of this source.
    source_id: SourceId,
    /// The path where crate files are extracted (`$CARGO_HOME/registry/src/$REG-HASH`).
    src_path: Filesystem,
    /// Local reference to [`Config`] for convenience.
    config: &'cfg Config,
    /// Abstraction for interfacing to the different registry kinds.
    ops: Box<dyn RegistryData + 'cfg>,
    /// Interface for managing the on-disk index.
    index: index::RegistryIndex<'cfg>,
    /// A set of packages that should be allowed to be used, even if they are
    /// yanked.
    ///
    /// This is populated from the entries in `Cargo.lock` to ensure that
    /// `cargo update somepkg` won't unlock yanked entries in `Cargo.lock`.
    /// Otherwise, the resolver would think that those entries no longer
    /// exist, and it would trigger updates to unrelated packages.
    yanked_whitelist: HashSet<PackageId>,
}

/// The [`config.json`] file stored in the index.
///
/// The config file may look like:
///
/// ```json
/// {
///     "dl": "https://example.com/api/{crate}/{version}/download",
///     "api": "https://example.com/api",
///     "auth-required": false             # unstable feature (RFC 3139)
/// }
/// ```
///
/// [`config.json`]: https://doc.rust-lang.org/nightly/cargo/reference/registry-index.html#index-configuration
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct RegistryConfig {
    /// Download endpoint for all crates.
    ///
    /// The string is a template which will generate the download URL for the
    /// tarball of a specific version of a crate. The substrings `{crate}` and
    /// `{version}` will be replaced with the crate's name and version
    /// respectively.  The substring `{prefix}` will be replaced with the
    /// crate's prefix directory name, and the substring `{lowerprefix}` will
    /// be replaced with the crate's prefix directory name converted to
    /// lowercase. The substring `{sha256-checksum}` will be replaced with the
    /// crate's sha256 checksum.
    ///
    /// For backwards compatibility, if the string does not contain any
    /// markers (`{crate}`, `{version}`, `{prefix}`, or `{lowerprefix}`), it
    /// will be extended with `/{crate}/{version}/download` to
    /// support registries like crates.io which were created before the
    /// templating setup was created.
    ///
    /// For more on the template of the download URL, see [Index Configuration](
    /// https://doc.rust-lang.org/nightly/cargo/reference/registry-index.html#index-configuration).
    pub dl: String,

    /// API endpoint for the registry. This is what's actually hit to perform
    /// operations like yanks, owner modifications, publish new crates, etc.
    /// If this is None, the registry does not support API commands.
    pub api: Option<String>,

    /// Whether all operations require authentication. See [RFC 3139].
    ///
    /// [RFC 3139]: https://rust-lang.github.io/rfcs/3139-cargo-alternative-registry-auth.html
    #[serde(default)]
    pub auth_required: bool,
}

/// Result from loading data from a registry.
pub enum LoadResponse {
    /// The cache is valid. The cached data should be used.
    CacheValid,

    /// The cache is out of date. Returned data should be used.
    Data {
        raw_data: Vec<u8>,
        /// Version of this data to determine whether it is out of date.
        index_version: Option<String>,
    },

    /// The requested crate was found.
    NotFound,
}

/// An abstract interface to handle both a local and and remote registry.
///
/// This allows [`RegistrySource`] to abstractly handle each registry kind.
///
/// For general concepts of registries, see the [module-level documentation](crate::sources::registry).
pub trait RegistryData {
    /// Performs initialization for the registry.
    ///
    /// This should be safe to call multiple times, the implementation is
    /// expected to not do any work if it is already prepared.
    fn prepare(&self) -> CargoResult<()>;

    /// Returns the path to the index.
    ///
    /// Note that different registries store the index in different formats
    /// (remote = git, http & local = files).
    fn index_path(&self) -> &Filesystem;

    /// Loads the JSON for a specific named package from the index.
    ///
    /// * `root` is the root path to the index.
    /// * `path` is the relative path to the package to load (like `ca/rg/cargo`).
    /// * `index_version` is the version of the requested crate data currently
    ///    in cache. This is useful for checking if a local cache is outdated.
    fn load(
        &mut self,
        root: &Path,
        path: &Path,
        index_version: Option<&str>,
    ) -> Poll<CargoResult<LoadResponse>>;

    /// Loads the `config.json` file and returns it.
    ///
    /// Local registries don't have a config, and return `None`.
    fn config(&mut self) -> Poll<CargoResult<Option<RegistryConfig>>>;

    /// Invalidates locally cached data.
    fn invalidate_cache(&mut self);

    /// If quiet, the source should not display any progress or status messages.
    fn set_quiet(&mut self, quiet: bool);

    /// Is the local cached data up-to-date?
    fn is_updated(&self) -> bool;

    /// Prepare to start downloading a `.crate` file.
    ///
    /// Despite the name, this doesn't actually download anything. If the
    /// `.crate` is already downloaded, then it returns [`MaybeLock::Ready`].
    /// If it hasn't been downloaded, then it returns [`MaybeLock::Download`]
    /// which contains the URL to download. The [`crate::core::package::Downloads`]
    /// system handles the actual download process. After downloading, it
    /// calls [`Self::finish_download`] to save the downloaded file.
    ///
    /// `checksum` is currently only used by local registries to verify the
    /// file contents (because local registries never actually download
    /// anything). Remote registries will validate the checksum in
    /// `finish_download`. For already downloaded `.crate` files, it does not
    /// validate the checksum, assuming the filesystem does not suffer from
    /// corruption or manipulation.
    fn download(&mut self, pkg: PackageId, checksum: &str) -> CargoResult<MaybeLock>;

    /// Finish a download by saving a `.crate` file to disk.
    ///
    /// After [`crate::core::package::Downloads`] has finished a download,
    /// it will call this to save the `.crate` file. This is only relevant
    /// for remote registries. This should validate the checksum and save
    /// the given data to the on-disk cache.
    ///
    /// Returns a [`File`] handle to the `.crate` file, positioned at the start.
    fn finish_download(&mut self, pkg: PackageId, checksum: &str, data: &[u8])
        -> CargoResult<File>;

    /// Returns whether or not the `.crate` file is already downloaded.
    fn is_crate_downloaded(&self, _pkg: PackageId) -> bool {
        true
    }

    /// Validates that the global package cache lock is held.
    ///
    /// Given the [`Filesystem`], this will make sure that the package cache
    /// lock is held. If not, it will panic. See
    /// [`Config::acquire_package_cache_lock`] for acquiring the global lock.
    ///
    /// Returns the [`Path`] to the [`Filesystem`].
    fn assert_index_locked<'a>(&self, path: &'a Filesystem) -> &'a Path;

    /// Block until all outstanding Poll::Pending requests are Poll::Ready.
    fn block_until_ready(&mut self) -> CargoResult<()>;
}

/// The status of [`RegistryData::download`] which indicates if a `.crate`
/// file has already been downloaded, or if not then the URL to download.
pub enum MaybeLock {
    /// The `.crate` file is already downloaded. [`File`] is a handle to the
    /// opened `.crate` file on the filesystem.
    Ready(File),
    /// The `.crate` file is not downloaded, here's the URL to download it from.
    ///
    /// `descriptor` is just a text string to display to the user of what is
    /// being downloaded.
    Download {
        url: String,
        descriptor: String,
        authorization: Option<String>,
    },
}

mod download;
mod http_remote;
mod index;
mod local;
mod remote;

/// Generates a unique name for [`SourceId`] to have a unique path to put their
/// index files.
fn short_name(id: SourceId, is_shallow: bool) -> String {
    let hash = hex::short_hash(&id);
    let ident = id.url().host_str().unwrap_or("").to_string();
    let mut name = format!("{}-{}", ident, hash);
    if is_shallow {
        name.push_str("-shallow");
    }
    name
}

impl<'cfg> RegistrySource<'cfg> {
    /// Creates a [`Source`] of a "remote" registry.
    /// It could be either an HTTP-based [`http_remote::HttpRegistry`] or
    /// a Git-based [`remote::RemoteRegistry`].
    ///
    /// * `yanked_whitelist` --- Packages allowed to be used, even if they are yanked.
    pub fn remote(
        source_id: SourceId,
        yanked_whitelist: &HashSet<PackageId>,
        config: &'cfg Config,
    ) -> CargoResult<RegistrySource<'cfg>> {
        assert!(source_id.is_remote_registry());
        let name = short_name(
            source_id,
            config
                .cli_unstable()
                .gitoxide
                .map_or(false, |gix| gix.fetch && gix.shallow_index)
                && !source_id.is_sparse(),
        );
        let ops = if source_id.is_sparse() {
            Box::new(http_remote::HttpRegistry::new(source_id, config, &name)?) as Box<_>
        } else {
            Box::new(remote::RemoteRegistry::new(source_id, config, &name)) as Box<_>
        };

        Ok(RegistrySource::new(
            source_id,
            config,
            &name,
            ops,
            yanked_whitelist,
        ))
    }

    /// Creates a [`Source`] of a local registry, with [`local::LocalRegistry`] under the hood.
    ///
    /// * `path` --- The root path of a local registry on the file system.
    /// * `yanked_whitelist` --- Packages allowed to be used, even if they are yanked.
    pub fn local(
        source_id: SourceId,
        path: &Path,
        yanked_whitelist: &HashSet<PackageId>,
        config: &'cfg Config,
    ) -> RegistrySource<'cfg> {
        let name = short_name(source_id, false);
        let ops = local::LocalRegistry::new(path, config, &name);
        RegistrySource::new(source_id, config, &name, Box::new(ops), yanked_whitelist)
    }

    /// Creates a source of a registry. This is a inner helper function.
    ///
    /// * `name` --- Name of a path segment which may affect where `.crate`
    ///   tarballs, the registry index and cache are stored. Expect to be unique.
    /// * `ops` --- The underlying [`RegistryData`] type.
    /// * `yanked_whitelist` --- Packages allowed to be used, even if they are yanked.
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
            index: index::RegistryIndex::new(source_id, ops.index_path(), config),
            yanked_whitelist: yanked_whitelist.clone(),
            ops,
        }
    }

    /// Decode the [configuration](RegistryConfig) stored within the registry.
    ///
    /// This requires that the index has been at least checked out.
    pub fn config(&mut self) -> Poll<CargoResult<Option<RegistryConfig>>> {
        self.ops.config()
    }

    /// Unpacks a downloaded package into a location where it's ready to be
    /// compiled.
    ///
    /// No action is taken if the source looks like it's already unpacked.
    ///
    /// # History of interruption detection with `.cargo-ok` file
    ///
    /// Cargo has always included a `.cargo-ok` file ([`PACKAGE_SOURCE_LOCK`])
    /// to detect if extraction was interrupted, but it was originally empty.
    ///
    /// In 1.34, Cargo was changed to create the `.cargo-ok` file before it
    /// started extraction to implement fine-grained locking. After it was
    /// finished extracting, it wrote two bytes to indicate it was complete.
    /// It would use the length check to detect if it was possibly interrupted.
    ///
    /// In 1.36, Cargo changed to not use fine-grained locking, and instead used
    /// a global lock. The use of `.cargo-ok` was no longer needed for locking
    /// purposes, but was kept to detect when extraction was interrupted.
    ///
    /// In 1.49, Cargo changed to not create the `.cargo-ok` file before it
    /// started extraction to deal with `.crate` files that inexplicably had
    /// a `.cargo-ok` file in them.
    ///
    /// In 1.64, Cargo changed to detect `.crate` files with `.cargo-ok` files
    /// in them in response to [CVE-2022-36113], which dealt with malicious
    /// `.crate` files making `.cargo-ok` a symlink causing cargo to write "ok"
    /// to any arbitrary file on the filesystem it has permission to.
    ///
    /// In 1.71, `.cargo-ok` changed to contain a JSON `{ v: 1 }` to indicate
    /// the version of it. A failure of parsing will result in a heavy-hammer
    /// approach that unpacks the `.crate` file again. This is in response to a
    /// security issue that the unpacking didn't respect umask on Unix systems.
    ///
    /// This is all a long-winded way of explaining the circumstances that might
    /// cause a directory to contain a `.cargo-ok` file that is empty or
    /// otherwise corrupted. Either this was extracted by a version of Rust
    /// before 1.34, in which case everything should be fine. However, an empty
    /// file created by versions 1.36 to 1.49 indicates that the extraction was
    /// interrupted and that we need to start again.
    ///
    /// Another possibility is that the filesystem is simply corrupted, in
    /// which case deleting the directory might be the safe thing to do. That
    /// is probably unlikely, though.
    ///
    /// To be safe, we deletes the directory and starts over again if an empty
    /// `.cargo-ok` file is found.
    ///
    /// [CVE-2022-36113]: https://blog.rust-lang.org/2022/09/14/cargo-cves.html#arbitrary-file-corruption-cve-2022-36113
    fn unpack_package(&self, pkg: PackageId, tarball: &File) -> CargoResult<PathBuf> {
        let package_dir = format!("{}-{}", pkg.name(), pkg.version());
        let dst = self.src_path.join(&package_dir);
        let path = dst.join(PACKAGE_SOURCE_LOCK);
        let path = self
            .config
            .assert_package_cache_locked(CacheLockMode::DownloadExclusive, &path);
        let unpack_dir = path.parent().unwrap();
        match fs::read_to_string(path) {
            Ok(ok) => match serde_json::from_str::<LockMetadata>(&ok) {
                Ok(lock_meta) if lock_meta.v == 1 => {
                    return Ok(unpack_dir.to_path_buf());
                }
                _ => {
                    if ok == "ok" {
                        tracing::debug!("old `ok` content found, clearing cache");
                    } else {
                        tracing::warn!("unrecognized .cargo-ok content, clearing cache: {ok}");
                    }
                    // See comment of `unpack_package` about why removing all stuff.
                    paths::remove_dir_all(dst.as_path_unlocked())?;
                }
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => anyhow::bail!("unable to read .cargo-ok file at {path:?}: {e}"),
        }
        dst.create_dir()?;
        let mut tar = {
            let size_limit = max_unpack_size(self.config, tarball.metadata()?.len());
            let gz = GzDecoder::new(tarball);
            let gz = LimitErrorReader::new(gz, size_limit);
            let mut tar = Archive::new(gz);
            set_mask(&mut tar);
            tar
        };
        let prefix = unpack_dir.file_name().unwrap();
        let parent = unpack_dir.parent().unwrap();
        for entry in tar.entries()? {
            let mut entry = entry.with_context(|| "failed to iterate over archive")?;
            let entry_path = entry
                .path()
                .with_context(|| "failed to read entry path")?
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
            // Prevent unpacking the lockfile from the crate itself.
            if entry_path
                .file_name()
                .map_or(false, |p| p == PACKAGE_SOURCE_LOCK)
            {
                continue;
            }
            // Unpacking failed
            let mut result = entry.unpack_in(parent).map_err(anyhow::Error::from);
            if cfg!(windows) && restricted_names::is_windows_reserved_path(&entry_path) {
                result = result.with_context(|| {
                    format!(
                        "`{}` appears to contain a reserved Windows path, \
                        it cannot be extracted on Windows",
                        entry_path.display()
                    )
                });
            }
            result
                .with_context(|| format!("failed to unpack entry at `{}`", entry_path.display()))?;
        }

        // Now that we've finished unpacking, create and write to the lock file to indicate that
        // unpacking was successful.
        let mut ok = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(&path)
            .with_context(|| format!("failed to open `{}`", path.display()))?;

        let lock_meta = LockMetadata { v: 1 };
        write!(ok, "{}", serde_json::to_string(&lock_meta).unwrap())?;

        Ok(unpack_dir.to_path_buf())
    }

    /// Turns the downloaded `.crate` tarball file into a [`Package`].
    ///
    /// This unconditionally sets checksum for the returned package, so it
    /// should only be called after doing integrity check. That is to say,
    /// you need to call either [`RegistryData::download`] or
    /// [`RegistryData::finish_download`] before calling this method.
    fn get_pkg(&mut self, package: PackageId, path: &File) -> CargoResult<Package> {
        let path = self
            .unpack_package(package, path)
            .with_context(|| format!("failed to unpack package `{}`", package))?;
        let mut src = PathSource::new(&path, self.source_id, self.config);
        src.update()?;
        let mut pkg = match src.download(package)? {
            MaybePackage::Ready(pkg) => pkg,
            MaybePackage::Download { .. } => unreachable!(),
        };

        // After we've loaded the package configure its summary's `checksum`
        // field with the checksum we know for this `PackageId`.
        let cksum = self
            .index
            .hash(package, &mut *self.ops)
            .expect("a downloaded dep now pending!?")
            .expect("summary not found");
        pkg.manifest_mut()
            .summary_mut()
            .set_checksum(cksum.to_string());

        Ok(pkg)
    }
}

impl<'cfg> Source for RegistrySource<'cfg> {
    fn query(
        &mut self,
        dep: &Dependency,
        kind: QueryKind,
        f: &mut dyn FnMut(Summary),
    ) -> Poll<CargoResult<()>> {
        // If this is a precise dependency, then it came from a lock file and in
        // theory the registry is known to contain this version. If, however, we
        // come back with no summaries, then our registry may need to be
        // updated, so we fall back to performing a lazy update.
        if kind == QueryKind::Exact && dep.source_id().has_precise() && !self.ops.is_updated() {
            debug!("attempting query without update");
            let mut called = false;
            ready!(self.index.query_inner(
                dep.package_name(),
                dep.version_req(),
                &mut *self.ops,
                &self.yanked_whitelist,
                &mut |s| {
                    if dep.matches(&s) {
                        called = true;
                        f(s);
                    }
                },
            ))?;
            if called {
                Poll::Ready(Ok(()))
            } else {
                debug!("falling back to an update");
                self.invalidate_cache();
                Poll::Pending
            }
        } else {
            let mut called = false;
            ready!(self.index.query_inner(
                dep.package_name(),
                dep.version_req(),
                &mut *self.ops,
                &self.yanked_whitelist,
                &mut |s| {
                    let matched = match kind {
                        QueryKind::Exact => dep.matches(&s),
                        QueryKind::Fuzzy => true,
                    };
                    if matched {
                        f(s);
                        called = true;
                    }
                }
            ))?;
            if called {
                return Poll::Ready(Ok(()));
            }
            let mut any_pending = false;
            if kind == QueryKind::Fuzzy {
                // Attempt to handle misspellings by searching for a chain of related
                // names to the original name. The resolver will later
                // reject any candidates that have the wrong name, and with this it'll
                // along the way produce helpful "did you mean?" suggestions.
                // For now we only try the canonical lysing `-` to `_` and vice versa.
                // More advanced fuzzy searching become in the future.
                for name_permutation in [
                    dep.package_name().replace('-', "_"),
                    dep.package_name().replace('_', "-"),
                ] {
                    let name_permutation = InternedString::new(&name_permutation);
                    if name_permutation == dep.package_name() {
                        continue;
                    }
                    any_pending |= self
                        .index
                        .query_inner(
                            name_permutation,
                            dep.version_req(),
                            &mut *self.ops,
                            &self.yanked_whitelist,
                            f,
                        )?
                        .is_pending();
                }
            }
            if any_pending {
                Poll::Pending
            } else {
                Poll::Ready(Ok(()))
            }
        }
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

    fn invalidate_cache(&mut self) {
        self.index.clear_summaries_cache();
        self.ops.invalidate_cache();
    }

    fn set_quiet(&mut self, quiet: bool) {
        self.ops.set_quiet(quiet);
    }

    fn download(&mut self, package: PackageId) -> CargoResult<MaybePackage> {
        let hash = loop {
            match self.index.hash(package, &mut *self.ops)? {
                Poll::Pending => self.block_until_ready()?,
                Poll::Ready(hash) => break hash,
            }
        };
        match self.ops.download(package, hash)? {
            MaybeLock::Ready(file) => self.get_pkg(package, &file).map(MaybePackage::Ready),
            MaybeLock::Download {
                url,
                descriptor,
                authorization,
            } => Ok(MaybePackage::Download {
                url,
                descriptor,
                authorization,
            }),
        }
    }

    fn finish_download(&mut self, package: PackageId, data: Vec<u8>) -> CargoResult<Package> {
        let hash = loop {
            match self.index.hash(package, &mut *self.ops)? {
                Poll::Pending => self.block_until_ready()?,
                Poll::Ready(hash) => break hash,
            }
        };
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

    fn is_yanked(&mut self, pkg: PackageId) -> Poll<CargoResult<bool>> {
        self.index.is_yanked(pkg, &mut *self.ops)
    }

    fn block_until_ready(&mut self) -> CargoResult<()> {
        // Before starting to work on the registry, make sure that
        // `<cargo_home>/registry` is marked as excluded from indexing and
        // backups. Older versions of Cargo didn't do this, so we do it here
        // regardless of whether `<cargo_home>` exists.
        //
        // This does not use `create_dir_all_excluded_from_backups_atomic` for
        // the same reason: we want to exclude it even if the directory already
        // exists.
        //
        // IO errors in creating and marking it are ignored, e.g. in case we're on a
        // read-only filesystem.
        let registry_base = self.config.registry_base_path();
        let _ = registry_base.create_dir();
        exclude_from_backups_and_indexing(&registry_base.into_path_unlocked());

        self.ops.block_until_ready()
    }
}

impl RegistryConfig {
    /// File name of [`RegistryConfig`].
    const NAME: &'static str = "config.json";
}

/// Get the maximum upack size that Cargo permits
/// based on a given `size` of your compressed file.
///
/// Returns the larger one between `size * max compression ratio`
/// and a fixed max unpacked size.
///
/// In reality, the compression ratio usually falls in the range of 2:1 to 10:1.
/// We choose 20:1 to cover almost all possible cases hopefully.
/// Any ratio higher than this is considered as a zip bomb.
///
/// In the future we might want to introduce a configurable size.
///
/// Some of the real world data from common compression algorithms:
///
/// * <https://www.zlib.net/zlib_tech.html>
/// * <https://cran.r-project.org/web/packages/brotli/vignettes/brotli-2015-09-22.pdf>
/// * <https://blog.cloudflare.com/results-experimenting-brotli/>
/// * <https://tukaani.org/lzma/benchmarks.html>
fn max_unpack_size(config: &Config, size: u64) -> u64 {
    const SIZE_VAR: &str = "__CARGO_TEST_MAX_UNPACK_SIZE";
    const RATIO_VAR: &str = "__CARGO_TEST_MAX_UNPACK_RATIO";
    const MAX_UNPACK_SIZE: u64 = 512 * 1024 * 1024; // 512 MiB
    const MAX_COMPRESSION_RATIO: usize = 20; // 20:1

    let max_unpack_size = if cfg!(debug_assertions) && config.get_env(SIZE_VAR).is_ok() {
        // For integration test only.
        config
            .get_env(SIZE_VAR)
            .unwrap()
            .parse()
            .expect("a max unpack size in bytes")
    } else {
        MAX_UNPACK_SIZE
    };
    let max_compression_ratio = if cfg!(debug_assertions) && config.get_env(RATIO_VAR).is_ok() {
        // For integration test only.
        config
            .get_env(RATIO_VAR)
            .unwrap()
            .parse()
            .expect("a max compression ratio in bytes")
    } else {
        MAX_COMPRESSION_RATIO
    };

    u64::max(max_unpack_size, size * max_compression_ratio as u64)
}

/// Set the current [`umask`] value for the given tarball. No-op on non-Unix
/// platforms.
///
/// On Windows, tar only looks at user permissions and tries to set the "read
/// only" attribute, so no-op as well.
///
/// [`umask`]: https://man7.org/linux/man-pages/man2/umask.2.html
#[allow(unused_variables)]
fn set_mask<R: Read>(tar: &mut Archive<R>) {
    #[cfg(unix)]
    tar.set_mask(crate::util::get_umask());
}
