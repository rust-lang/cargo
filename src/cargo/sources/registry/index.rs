//! Management of the index of a registry source
//!
//! This module contains management of the index and various operations, such as
//! actually parsing the index, looking for crates, etc. This is intended to be
//! abstract over remote indices (downloaded via git) and local registry indices
//! (which are all just present on the filesystem).
//!
//! ## Index Performance
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
//! When we get queries from the resolver we're given a `Dependency`. This
//! dependency in turn has a version requirement, and with lock files that
//! already exist these version requirements are exact version requirements
//! `=a.b.c`. This means that we in theory only need to parse one line of JSON
//! per query in the registry, the one that matches version `a.b.c`.
//!
//! The crates.io index, however, is not amenable to this form of query. Instead
//! the crates.io index simply is a file where each line is a JSON blob. To
//! learn about the versions in each JSON blob we would need to parse the JSON,
//! defeating the purpose of trying to parse as little as possible.
//!
//! > Note that as a small aside even *loading* the JSON from the registry is
//! > actually pretty slow. For crates.io and remote registries we don't
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
//! emit a new file (a cache) which is amenable for speedily parsing in future
//! invocations.
//!
//! This cache file is currently organized by basically having the semver
//! version extracted from each JSON blob. That way Cargo can quickly and easily
//! parse all versions contained and which JSON blob they're associated with.
//! The JSON blob then doesn't actually need to get parsed unless the version is
//! parsed.
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

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::str;

use log::info;
use semver::{Version, VersionReq};

use crate::core::dependency::Dependency;
use crate::core::{InternedString, PackageId, SourceId, Summary};
use crate::sources::registry::{RegistryData, RegistryPackage};
use crate::util::{internal, CargoResult, Config, Filesystem, ToSemver};

/// Crates.io treats hyphen and underscores as interchangeable, but the index and old Cargo do not.
/// Therefore, the index must store uncanonicalized version of the name so old Cargo's can find it.
/// This loop tries all possible combinations of switching hyphen and underscores to find the
/// uncanonicalized one. As all stored inputs have the correct spelling, we start with the spelling
/// as-provided.
struct UncanonicalizedIter<'s> {
    input: &'s str,
    num_hyphen_underscore: u32,
    hyphen_combination_num: u16,
}

impl<'s> UncanonicalizedIter<'s> {
    fn new(input: &'s str) -> Self {
        let num_hyphen_underscore = input.chars().filter(|&c| c == '_' || c == '-').count() as u32;
        UncanonicalizedIter {
            input,
            num_hyphen_underscore,
            hyphen_combination_num: 0,
        }
    }
}

impl<'s> Iterator for UncanonicalizedIter<'s> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        if self.hyphen_combination_num > 0
            && self.hyphen_combination_num.trailing_zeros() >= self.num_hyphen_underscore
        {
            return None;
        }

        let ret = Some(
            self.input
                .chars()
                .scan(0u16, |s, c| {
                    // the check against 15 here's to prevent
                    // shift overflow on inputs with more then 15 hyphens
                    if (c == '_' || c == '-') && *s <= 15 {
                        let switch = (self.hyphen_combination_num & (1u16 << *s)) > 0;
                        let out = if (c == '_') ^ switch { '_' } else { '-' };
                        *s += 1;
                        Some(out)
                    } else {
                        Some(c)
                    }
                })
                .collect(),
        );
        self.hyphen_combination_num += 1;
        ret
    }
}

#[test]
fn no_hyphen() {
    assert_eq!(
        UncanonicalizedIter::new("test").collect::<Vec<_>>(),
        vec!["test".to_string()]
    )
}

#[test]
fn two_hyphen() {
    assert_eq!(
        UncanonicalizedIter::new("te-_st").collect::<Vec<_>>(),
        vec![
            "te-_st".to_string(),
            "te__st".to_string(),
            "te--st".to_string(),
            "te_-st".to_string()
        ]
    )
}

#[test]
fn overflow_hyphen() {
    assert_eq!(
        UncanonicalizedIter::new("te-_-_-_-_-_-_-_-_-st")
            .take(100)
            .count(),
        100
    )
}

