//! Management of the index of a registry source.
//!
//! This module contains management of the index and various operations, such as
//! actually parsing the index, looking for crates, etc. This is intended to be
//! abstract over remote indices (downloaded via Git or HTTP) and local registry
//! indices (which are all just present on the filesystem).
//!
//! ## How the index works
//!
//! Here is a simple flow when loading a [`Summary`] (metadata) from the index:
//!
//! 1. A query is fired via [`RegistryIndex::query_inner`].
//! 2. Tries loading all summaries via [`RegistryIndex::load_summaries`], and
//!    under the hood calling [`Summaries::parse`] to parse an index file.
//!     1. If an on-disk index cache is present, loads it via
//!        [`Summaries::parse_cache`].
//!     2. Otherwise goes to the slower path [`RegistryData::load`] to get the
//!        specific index file.
//! 3. A [`Summary`] is now ready in callback `f` in [`RegistryIndex::query_inner`].
//!
//! To learn the rationale behind this multi-layer index metadata loading,
//! see [the documentation of the on-disk index cache](cache).
use crate::core::dependency::{Artifact, DepKind};
use crate::core::Dependency;
use crate::core::{PackageId, SourceId, Summary};
use crate::sources::registry::{LoadResponse, RegistryData};
use crate::util::interning::InternedString;
use crate::util::IntoUrl;
use crate::util::{internal, CargoResult, Filesystem, GlobalContext, OptVersionReq};
use cargo_util::registry::make_dep_path;
use cargo_util_schemas::manifest::RustVersion;
use semver::Version;
use serde::Deserialize;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::Path;
use std::str;
use std::task::{ready, Poll};
use tracing::{debug, info};

mod cache;
use self::cache::{CacheManager, MaybeSummaries};
use self::cache::SummariesCache;

/// The maximum schema version of the `v` field in the index this version of
/// cargo understands. See [`IndexPackage::v`] for the detail.
const INDEX_V_MAX: u32 = 2;

/// Manager for handling the on-disk index.
///
/// Different kinds of registries store the index differently:
///
/// * [`LocalRegistry`]` is a simple on-disk tree of files of the raw index.
/// * [`RemoteRegistry`] is stored as a raw git repository.
/// * [`HttpRegistry`] fills the on-disk index cache directly without keeping
///   any raw index.
///
/// These means of access are handled via the [`RegistryData`] trait abstraction.
/// This transparently handles caching of the index in a more efficient format.
///
/// [`LocalRegistry`]: super::local::LocalRegistry
/// [`RemoteRegistry`]: super::remote::RemoteRegistry
/// [`HttpRegistry`]: super::http_remote::HttpRegistry
pub struct RegistryIndex<'gctx> {
    source_id: SourceId,
    /// Root directory of the index for the registry.
    path: Filesystem,
    /// In-memory cache of summary data.
    ///
    /// This is keyed off the package name. The [`Summaries`] value handles
    /// loading the summary data. It keeps an optimized on-disk representation
    /// of the JSON files, which is created in an as-needed fashion. If it
    /// hasn't been cached already, it uses [`RegistryData::load`] to access
    /// to JSON files from the index, and the creates the optimized on-disk
    /// summary cache.
    summaries_cache: HashMap<InternedString, Summaries>,
    /// [`GlobalContext`] reference for convenience.
    gctx: &'gctx GlobalContext,
    /// Manager of on-disk caches.
    cache_manager: CacheManager<'gctx>,
}

/// An internal cache of summaries for a particular package.
///
/// A list of summaries are loaded from disk via one of two methods:
///
/// 1. From raw registry index --- Primarily Cargo will parse the corresponding
///    file for a crate in the upstream crates.io registry. That's just a JSON
///    blob per line which we can parse, extract the version, and then store here.
///    See [`IndexPackage`] and [`IndexSummary::parse`].
///
/// 2. From on-disk index cache --- If Cargo has previously run, we'll have a
///    cached index of dependencies for the upstream index. This is a file that
///    Cargo maintains lazily on the local filesystem and is much faster to
///    parse since it doesn't involve parsing all of the JSON.
///    See [`SummariesCache`].
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

