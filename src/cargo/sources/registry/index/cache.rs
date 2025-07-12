//! A layer of on-disk index cache for performance.
//!
//! One important aspect of the index is that we want to optimize the "happy
//! path" as much as possible. Whenever you type `cargo build` Cargo will
//! *always* reparse the registry and learn about dependency information. This
//! is done because Cargo needs to learn about the upstream crates.io crates
//! that you're using and ensure that the preexisting `Cargo.lock` still matches
//! the current state of the world.
//!
//! Consequently, Cargo "null builds" (the index that Cargo adds to each build
//! itself) need to be fast when accessing the index. The primary performance
//! optimization here is to avoid parsing JSON blobs from the registry if we
//! don't need them. Most secondary optimizations are centered around removing
//! allocations and such, but avoiding parsing JSON is the #1 optimization.
//!
//! When we get queries from the resolver we're given a [`Dependency`]. This
//! dependency in turn has a version requirement, and with lock files that
//! already exist these version requirements are exact version requirements
//! `=a.b.c`. This means that we in theory only need to parse one line of JSON
//! per query in the registry, the one that matches version `a.b.c`.
//!
//! The crates.io index, however, is not amenable to this form of query. Instead
//! the crates.io index simply is a file where each line is a JSON blob, aka
//! [`IndexPackage`]. To learn about the versions in each JSON blob we would
//! need to parse the JSON via [`IndexSummary::parse`], defeating the purpose
//! of trying to parse as little as possible.
//!
//! > Note that as a small aside even *loading* the JSON from the registry is
//! > actually pretty slow. For crates.io and [`RemoteRegistry`] we don't
//! > actually check out the git index on disk because that takes quite some
//! > time and is quite large. Instead we use `libgit2` to read the JSON from
//! > the raw git objects. This in turn can be slow (aka show up high in
//! > profiles) because libgit2 has to do deflate decompression and such.
//!
//! To solve all these issues a strategy is employed here where Cargo basically
//! creates an index into the index. The first time a package is queried about
//! (first time being for an entire computer) Cargo will load the contents
//! (slowly via libgit2) from the registry. It will then (slowly) parse every
//! single line to learn about its versions. Afterwards, however, Cargo will
//! emit a new file (a cache, representing as [`SummariesCache`]) which is
//! amenable for speedily parsing in future invocations.
//!
//! This cache file is currently organized by basically having the semver
//! version extracted from each JSON blob. That way Cargo can quickly and
//! easily parse all versions contained and which JSON blob they're associated
//! with. The JSON blob then doesn't actually need to get parsed unless the
//! version is parsed.
//!
//! Altogether the initial measurements of this shows a massive improvement for
//! Cargo null build performance. It's expected that the improvements earned
//! here will continue to grow over time in the sense that the previous
//! implementation (parse all lines each time) actually continues to slow down
//! over time as new versions of a crate are published. In any case when first
//! implemented a null build of Cargo itself would parse 3700 JSON blobs from
//! the registry and load 150 blobs from git. Afterwards it parses 150 JSON
//! blobs and loads 0 files git. Removing 200ms or more from Cargo's startup
//! time is certainly nothing to sneeze at!
//!
//! Note that this is just a high-level overview, there's of course lots of
//! details like invalidating caches and whatnot which are handled below, but
//! hopefully those are more obvious inline in the code itself.
//!
//! [`Dependency`]: crate::core::Dependency
//! [`IndexPackage`]: super::IndexPackage
//! [`IndexSummary::parse`]: super::IndexSummary::parse
//! [`RemoteRegistry`]: crate::sources::registry::remote::RemoteRegistry

use std::cell::RefCell;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::str;

use anyhow::bail;
use cargo_util::registry::make_dep_path;
use semver::Version;

use crate::CargoResult;
use crate::GlobalContext;
use crate::util::Filesystem;
use crate::util::cache_lock::CacheLockMode;

use super::INDEX_V_MAX;
use super::split;

/// The current version of [`SummariesCache`].
const CURRENT_CACHE_VERSION: u8 = 3;