pub struct RegistryIndex<'cfg> {
    source_id: SourceId,
    path: Filesystem,
    summaries_cache: HashMap<InternedString, Summaries>,
    config: &'cfg Config,
}

/// An internal cache of summaries for a particular package.
///
/// A list of summaries are loaded from disk via one of two methods:
///
/// 1. Primarily Cargo will parse the corresponding file for a crate in the
///    upstream crates.io registry. That's just a JSON blob per line which we
///    can parse, extract the version, and then store here.
///
/// 2. Alternatively, if Cargo has previously run, we'll have a cached index of
///    dependencies for the upstream index. This is a file that Cargo maintains
///    lazily on the local filesystem and is much faster to parse since it
///    doesn't involve parsing all of the JSON.
///
/// The outward-facing interface of this doesn't matter too much where it's
/// loaded from, but it's important when reading the implementation to note that
/// we try to parse as little as possible!
#[derive(Default)]
struct Summaries {
    /// A raw vector of uninterpreted bytes. This is what `Unparsed` start/end
    /// fields are indexes into. If a `Summaries` is loaded from the crates.io
    /// index then this field will be empty since nothing is `Unparsed`.
    raw_data: Vec<u8>,

    /// All known versions of a crate, keyed from their `Version` to the
    /// possibly parsed or unparsed version of the full summary.
    versions: HashMap<Version, MaybeIndexSummary>,
}

/// A lazily parsed `IndexSummary`.
enum MaybeIndexSummary {
    /// A summary which has not been parsed, The `start` and `end` are pointers
    /// into `Summaries::raw_data` which this is an entry of.
    Unparsed { start: usize, end: usize },

    /// An actually parsed summary.
    Parsed(IndexSummary),
}

/// A parsed representation of a summary from the index.
///
/// In addition to a full `Summary` we have a few auxiliary pieces of
/// information liked `yanked` and what the checksum hash is.
pub struct IndexSummary {
    pub summary: Summary,
    pub yanked: bool,
    pub hash: String,
}

/// A representation of the cache on disk that Cargo maintains of summaries.
/// Cargo will initially parse all summaries in the registry and will then
/// serialize that into this form and place it in a new location on disk,
/// ensuring that access in the future is much speedier.
#[derive(Default)]
struct SummariesCache<'a> {
    versions: Vec<(Version, &'a [u8])>,
}