/// A lazily parsed [`IndexSummary`].
enum MaybeIndexSummary {
    /// A summary which has not been parsed, The `start` and `end` are pointers
    /// into [`Summaries::raw_data`] which this is an entry of.
    Unparsed(std::ops::Range<usize>),
    UnparsedData(Vec<u8>),

    /// An actually parsed summary.
    Parsed(IndexSummary),
}

/// A parsed representation of a summary from the index. This is usually parsed
/// from a line from a raw index file, or a JSON blob from on-disk index cache.
///
/// In addition to a full [`Summary`], we have information on whether it is `yanked`.
#[derive(Clone, Debug)]
pub enum IndexSummary {
    /// Available for consideration
    Candidate(Summary),
    /// Yanked within its registry
    Yanked(Summary),
    /// Not available as we are offline and create is not downloaded yet
    Offline(Summary),
    /// From a newer schema version and is likely incomplete or inaccurate
    Unsupported(Summary, u32),
}

impl IndexSummary {
    /// Extract the summary from any variant
    pub fn as_summary(&self) -> &Summary {
        match self {
            IndexSummary::Candidate(sum)
            | IndexSummary::Yanked(sum)
            | IndexSummary::Offline(sum)
            | IndexSummary::Unsupported(sum, _) => sum,
        }
    }

    /// Extract the summary from any variant
    pub fn into_summary(self) -> Summary {
        match self {
            IndexSummary::Candidate(sum)
            | IndexSummary::Yanked(sum)
            | IndexSummary::Offline(sum)
            | IndexSummary::Unsupported(sum, _) => sum,
        }
    }

    pub fn map_summary(self, f: impl Fn(Summary) -> Summary) -> Self {
        match self {
            IndexSummary::Candidate(s) => IndexSummary::Candidate(f(s)),
            IndexSummary::Yanked(s) => IndexSummary::Yanked(f(s)),
            IndexSummary::Offline(s) => IndexSummary::Offline(f(s)),
            IndexSummary::Unsupported(s, v) => IndexSummary::Unsupported(f(s), v.clone()),
        }
    }

    /// Extract the package id from any variant
    pub fn package_id(&self) -> PackageId {
        match self {
            IndexSummary::Candidate(sum)
            | IndexSummary::Yanked(sum)
            | IndexSummary::Offline(sum)
            | IndexSummary::Unsupported(sum, _) => sum.package_id(),
        }
    }

    /// Returns `true` if the index summary is [`Yanked`].
    ///
    /// [`Yanked`]: IndexSummary::Yanked
    #[must_use]
    pub fn is_yanked(&self) -> bool {
        matches!(self, Self::Yanked(..))
    }

    /// Returns `true` if the index summary is [`Offline`].
    ///
    /// [`Offline`]: IndexSummary::Offline
    #[must_use]
    pub fn is_offline(&self) -> bool {
        matches!(self, Self::Offline(..))
    }
}