/// A representation of the cache on disk that Cargo maintains of summaries.
///
/// Cargo will initially parse all summaries in the registry and will then
/// serialize that into this form and place it in a new location on disk,
/// ensuring that access in the future is much speedier.
///
/// For serialization and deserialization of this on-disk index cache of
/// summaries, see [`SummariesCache::serialize`]  and [`SummariesCache::parse`].
///
/// # The format of the index cache
///
/// The idea of this format is that it's a very easy file for Cargo to parse in
/// future invocations. The read from disk should be fast and then afterwards
/// all we need to know is what versions correspond to which JSON blob.
///
/// Currently the format looks like:
///
/// ```text
/// +---------------+----------------------+--------------------+---+
/// | cache version | index schema version | index file version | 0 |
/// +---------------+----------------------+--------------------+---+
/// ```
///
/// followed by one or more (version + JSON blob) pairs...
///
/// ```text
/// +----------------+---+-----------+---+
/// | semver version | 0 | JSON blob | 0 | ...
/// +----------------+---+-----------+---+
/// ```
///
/// Each field represents:
///
/// * _cache version_ --- Intended to ensure that there's some level of
///   future compatibility against changes to this cache format so if different
///   versions of Cargo share the same cache they don't get too confused.
/// * _index schema version_ --- The schema version of the raw index file.
///   See [`IndexPackage::v`] for the detail.
/// * _index file version_ --- Tracks when a cache needs to be regenerated.
///   A cache regeneration is required whenever the index file itself updates.
/// * _semver version_ --- The version for each JSON blob. Extracted from the
///   blob for fast queries without parsing the entire blob.
/// * _JSON blob_ --- The actual metadata for each version of the package. It
///   has the same representation as [`IndexPackage`].
///
/// # Changes between each cache version
///
/// * `1`: The original version.
/// * `2`: Added the "index schema version" field so that if the index schema
///   changes, different versions of cargo won't get confused reading each
///   other's caches.
/// * `3`: Bumped the version to work around an issue where multiple versions of
///   a package were published that differ only by semver metadata. For
///   example, openssl-src 110.0.0 and 110.0.0+1.1.0f. Previously, the cache
///   would be incorrectly populated with two entries, both 110.0.0. After
///   this, the metadata will be correctly included. This isn't really a format
///   change, just a version bump to clear the incorrect cache entries. Note:
///   the index shouldn't allow these, but unfortunately crates.io doesn't
///   check it.
///
/// See [`CURRENT_CACHE_VERSION`] for the current cache version.
///
/// [`IndexPackage::v`]: super::IndexPackage::v
/// [`IndexPackage`]: super::IndexPackage
#[derive(Default)]
pub struct SummariesCache<'a> {
    /// JSON blobs of the summaries. Each JSON blob has a [`Version`] beside,
    /// so that Cargo can query a version without full JSON parsing.
    pub versions: Vec<(Version, &'a [u8])>,
    /// For cache invalidation, we tracks the index file version to determine
    /// when to regenerate the cache itself.
    pub index_version: &'a str,
}