impl<'cfg> RegistryIndex<'cfg> {
    pub fn new(
        source_id: SourceId,
        path: &Filesystem,
        config: &'cfg Config,
    ) -> RegistryIndex<'cfg> {
        RegistryIndex {
            source_id,
            path: path.clone(),
            summaries_cache: HashMap::new(),
            config,
        }
    }

    /// Returns the hash listed for a specified `PackageId`.
    pub fn hash(&mut self, pkg: PackageId, load: &mut dyn RegistryData) -> CargoResult<String> {
        let req = VersionReq::exact(pkg.version());
        let summary = self
            .summaries(pkg.name(), &req, load)?
            .next()
            .ok_or_else(|| internal(format!("no hash listed for {}", pkg)))?;
        Ok(summary.hash.clone())
    }

    /// Load a list of summaries for `name` package in this registry which
    /// match `req`
    ///
    /// This function will semantically parse the on-disk index, match all
    /// versions, and then return an iterator over all summaries which matched.
    /// Internally there's quite a few layer of caching to amortize this cost
    /// though since this method is called quite a lot on null builds in Cargo.
    pub fn summaries<'a, 'b>(
        &'a mut self,
        name: InternedString,
        req: &'b VersionReq,
        load: &mut dyn RegistryData,
    ) -> CargoResult<impl Iterator<Item = &'a IndexSummary> + 'b>
    where
        'a: 'b,
    {
        let source_id = self.source_id;

        // First up actually parse what summaries we have available. If Cargo
        // has run previously this will parse a Cargo-specific cache file rather
        // than the registry itself. In effect this is intended to be a quite
        // cheap operation.
        let summaries = self.load_summaries(name, load)?;

        // Iterate over our summaries, extract all relevant ones which match our
        // version requirement, and then parse all corresponding rows in the
        // registry. As a reminder this `summaries` method is called for each
        // entry in a lock file on every build, so we want to absolutely
        // minimize the amount of work being done here and parse as little as
        // necessary.
        let raw_data = &summaries.raw_data;
        Ok(summaries
            .versions
            .iter_mut()
            .filter_map(move |(k, v)| if req.matches(k) { Some(v) } else { None })
            .filter_map(move |maybe| match maybe.parse(raw_data, source_id) {
                Ok(summary) => Some(summary),
                Err(e) => {
                    info!("failed to parse `{}` registry package: {}", name, e);
                    None
                }
            }))
    }

    fn load_summaries(
        &mut self,
        name: InternedString,
        load: &mut dyn RegistryData,
    ) -> CargoResult<&mut Summaries> {
        // If we've previously loaded what versions are present for `name`, just
        // return that since our cache should still be valid.
        if self.summaries_cache.contains_key(&name) {
            return Ok(self.summaries_cache.get_mut(&name).unwrap());
        }

        // Prepare the `RegistryData` which will lazily initialize internal data
        // structures.
        load.prepare()?;

        // let root = self.config.assert_package_cache_locked(&self.path);
        let root = load.assert_index_locked(&self.path);
        let cache_root = root.join(".cache");
        let index_version = load.current_version();

        // See module comment in `registry/mod.rs` for why this is structured
        // the way it is.
        let fs_name = name
            .chars()
            .flat_map(|c| c.to_lowercase())
            .collect::<String>();
        let raw_path = match fs_name.len() {
            1 => format!("1/{}", fs_name),
            2 => format!("2/{}", fs_name),
            3 => format!("3/{}/{}", &fs_name[..1], fs_name),
            _ => format!("{}/{}/{}", &fs_name[0..2], &fs_name[2..4], fs_name),
        };

        // Attempt to handle misspellings by searching for a chain of related
        // names to the original `raw_path` name. Only return summaries
        // associated with the first hit, however. The resolver will later
        // reject any candidates that have the wrong name, and with this it'll
        // along the way produce helpful "did you mean?" suggestions.
        for path in UncanonicalizedIter::new(&raw_path).take(1024) {
            let summaries = Summaries::parse(
                index_version.as_ref().map(|s| &**s),
                root,
                &cache_root,
                path.as_ref(),
                self.source_id,
                load,
                self.config,
            )?;
            if let Some(summaries) = summaries {
                self.summaries_cache.insert(name, summaries);
                return Ok(self.summaries_cache.get_mut(&name).unwrap());
            }
        }

        // If nothing was found then this crate doesn't exists, so just use an
        // empty `Summaries` list.
        self.summaries_cache.insert(name, Summaries::default());
        Ok(self.summaries_cache.get_mut(&name).unwrap())
    }

    pub fn query_inner(
        &mut self,
        dep: &Dependency,
        load: &mut dyn RegistryData,
        yanked_whitelist: &HashSet<PackageId>,
        f: &mut dyn FnMut(Summary),
    ) -> CargoResult<()> {
        if self.config.offline()
            && self.query_inner_with_online(dep, load, yanked_whitelist, f, false)? != 0
        {
            return Ok(());
            // If offline, and there are no matches, try again with online.
            // This is necessary for dependencies that are not used (such as
            // target-cfg or optional), but are not downloaded. Normally the
            // build should succeed if they are not downloaded and not used,
            // but they still need to resolve. If they are actually needed
            // then cargo will fail to download and an error message
            // indicating that the required dependency is unavailable while
            // offline will be displayed.
        }
        self.query_inner_with_online(dep, load, yanked_whitelist, f, true)?;
        Ok(())
    }

    fn query_inner_with_online(
        &mut self,
        dep: &Dependency,
        load: &mut dyn RegistryData,
        yanked_whitelist: &HashSet<PackageId>,
        f: &mut dyn FnMut(Summary),
        online: bool,
    ) -> CargoResult<usize> {
        let source_id = self.source_id;
        let summaries = self
            .summaries(dep.package_name(), dep.version_req(), load)?
            // First filter summaries for `--offline`. If we're online then
            // everything is a candidate, otherwise if we're offline we're only
            // going to consider candidates which are actually present on disk.
            //
            // Note: This particular logic can cause problems with
            // optional dependencies when offline. If at least 1 version
            // of an optional dependency is downloaded, but that version
            // does not satisfy the requirements, then resolution will
            // fail. Unfortunately, whether or not something is optional
            // is not known here.
            .filter(|s| (online || load.is_crate_downloaded(s.summary.package_id())))
            // Next filter out all yanked packages. Some yanked packages may
            // leak throguh if they're in a whitelist (aka if they were
            // previously in `Cargo.lock`
            .filter(|s| !s.yanked || yanked_whitelist.contains(&s.summary.package_id()))
            .map(|s| s.summary.clone());

        // Handle `cargo update --precise` here. If specified, our own source
        // will have a precise version listed of the form
        // `<pkg>=<p_req>o-><f_req>` where `<pkg>` is the name of a crate on
        // this source, `<p_req>` is the version installed and `<f_req> is the
        // version requested (argument to `--precise`).
        let name = dep.package_name().as_str();
        let summaries = summaries.filter(|s| match source_id.precise() {
            Some(p) if p.starts_with(name) && p[name.len()..].starts_with('=') => {
                let mut vers = p[name.len() + 1..].splitn(2, "->");
                if dep
                    .version_req()
                    .matches(&vers.next().unwrap().to_semver().unwrap())
                {
                    vers.next().unwrap() == s.version().to_string()
                } else {
                    true
                }
            }
            _ => true,
        });

        let mut count = 0;
        for summary in summaries {
            f(summary);
            count += 1;
        }
        Ok(count)
    }

    pub fn is_yanked(&mut self, pkg: PackageId, load: &mut dyn RegistryData) -> CargoResult<bool> {
        let req = VersionReq::exact(pkg.version());
        let found = self
            .summaries(pkg.name(), &req, load)?
            .any(|summary| summary.yanked);
        Ok(found)
    }
}