/// A single line in the index representing a single version of a package.
#[derive(Deserialize)]
pub struct IndexPackage<'a> {
    /// Name of the package.
    name: InternedString,
    /// The version of this dependency.
    vers: Version,
    /// All kinds of direct dependencies of the package, including dev and
    /// build dependencies.
    #[serde(borrow)]
    deps: Vec<RegistryDependency<'a>>,
    /// Set of features defined for the package, i.e., `[features]` table.
    features: BTreeMap<InternedString, Vec<InternedString>>,
    /// This field contains features with new, extended syntax. Specifically,
    /// namespaced features (`dep:`) and weak dependencies (`pkg?/feat`).
    ///
    /// This is separated from `features` because versions older than 1.19
    /// will fail to load due to not being able to parse the new syntax, even
    /// with a `Cargo.lock` file.
    features2: Option<BTreeMap<InternedString, Vec<InternedString>>>,
    /// Checksum for verifying the integrity of the corresponding downloaded package.
    cksum: String,
    /// If `true`, Cargo will skip this version when resolving.
    ///
    /// This was added in 2014. Everything in the crates.io index has this set
    /// now, so this probably doesn't need to be an option anymore.
    yanked: Option<bool>,
    /// Native library name this package links to.
    ///
    /// Added early 2018 (see <https://github.com/rust-lang/cargo/pull/4978>),
    /// can be `None` if published before then.
    links: Option<InternedString>,
    /// Required version of rust
    ///
    /// Corresponds to `package.rust-version`.
    ///
    /// Added in 2023 (see <https://github.com/rust-lang/crates.io/pull/6267>),
    /// can be `None` if published before then or if not set in the manifest.
    rust_version: Option<RustVersion>,
    /// The schema version for this entry.
    ///
    /// If this is None, it defaults to version `1`. Entries with unknown
    /// versions are ignored.
    ///
    /// Version `2` schema adds the `features2` field.
    ///
    /// Version `3` schema adds `artifact`, `bindep_targes`, and `lib` for
    /// artifact dependencies support.
    ///
    /// This provides a method to safely introduce changes to index entries
    /// and allow older versions of cargo to ignore newer entries it doesn't
    /// understand. This is honored as of 1.51, so unfortunately older
    /// versions will ignore it, and potentially misinterpret version 2 and
    /// newer entries.
    ///
    /// The intent is that versions older than 1.51 will work with a
    /// pre-existing `Cargo.lock`, but they may not correctly process `cargo
    /// update` or build a lock from scratch. In that case, cargo may
    /// incorrectly select a new package that uses a new index schema. A
    /// workaround is to downgrade any packages that are incompatible with the
    /// `--precise` flag of `cargo update`.
    v: Option<u32>,
}

/// A dependency as encoded in the [`IndexPackage`] index JSON.
#[derive(Deserialize)]
struct RegistryDependency<'a> {
    /// Name of the dependency. If the dependency is renamed, the original
    /// would be stored in [`RegistryDependency::package`].
    name: InternedString,
    /// The SemVer requirement for this dependency.
    #[serde(borrow)]
    req: Cow<'a, str>,
    /// Set of features enabled for this dependency.
    features: Vec<InternedString>,
    /// Whether or not this is an optional dependency.
    optional: bool,
    /// Whether or not default features are enabled.
    default_features: bool,
    /// The target platform for this dependency.
    target: Option<Cow<'a, str>>,
    /// The dependency kind. "dev", "build", and "normal".
    kind: Option<Cow<'a, str>>,
    // The URL of the index of the registry where this dependency is from.
    // `None` if it is from the same index.
    registry: Option<Cow<'a, str>>,
    /// The original name if the dependency is renamed.
    package: Option<InternedString>,
    /// Whether or not this is a public dependency. Unstable. See [RFC 1977].
    ///
    /// [RFC 1977]: https://rust-lang.github.io/rfcs/1977-public-private-dependencies.html
    public: Option<bool>,
    artifact: Option<Vec<Cow<'a, str>>>,
    bindep_target: Option<Cow<'a, str>>,
    #[serde(default)]
    lib: bool,
}