impl<'a> SummariesCache<'a> {
    /// Deserializes an on-disk cache.
    pub fn parse(data: &'a [u8]) -> CargoResult<SummariesCache<'a>> {
        // NB: keep this method in sync with `serialize` below
        let (first_byte, rest) = data
            .split_first()
            .ok_or_else(|| anyhow::format_err!("malformed cache"))?;
        if *first_byte != CURRENT_CACHE_VERSION {
            bail!("looks like a different Cargo's cache, bailing out");
        }
        let index_v_bytes = rest
            .get(..4)
            .ok_or_else(|| anyhow::anyhow!("cache expected 4 bytes for index schema version"))?;
        let index_v = u32::from_le_bytes(index_v_bytes.try_into().unwrap());
        if index_v != INDEX_V_MAX {
            bail!(
                "index schema version {index_v} doesn't match the version I know ({INDEX_V_MAX})",
            );
        }
        let rest = &rest[4..];

        let mut iter = split(rest, 0);
        let last_index_update = if let Some(update) = iter.next() {
            str::from_utf8(update)?
        } else {
            bail!("malformed file");
        };
        let mut ret = SummariesCache::default();
        ret.index_version = last_index_update;
        while let Some(version) = iter.next() {
            let version = str::from_utf8(version)?;
            let version = Version::parse(version)?;
            let summary = iter.next().unwrap();
            ret.versions.push((version, summary));
        }
        Ok(ret)
    }

    /// Serializes itself with a given `index_version`.
    pub fn serialize(&self, index_version: &str) -> Vec<u8> {
        // NB: keep this method in sync with `parse` above
        let size = self
            .versions
            .iter()
            .map(|(_version, data)| 10 + data.len())
            .sum();
        let mut contents = Vec::with_capacity(size);
        contents.push(CURRENT_CACHE_VERSION);
        contents.extend(&u32::to_le_bytes(INDEX_V_MAX));
        contents.extend_from_slice(index_version.as_bytes());
        contents.push(0);
        for (version, data) in self.versions.iter() {
            contents.extend_from_slice(version.to_string().as_bytes());
            contents.push(0);
            contents.extend_from_slice(data);
            contents.push(0);
        }
        contents
    }
}

/// Manages the on-disk index caches.
pub struct CacheManager<'gctx> {
    /// The root path where caches are located.
    cache_root: Filesystem,
    /// [`GlobalContext`] reference for convenience.
    gctx: &'gctx GlobalContext,
    /// Keeps track of if we have sent a warning message if there was an error updating the cache.
    /// The motivation is to avoid warning spam if the cache is not writable.
    has_warned: RefCell<bool>,
}

impl<'gctx> CacheManager<'gctx> {
    /// Creates a new instance of the on-disk index cache manager.
    ///
    /// `root` --- The root path where caches are located.
    pub fn new(cache_root: Filesystem, gctx: &'gctx GlobalContext) -> CacheManager<'gctx> {
        CacheManager {
            cache_root,
            gctx,
            has_warned: Default::default(),
        }
    }

    /// Gets the cache associated with the key.
    pub fn get(&self, key: &str) -> Option<Vec<u8>> {
        let cache_path = &self.cache_path(key);
        match fs::read(cache_path) {
            Ok(contents) => Some(contents),
            Err(e) => {
                tracing::debug!(?cache_path, "cache missing: {e}");
                None
            }
        }
    }

    /// Associates the value with the key.
    pub fn put(&self, key: &str, value: &[u8]) {
        let cache_path = &self.cache_path(key);
        if let Err(e) = self.put_inner(cache_path, value) {
            tracing::info!(?cache_path, "failed to write cache: {e}");

            if !*self.has_warned.borrow() {
                let _ = self.gctx.shell().warn(format!(
                    "failed to write cache, path: {}, error: {e}",
                    cache_path.to_str().unwrap_or_default()
                ));
                *self.has_warned.borrow_mut() = true;
            }
        }
    }

    fn put_inner(&self, cache_path: &PathBuf, value: &[u8]) -> std::io::Result<()> {
        fs::create_dir_all(cache_path.parent().unwrap())?;
        let path = Filesystem::new(cache_path.clone());
        self.gctx
            .assert_package_cache_locked(CacheLockMode::DownloadExclusive, &path);
        fs::write(cache_path, value)?;
        Ok(())
    }

    /// Invalidates the cache associated with the key.
    pub fn invalidate(&self, key: &str) {
        let cache_path = &self.cache_path(key);
        if let Err(e) = fs::remove_file(cache_path) {
            if e.kind() != io::ErrorKind::NotFound {
                tracing::debug!(?cache_path, "failed to remove from cache: {e}");
            }
        }
    }

    fn cache_path(&self, key: &str) -> PathBuf {
        let relative = make_dep_path(key, false);
        // This is the file we're loading from cache or the index data.
        // See module comment in `registry/mod.rs` for why this is structured
        // the way it is.
        self.cache_root.join(relative).into_path_unlocked()
    }
}