impl Summaries {
    /// Parse out a `Summaries` instances from on-disk state.
    ///
    /// This will attempt to prefer parsing a previous cache file that already
    /// exists from a previous invocation of Cargo (aka you're typing `cargo
    /// build` again after typing it previously). If parsing fails or the cache
    /// isn't found, then we take a slower path which loads the full descriptor
    /// for `relative` from the underlying index (aka typically libgit2 with
    /// crates.io) and then parse everything in there.
    ///
    /// * `index_version` - a version string to describe the current state of
    ///   the index which for remote registries is the current git sha and
    ///   for local registries is not available.
    /// * `root` - this is the root argument passed to `load`
    /// * `cache_root` - this is the root on the filesystem itself of where to
    ///   store cache files.
    /// * `relative` - this is the file we're loading from cache or the index
    ///   data
    /// * `source_id` - the registry's SourceId used when parsing JSON blobs to
    ///   create summaries.
    /// * `load` - the actual index implementation which may be very slow to
    ///   call. We avoid this if we can.
    pub fn parse(
        index_version: Option<&str>,
        root: &Path,
        cache_root: &Path,
        relative: &Path,
        source_id: SourceId,
        load: &mut dyn RegistryData,
        config: &Config,
    ) -> CargoResult<Option<Summaries>> {
        // First up, attempt to load the cache. This could fail for all manner
        // of reasons, but consider all of them non-fatal and just log their
        // occurrence in case anyone is debugging anything.
        let cache_path = cache_root.join(relative);
        let mut cache_contents = None;
        if let Some(index_version) = index_version {
            match fs::read(&cache_path) {
                Ok(contents) => match Summaries::parse_cache(contents, index_version) {
                    Ok(s) => {
                        log::debug!("fast path for registry cache of {:?}", relative);
                        if cfg!(debug_assertions) {
                            cache_contents = Some(s.raw_data);
                        } else {
                            return Ok(Some(s));
                        }
                    }
                    Err(e) => {
                        log::debug!("failed to parse {:?} cache: {}", relative, e);
                    }
                },
                Err(e) => log::debug!("cache missing for {:?} error: {}", relative, e),
            }
        }

        // This is the fallback path where we actually talk to libgit2 to load
        // information. Here we parse every single line in the index (as we need
        // to find the versions)
        log::debug!("slow path for {:?}", relative);
        let mut ret = Summaries::default();
        let mut hit_closure = false;
        let mut cache_bytes = None;
        let err = load.load(root, relative, &mut |contents| {
            ret.raw_data = contents.to_vec();
            let mut cache = SummariesCache::default();
            hit_closure = true;
            for line in split(contents, b'\n') {
                // Attempt forwards-compatibility on the index by ignoring
                // everything that we ourselves don't understand, that should
                // allow future cargo implementations to break the
                // interpretation of each line here and older cargo will simply
                // ignore the new lines.
                let summary = match IndexSummary::parse(line, source_id) {
                    Ok(summary) => summary,
                    Err(e) => {
                        log::info!("failed to parse {:?} registry package: {}", relative, e);
                        continue;
                    }
                };
                let version = summary.summary.package_id().version().clone();
                cache.versions.push((version.clone(), line));
                ret.versions.insert(version, summary.into());
            }
            if let Some(index_version) = index_version {
                cache_bytes = Some(cache.serialize(index_version));
            }
            Ok(())
        });

        // We ignore lookup failures as those are just crates which don't exist
        // or we haven't updated the registry yet. If we actually ran the
        // closure though then we care about those errors.
        if !hit_closure {
            debug_assert!(cache_contents.is_none());
            return Ok(None);
        }
        err?;

        // If we've got debug assertions enabled and the cache was previously
        // present and considered fresh this is where the debug assertions
        // actually happens to verify that our cache is indeed fresh and
        // computes exactly the same value as before.
        if cfg!(debug_assertions) && cache_contents.is_some() {
            assert_eq!(cache_bytes, cache_contents);
        }

        // Once we have our `cache_bytes` which represents the `Summaries` we're
        // about to return, write that back out to disk so future Cargo
        // invocations can use it.
        //
        // This is opportunistic so we ignore failure here but are sure to log
        // something in case of error.
        if let Some(cache_bytes) = cache_bytes {
            if fs::create_dir_all(cache_path.parent().unwrap()).is_ok() {
                let path = Filesystem::new(cache_path.clone());
                config.assert_package_cache_locked(&path);
                if let Err(e) = fs::write(cache_path, cache_bytes) {
                    log::info!("failed to write cache: {}", e);
                }
            }
        }

        Ok(Some(ret))
    }