impl<'gctx> RegistryIndex<'gctx> {
    /// Creates an empty registry index at `path`.
    pub fn new(
        source_id: SourceId,
        path: &Filesystem,
        gctx: &'gctx GlobalContext,
    ) -> RegistryIndex<'gctx> {
        RegistryIndex {
            source_id,
            path: path.clone(),
            summaries_cache: HashMap::new(),
            gctx,
            cache_manager: CacheManager::new(path.join(".cache"), gctx),
        }
    }

    /// Returns the hash listed for a specified `PackageId`. Primarily for
    /// checking the integrity of a downloaded package matching the checksum in
    /// the index file, aka [`IndexSummary`].
    pub fn hash(&mut self, pkg: PackageId, load: &mut dyn RegistryData) -> Poll<CargoResult<&str>> {
        let req = OptVersionReq::lock_to_exact(pkg.version());
        let summary = self.summaries(pkg.name(), &req, load)?;
        let summary = ready!(summary).next();
        Poll::Ready(Ok(summary
            .ok_or_else(|| internal(format!("no hash listed for {}", pkg)))?
            .as_summary()
            .checksum()
            .ok_or_else(|| internal(format!("no hash listed for {}", pkg)))?))
    }

    /// Load a list of summaries for `name` package in this registry which
    /// match `req`.
    ///
    /// This function will semantically
    ///
    /// 1. parse the index file (either raw or cache),
    /// 2. match all versions,
    /// 3. and then return an iterator over all summaries which matched.
    ///
    /// Internally there's quite a few layer of caching to amortize this cost
    /// though since this method is called quite a lot on null builds in Cargo.
    fn summaries<'a, 'b>(
        &'a mut self,
        name: InternedString,
        req: &'b OptVersionReq,
        load: &mut dyn RegistryData,
    ) -> Poll<CargoResult<impl Iterator<Item = &'a IndexSummary> + 'b>>
    where
        'a: 'b,
    {
        let bindeps = self.gctx.cli_unstable().bindeps;

        let source_id = self.source_id;

        // First up parse what summaries we have available.
        let summaries = ready!(self.load_summaries(name, load)?);

        // Iterate over our summaries, extract all relevant ones which match our
        // version requirement, and then parse all corresponding rows in the
        // registry. As a reminder this `summaries` method is called for each
        // entry in a lock file on every build, so we want to absolutely
        // minimize the amount of work being done here and parse as little as
        // necessary.
        let raw_data = &summaries.raw_data;
        Poll::Ready(Ok(summaries
            .versions
            .iter_mut()
            .filter_map(move |(k, v)| if req.matches(k) { Some(v) } else { None })
            .filter_map(move |maybe| {
                match maybe.parse(raw_data, source_id, bindeps) {
                    Ok(sum @ IndexSummary::Candidate(_) | sum @ IndexSummary::Yanked(_)) => {
                        Some(sum)
                    }
                    Ok(IndexSummary::Unsupported(summary, v)) => {
                        debug!(
                            "unsupported schema version {} ({} {})",
                            v,
                            summary.name(),
                            summary.version()
                        );
                        None
                    }
                    Ok(IndexSummary::Offline(_)) => {
                        unreachable!("We do not check for off-line until later")
                    }
                    Err(e) => {
                        info!("failed to parse `{}` registry package: {}", name, e);
                        None
                    }
                }
            })))
    }

    /// Actually parses what summaries we have available.
    ///
    /// If Cargo has run previously, this tries in this order:
    ///
    /// 1. Returns from in-memory cache, aka [`RegistryIndex::summaries_cache`].
    /// 2. If missing, hands over to [`Summaries::parse`] to parse an index file.
    ///
    ///    The actual kind index file being parsed depends on which kind of
    ///    [`RegistryData`] the `load` argument is given. For example, a
    ///    Git-based [`RemoteRegistry`] will first try a on-disk index cache
    ///    file, and then try parsing registry raw index from Git repository.
    ///
    /// In effect, this is intended to be a quite cheap operation.
    ///
    /// [`RemoteRegistry`]: super::remote::RemoteRegistry
    fn load_summaries(
        &mut self,
        name: InternedString,
        load: &mut dyn RegistryData,
    ) -> Poll<CargoResult<&mut Summaries>> {
        // If we've previously loaded what versions are present for `name`, just
        // return that since our in-memory cache should still be valid.
        if self.summaries_cache.contains_key(&name) {
            return Poll::Ready(Ok(self.summaries_cache.get_mut(&name).unwrap()));
        }

        // Prepare the `RegistryData` which will lazily initialize internal data
        // structures.
        load.prepare()?;

        let root = load.assert_index_locked(&self.path);
        let summaries = ready!(Summaries::parse(
            root,
            &name,
            self.source_id,
            load,
            self.gctx.cli_unstable().bindeps,
            &self.cache_manager,
        ))?
        .unwrap_or_default();
        self.summaries_cache.insert(name, summaries);
        Poll::Ready(Ok(self.summaries_cache.get_mut(&name).unwrap()))
    }

    /// Clears the in-memory summaries cache.
    pub fn clear_summaries_cache(&mut self) {
        self.summaries_cache.clear();
    }

    /// Attempts to find the packages that match a `name` and a version `req`.
    ///
    /// This is primarily used by [`Source::query`](super::Source).
    pub fn query_inner(
        &mut self,
        name: InternedString,
        req: &OptVersionReq,
        load: &mut dyn RegistryData,
        f: &mut dyn FnMut(IndexSummary),
    ) -> Poll<CargoResult<()>> {
        if self.gctx.offline() {
            // This should only return `Poll::Ready(Ok(()))` if there is at least 1 match.
            //
            // If there are 0 matches it should fall through and try again with online.
            // This is necessary for dependencies that are not used (such as
            // target-cfg or optional), but are not downloaded. Normally the
            // build should succeed if they are not downloaded and not used,
            // but they still need to resolve. If they are actually needed
            // then cargo will fail to download and an error message
            // indicating that the required dependency is unavailable while
            // offline will be displayed.
            let mut called = false;
            let callback = &mut |s: IndexSummary| {
                if !s.is_offline() {
                    called = true;
                    f(s);
                }
            };
            ready!(self.query_inner_with_online(name, req, load, callback, false)?);
            if called {
                return Poll::Ready(Ok(()));
            }
        }
        self.query_inner_with_online(name, req, load, f, true)
    }

    /// Inner implementation of [`Self::query_inner`]. Returns the number of
    /// summaries we've got.
    ///
    /// The `online` controls whether Cargo can access the network when needed.
    fn query_inner_with_online(
        &mut self,
        name: InternedString,
        req: &OptVersionReq,
        load: &mut dyn RegistryData,
        f: &mut dyn FnMut(IndexSummary),
        online: bool,
    ) -> Poll<CargoResult<()>> {
        ready!(self.summaries(name, &req, load))?
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
            .map(|s| {
                if online || load.is_crate_downloaded(s.package_id()) {
                    s.clone()
                } else {
                    IndexSummary::Offline(s.as_summary().clone())
                }
            })
            .for_each(f);
        Poll::Ready(Ok(()))
    }

    /// Looks into the summaries to check if a package has been yanked.
    pub fn is_yanked(
        &mut self,
        pkg: PackageId,
        load: &mut dyn RegistryData,
    ) -> Poll<CargoResult<bool>> {
        let req = OptVersionReq::lock_to_exact(pkg.version());
        let found = ready!(self.summaries(pkg.name(), &req, load))?.any(|s| s.is_yanked());
        Poll::Ready(Ok(found))
    }
}

impl Summaries {
    /// Parse out a [`Summaries`] instances from on-disk state.
    ///
    /// This will do the followings in order:
    ///
    /// 1. Attempt to prefer parsing a previous index cache file that already
    ///    exists from a previous invocation of Cargo (aka you're typing `cargo
    ///    build` again after typing it previously).
    /// 2. If parsing fails, or the cache isn't found or is invalid, we then
    ///    take a slower path which loads the full descriptor for `relative`
    ///    from the underlying index (aka libgit2 with crates.io, or from a
    ///    remote HTTP index) and then parse everything in there.
    ///
    /// * `root` --- this is the root argument passed to `load`
    /// * `name` --- the name of the package.
    /// * `source_id` --- the registry's SourceId used when parsing JSON blobs
    ///   to create summaries.
    /// * `load` --- the actual index implementation which may be very slow to
    ///   call. We avoid this if we can.
    /// * `bindeps` --- whether the `-Zbindeps` unstable flag is enabled
    pub fn parse(
        root: &Path,
        name: &str,
        source_id: SourceId,
        load: &mut dyn RegistryData,
        bindeps: bool,
        cache_manager: &CacheManager<'_>,
    ) -> Poll<CargoResult<Option<Summaries>>> {
        // This is the file we're loading from cache or the index data.
        // See module comment in `registry/mod.rs` for why this is structured the way it is.
        let name = &name.to_lowercase();
        let relative = make_dep_path(&name, false);

        let mut cached_summaries = None;
        let mut index_version = None;
        if let Some(maybe_summaries) = cache_manager.get(name) {
            match maybe_summaries {
                MaybeSummaries::Unparsed(contents) => match Summaries::parse_cache(contents) {
                    Ok((s, v)) => {
                        cached_summaries = Some(s);
                        index_version = Some(v);
                    }
                    Err(e) => {
                        tracing::debug!("failed to parse {name:?} cache: {e}");
                    }
                }
                MaybeSummaries::Parsed(summaries) => {
                    cached_summaries = Some(summaries);
                    index_version = Some("2".into());
                }
            }
        }

        let response = ready!(load.load(root, relative.as_ref(), index_version.as_deref())?);

        match response {
            LoadResponse::CacheValid => {
                tracing::debug!("fast path for registry cache of {:?}", relative);
                return Poll::Ready(Ok(cached_summaries));
            }
            LoadResponse::NotFound => {
                cache_manager.invalidate(name);
                return Poll::Ready(Ok(None));
            }
            LoadResponse::Data {
                raw_data,
                index_version,
            } => {
                // This is the fallback path where we actually talk to the registry backend to load
                // information. Here we parse every single line in the index (as we need
                // to find the versions)
                tracing::debug!("slow path for {:?}", relative);
                let mut cache = SummariesCache::default();
                let mut ret = Summaries::default();
                ret.raw_data = raw_data;
                for line in split(&ret.raw_data, b'\n') {
                    // Attempt forwards-compatibility on the index by ignoring
                    // everything that we ourselves don't understand, that should
                    // allow future cargo implementations to break the
                    // interpretation of each line here and older cargo will simply
                    // ignore the new lines.
                    let summary = match IndexSummary::parse(line, source_id, bindeps) {
                        Ok(summary) => summary,
                        Err(e) => {
                            // This should only happen when there is an index
                            // entry from a future version of cargo that this
                            // version doesn't understand. Hopefully, those future
                            // versions of cargo correctly set INDEX_V_MAX and
                            // CURRENT_CACHE_VERSION, otherwise this will skip
                            // entries in the cache preventing those newer
                            // versions from reading them (that is, until the
                            // cache is rebuilt).
                            tracing::info!(
                                "failed to parse {:?} registry package: {}",
                                relative,
                                e
                            );
                            continue;
                        }
                    };
                    let version = summary.package_id().version().clone();
                    if cache_manager.is_sqlite() {
                        cache_manager.put_summary((&name, &version), line);
                    } else {
                        cache.versions.push((version.clone(), line));
                    }
                    ret.versions.insert(version, summary.into());
                }

                if cache_manager.is_sqlite() {
                    return Poll::Ready(Ok(Some(ret)));
                }

                if let Some(index_version) = index_version {
                    tracing::trace!("caching index_version {}", index_version);
                    let cache_bytes = cache.serialize(index_version.as_str());
                    // Once we have our `cache_bytes` which represents the `Summaries` we're
                    // about to return, write that back out to disk so future Cargo
                    // invocations can use it.
                    cache_manager.put(name, &cache_bytes);

                    // If we've got debug assertions enabled read back in the cached values
                    // and assert they match the expected result.
                    #[cfg(debug_assertions)]
                    {
                        let readback = SummariesCache::parse(&cache_bytes)
                            .expect("failed to parse cache we just wrote");
                        assert_eq!(
                            readback.index_version, index_version,
                            "index_version mismatch"
                        );
                        assert_eq!(readback.versions, cache.versions, "versions mismatch");
                    }
                }
                Poll::Ready(Ok(Some(ret)))
            }
        }
    }