    /// Parses an open `File` which represents information previously cached by
    /// Cargo.
    pub fn parse_cache(contents: Vec<u8>, last_index_update: &str) -> CargoResult<Summaries> {
        let cache = SummariesCache::parse(&contents, last_index_update)?;
        let mut ret = Summaries::default();
        for (version, summary) in cache.versions {
            let (start, end) = subslice_bounds(&contents, summary);
            ret.versions
                .insert(version, MaybeIndexSummary::Unparsed { start, end });
        }
        ret.raw_data = contents;
        return Ok(ret);

        // Returns the start/end offsets of `inner` with `outer`. Asserts that
        // `inner` is a subslice of `outer`.
        fn subslice_bounds(outer: &[u8], inner: &[u8]) -> (usize, usize) {
            let outer_start = outer.as_ptr() as usize;
            let outer_end = outer_start + outer.len();
            let inner_start = inner.as_ptr() as usize;
            let inner_end = inner_start + inner.len();
            assert!(inner_start >= outer_start);
            assert!(inner_end <= outer_end);
            (inner_start - outer_start, inner_end - outer_start)
        }
    }
}

// Implementation of serializing/deserializing the cache of summaries on disk.
// Currently the format looks like:
//
// +--------------+-------------+---+
// | version byte | git sha rev | 0 |
// +--------------+-------------+---+
//
// followed by...
//
// +----------------+---+------------+---+
// | semver version | 0 |  JSON blob | 0 | ...
// +----------------+---+------------+---+
//
// The idea is that this is a very easy file for Cargo to parse in future
// invocations. The read from disk should be quite fast and then afterwards all
// we need to know is what versions correspond to which JSON blob.
//
// The leading version byte is intended to ensure that there's some level of
// future compatibility against changes to this cache format so if different
// versions of Cargo share the same cache they don't get too confused. The git
// sha lets us know when the file needs to be regenerated (it needs regeneration
// whenever the index itself updates).

const CURRENT_CACHE_VERSION: u8 = 1;