    /// Parses the contents of an on-disk cache, aka [`SummariesCache`], which
    /// represents information previously cached by Cargo.
    pub fn parse_cache(contents: Vec<u8>) -> CargoResult<(Summaries, InternedString)> {
        let cache = SummariesCache::parse(&contents)?;
        let index_version = InternedString::new(cache.index_version);
        let mut ret = Summaries::default();
        for (version, summary) in cache.versions {
            let (start, end) = subslice_bounds(&contents, summary);
            ret.versions
                .insert(version, MaybeIndexSummary::Unparsed(start..end));
        }
        ret.raw_data = contents;
        return Ok((ret, index_version));

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

impl MaybeIndexSummary {
    /// Parses this "maybe a summary" into a `Parsed` for sure variant.
    ///
    /// Does nothing if this is already `Parsed`, and otherwise the `raw_data`
    /// passed in is sliced with the bounds in `Unparsed` and then actually
    /// parsed.
    fn parse(
        &mut self,
        raw_data: &[u8],
        source_id: SourceId,
        bindeps: bool,
    ) -> CargoResult<&IndexSummary> {
        let data = match self {
            MaybeIndexSummary::Unparsed(range) => &raw_data[range.clone()],
            MaybeIndexSummary::UnparsedData(data) => data,
            MaybeIndexSummary::Parsed(summary) => return Ok(summary),
        };
        let summary = IndexSummary::parse(data, source_id, bindeps)?;
        *self = MaybeIndexSummary::Parsed(summary);
        match self {
            MaybeIndexSummary::Unparsed { .. } => unreachable!(),
            MaybeIndexSummary::UnparsedData { .. } => unreachable!(),
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
    /// Parses a line from the registry's index file into an [`IndexSummary`]
    /// for a package.
    ///
    /// The `line` provided is expected to be valid JSON. It is supposed to be
    /// a [`IndexPackage`].
    fn parse(line: &[u8], source_id: SourceId, bindeps: bool) -> CargoResult<IndexSummary> {
        // ****CAUTION**** Please be extremely careful with returning errors
        // from this function. Entries that error are not included in the
        // index cache, and can cause cargo to get confused when switching
        // between different versions that understand the index differently.
        // Make sure to consider the INDEX_V_MAX and CURRENT_CACHE_VERSION
        // values carefully when making changes here.
        let IndexPackage {
            name,
            vers,
            cksum,
            deps,
            mut features,
            features2,
            yanked,
            links,
            rust_version,
            v,
        } = serde_json::from_slice(line)?;
        let v = v.unwrap_or(1);
        tracing::trace!("json parsed registry {}/{}", name, vers);
        let pkgid = PackageId::new(name.into(), vers.clone(), source_id);
        let deps = deps
            .into_iter()
            .map(|dep| dep.into_dep(source_id))
            .collect::<CargoResult<Vec<_>>>()?;
        if let Some(features2) = features2 {
            for (name, values) in features2 {
                features.entry(name).or_default().extend(values);
            }
        }
        let mut summary = Summary::new(pkgid, deps, &features, links, rust_version)?;
        summary.set_checksum(cksum);

        let v_max = if bindeps {
            INDEX_V_MAX + 1
        } else {
            INDEX_V_MAX
        };

        if v_max < v {
            Ok(IndexSummary::Unsupported(summary, v))
        } else if yanked.unwrap_or(false) {
            Ok(IndexSummary::Yanked(summary))
        } else {
            Ok(IndexSummary::Candidate(summary))
        }
    }
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
            artifact,
            bindep_target,
            lib,
        } = self;

        let id = if let Some(registry) = &registry {
            SourceId::for_registry(&registry.into_url()?)?
        } else {
            default
        };

        let mut dep = Dependency::parse(package.unwrap_or(name), Some(&req), id)?;
        if package.is_some() {
            dep.set_explicit_name_in_toml(name);
        }
        let kind = match kind.as_deref().unwrap_or("") {
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
        if !id.is_crates_io() {
            dep.set_registry_id(id);
        }

        if let Some(artifacts) = artifact {
            let artifact = Artifact::parse(&artifacts, lib, bindep_target.as_deref())?;
            dep.set_artifact(artifact);
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

/// Like [`slice::split`] but is optimized by [`memchr`].
fn split(haystack: &[u8], needle: u8) -> impl Iterator<Item = &[u8]> {
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

#[test]
fn escaped_char_in_index_json_blob() {
    let _: IndexPackage<'_> = serde_json::from_str(
        r#"{"name":"a","vers":"0.0.1","deps":[],"cksum":"bae3","features":{}}"#,
    )
    .unwrap();
    let _: IndexPackage<'_> = serde_json::from_str(
        r#"{"name":"a","vers":"0.0.1","deps":[],"cksum":"bae3","features":{"test":["k","q"]},"links":"a-sys"}"#
    ).unwrap();

    // Now we add escaped cher all the places they can go
    // these are not valid, but it should error later than json parsing
    let _: IndexPackage<'_> = serde_json::from_str(
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