impl<'a> SummariesCache<'a> {
    fn parse(data: &'a [u8], last_index_update: &str) -> CargoResult<SummariesCache<'a>> {
        // NB: keep this method in sync with `serialize` below
        let (first_byte, rest) = data
            .split_first()
            .ok_or_else(|| failure::format_err!("malformed cache"))?;
        if *first_byte != CURRENT_CACHE_VERSION {
            failure::bail!("looks like a different Cargo's cache, bailing out");
        }
        let mut iter = split(rest, 0);
        if let Some(update) = iter.next() {
            if update != last_index_update.as_bytes() {
                failure::bail!(
                    "cache out of date: current index ({}) != cache ({})",
                    last_index_update,
                    str::from_utf8(update)?,
                )
            }
        } else {
            failure::bail!("malformed file");
        }
        let mut ret = SummariesCache::default();
        while let Some(version) = iter.next() {
            let version = str::from_utf8(version)?;
            let version = Version::parse(version)?;
            let summary = iter.next().unwrap();
            ret.versions.push((version, summary));
        }
        Ok(ret)
    }

    fn serialize(&self, index_version: &str) -> Vec<u8> {
        // NB: keep this method in sync with `parse` above
        let size = self
            .versions
            .iter()
            .map(|(_version, data)| (10 + data.len()))
            .sum();
        let mut contents = Vec::with_capacity(size);
        contents.push(CURRENT_CACHE_VERSION);
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

impl MaybeIndexSummary {
    /// Parses this "maybe a summary" into a `Parsed` for sure variant.
    ///
    /// Does nothing if this is already `Parsed`, and otherwise the `raw_data`
    /// passed in is sliced with the bounds in `Unparsed` and then actually
    /// parsed.
    fn parse(&mut self, raw_data: &[u8], source_id: SourceId) -> CargoResult<&IndexSummary> {
        let (start, end) = match self {
            MaybeIndexSummary::Unparsed { start, end } => (*start, *end),
            MaybeIndexSummary::Parsed(summary) => return Ok(summary),
        };
        let summary = IndexSummary::parse(&raw_data[start..end], source_id)?;
        *self = MaybeIndexSummary::Parsed(summary);
        match self {
            MaybeIndexSummary::Unparsed { .. } => unreachable!(),
            MaybeIndexSummary::Parsed(summary) => Ok(summary),
        }
    }
}

impl From<IndexSummary> for MaybeIndexSummary {
    fn from(summary: IndexSummary) -> MaybeIndexSummary {
        MaybeIndexSummary::Parsed(summary)
    }
}

impl IndexSummary {
    /// Parses a line from the registry's index file into an `IndexSummary` for
    /// a package.
    ///
    /// The `line` provided is expected to be valid JSON.
    fn parse(line: &[u8], source_id: SourceId) -> CargoResult<IndexSummary> {
        let RegistryPackage {
            name,
            vers,
            cksum,
            deps,
            features,
            yanked,
            links,
        } = serde_json::from_slice(line)?;
        log::trace!("json parsed registry {}/{}", name, vers);
        let pkgid = PackageId::new(&name, &vers, source_id)?;
        let deps = deps
            .into_iter()
            .map(|dep| dep.into_dep(source_id))
            .collect::<CargoResult<Vec<_>>>()?;
        let mut summary = Summary::new(pkgid, deps, &features, links, false)?;
        summary.set_checksum(cksum.clone());
        Ok(IndexSummary {
            summary,
            yanked: yanked.unwrap_or(false),
            hash: cksum,
        })
    }
}

fn split<'a>(haystack: &'a [u8], needle: u8) -> impl Iterator<Item = &'a [u8]> + 'a {
    struct Split<'a> {
        haystack: &'a [u8],
        needle: u8,
    }

    impl<'a> Iterator for Split<'a> {
        type Item = &'a [u8];

        fn next(&mut self) -> Option<&'a [u8]> {
            if self.haystack.is_empty() {
                return None;
            }
            let (ret, remaining) = match memchr::memchr(self.needle, self.haystack) {
                Some(pos) => (&self.haystack[..pos], &self.haystack[pos + 1..]),
                None => (self.haystack, &[][..]),
            };
            self.haystack = remaining;
            Some(ret)
        }
    }

    Split { haystack, needle }
}
