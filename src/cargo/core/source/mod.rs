//! Fundamental types and traits for sources of Cargo packages.
//!
//! A source is a provider that contains source files and metadata of packages.
//! It provides a number of methods to fetch those package informations, for
//! example, querying metadata or downloading files for a package. These
//! informations then can be used as dependencies for other Cargo packages.
//!
//! Notably, this module contains
//!
//! * [`Source`] trait as an abstraction of different sources
//! * [`SourceMap`] struct as a map of all available sources
//! * [`SourceId`] struct as an unique identifier for a certain source
//!
//! For implementations of `Source` trait, see [`crate::sources`].

use std::collections::hash_map::HashMap;
use std::fmt;
use std::task::Poll;

use crate::core::package::PackageSet;
use crate::core::{Dependency, Package, PackageId, Summary};
use crate::util::{CargoResult, Config};

mod source_id;

pub use self::source_id::{GitReference, SourceId};

/// An abstraction of different sources of Cargo packages.
///
/// The [`Source`] trait generalizes the API to interact with these providers.
/// For example,
///
/// * [`Source::query`] is for querying package metadata on a given
///   [`Dependency`] requested by a Cargo manifest.
/// * [`Source::download`] is for fetching the full package information on
///   given names and versions.
/// * [`Source::source_id`] is for defining an unique identifier of a source to
///   distinguish one source from another, keeping Cargo safe from [dependency
///   confusion attack].
///
/// Normally, developers don't need to implement their own [`Source`]s. Cargo
/// provides several kinds of sources implementations that should cover almost
/// all use cases. See [`crate::sources`] for implementations provided by Cargo.
///
/// [dependency confusion attack]: https://medium.com/@alex.birsan/dependency-confusion-4a5d60fec610
pub trait Source {
    /// Returns the [`SourceId`] corresponding to this source.
    fn source_id(&self) -> SourceId;

    /// Returns the replaced [`SourceId`] corresponding to this source.
    fn replaced_source_id(&self) -> SourceId {
        self.source_id()
    }

    /// Returns whether or not this source will return [`Summary`] items with
    /// checksums listed.
    fn supports_checksums(&self) -> bool;

    /// Returns whether or not this source will return [`Summary`] items with
    /// the `precise` field in the [`SourceId`] listed.
    fn requires_precise(&self) -> bool;

    /// Attempts to find the packages that match a dependency request.
    ///
    /// Usually you should call [`Source::block_until_ready`] somewhere and
    /// wait until package informations become available. Otherwise any query
    /// may return a [`Poll::Pending`].
    ///
    /// The `f` argument is expected to get called when any [`Summary`] becomes available.
    fn query(
        &mut self,
        dep: &Dependency,
        kind: QueryKind,
        f: &mut dyn FnMut(Summary),
    ) -> Poll<CargoResult<()>>;

    /// Gathers the result from [`Source::query`] as a list of [`Summary`] items
    /// when they become available.
    fn query_vec(&mut self, dep: &Dependency, kind: QueryKind) -> Poll<CargoResult<Vec<Summary>>> {
        let mut ret = Vec::new();
        self.query(dep, kind, &mut |s| ret.push(s)).map_ok(|_| ret)
    }

    /// Ensure that the source is fully up-to-date for the current session on the next query.
    fn invalidate_cache(&mut self);

    /// If quiet, the source should not display any progress or status messages.
    fn set_quiet(&mut self, quiet: bool);

    /// Fetches the full package for each name and version specified.
    fn download(&mut self, package: PackageId) -> CargoResult<MaybePackage>;

    /// Fetches the full package **immediately** for each name and version specified.
    fn download_now(self: Box<Self>, package: PackageId, config: &Config) -> CargoResult<Package>
    where
        Self: std::marker::Sized,
    {
        let mut sources = SourceMap::new();
        sources.insert(self);
        let pkg_set = PackageSet::new(&[package], sources, config)?;
        let pkg = pkg_set.get_one(package)?;
        Ok(Package::clone(pkg))
    }

    /// Finalizes the download contents of the given [`PackageId`] to a [`Package`].
    fn finish_download(&mut self, pkg_id: PackageId, contents: Vec<u8>) -> CargoResult<Package>;

    /// Generates a unique string which represents the fingerprint of the
    /// current state of the source.
    ///
    /// This fingerprint is used to determine the "freshness" of the source
    /// later on. It must be guaranteed that the fingerprint of a source is
    /// constant if and only if the output product will remain constant.
    ///
    /// The `pkg` argument is the package which this fingerprint should only be
    /// interested in for when this source may contain multiple packages.
    fn fingerprint(&self, pkg: &Package) -> CargoResult<String>;

    /// If this source supports it, verifies the source of the package
    /// specified.
    ///
    /// Note that the source may also have performed other checksum-based
    /// verification during the `download` step, but this is intended to be run
    /// just before a crate is compiled so it may perform more expensive checks
    /// which may not be cacheable.
    fn verify(&self, _pkg: PackageId) -> CargoResult<()> {
        Ok(())
    }

    /// Describes this source in a human readable fashion, used for display in
    /// resolver error messages currently.
    fn describe(&self) -> String;

    /// Returns whether a source is being replaced by another here.
    fn is_replaced(&self) -> bool {
        false
    }

    /// Add a number of crates that should be whitelisted for showing up during
    /// queries, even if they are yanked. Currently only applies to registry
    /// sources.
    fn add_to_yanked_whitelist(&mut self, pkgs: &[PackageId]);

    /// Query if a package is yanked. Only registry sources can mark packages
    /// as yanked. This ignores the yanked whitelist.
    fn is_yanked(&mut self, _pkg: PackageId) -> Poll<CargoResult<bool>>;

    /// Block until all outstanding [`Poll::Pending`] requests are [`Poll::Ready`].
    ///
    /// After calling this function, the source should return `Poll::Ready` for
    /// any queries that previously returned `Poll::Pending`.
    ///
    /// If no queries previously returned `Poll::Pending`, and [`Source::invalidate_cache`]
    /// was not called, this function should be a no-op.
    fn block_until_ready(&mut self) -> CargoResult<()>;
}

/// Defines how a dependency query will be performed for a [`Source`].
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum QueryKind {
    /// A query for packages exactly matching the given dependency requirement.
    ///
    /// Each source gets to define what `exact` means for it.
    Exact,
    /// A query for packages close to the given dependency requirement.
    ///
    /// Each source gets to define what `close` means for it.
    ///
    /// Path/Git sources may return all dependencies that are at that URI,
    /// whereas an `Registry` source may return dependencies that have the same
    /// canonicalization.
    Fuzzy,
}

/// A download status that represents if a [`Package`] has already been
/// downloaded, or if not then a location to download.
pub enum MaybePackage {
    /// The [`Package`] is already downloaded.
    Ready(Package),
    /// Not yet downloaded. Here is the URL to download the [`Package`] from.
    Download {
        /// URL to download the content.
        url: String,
        /// Text to display to the user of what is being downloaded.
        descriptor: String,
        /// Authorization data that may be required to attach when downloading.
        authorization: Option<String>,
    },
}

/// A blanket implementation forwards all methods to [`Source`].
impl<'a, T: Source + ?Sized + 'a> Source for Box<T> {
    fn source_id(&self) -> SourceId {
        (**self).source_id()
    }

    fn replaced_source_id(&self) -> SourceId {
        (**self).replaced_source_id()
    }

    fn supports_checksums(&self) -> bool {
        (**self).supports_checksums()
    }

    fn requires_precise(&self) -> bool {
        (**self).requires_precise()
    }

    fn query(
        &mut self,
        dep: &Dependency,
        kind: QueryKind,
        f: &mut dyn FnMut(Summary),
    ) -> Poll<CargoResult<()>> {
        (**self).query(dep, kind, f)
    }

    fn invalidate_cache(&mut self) {
        (**self).invalidate_cache()
    }

    fn set_quiet(&mut self, quiet: bool) {
        (**self).set_quiet(quiet)
    }

    fn download(&mut self, id: PackageId) -> CargoResult<MaybePackage> {
        (**self).download(id)
    }

    fn finish_download(&mut self, id: PackageId, data: Vec<u8>) -> CargoResult<Package> {
        (**self).finish_download(id, data)
    }

    fn fingerprint(&self, pkg: &Package) -> CargoResult<String> {
        (**self).fingerprint(pkg)
    }

    fn verify(&self, pkg: PackageId) -> CargoResult<()> {
        (**self).verify(pkg)
    }

    fn describe(&self) -> String {
        (**self).describe()
    }

    fn is_replaced(&self) -> bool {
        (**self).is_replaced()
    }

    fn add_to_yanked_whitelist(&mut self, pkgs: &[PackageId]) {
        (**self).add_to_yanked_whitelist(pkgs);
    }

    fn is_yanked(&mut self, pkg: PackageId) -> Poll<CargoResult<bool>> {
        (**self).is_yanked(pkg)
    }

    fn block_until_ready(&mut self) -> CargoResult<()> {
        (**self).block_until_ready()
    }
}

/// A blanket implementation forwards all methods to [`Source`].
impl<'a, T: Source + ?Sized + 'a> Source for &'a mut T {
    fn source_id(&self) -> SourceId {
        (**self).source_id()
    }

    fn replaced_source_id(&self) -> SourceId {
        (**self).replaced_source_id()
    }

    fn supports_checksums(&self) -> bool {
        (**self).supports_checksums()
    }

    fn requires_precise(&self) -> bool {
        (**self).requires_precise()
    }

    fn query(
        &mut self,
        dep: &Dependency,
        kind: QueryKind,
        f: &mut dyn FnMut(Summary),
    ) -> Poll<CargoResult<()>> {
        (**self).query(dep, kind, f)
    }

    fn invalidate_cache(&mut self) {
        (**self).invalidate_cache()
    }

    fn set_quiet(&mut self, quiet: bool) {
        (**self).set_quiet(quiet)
    }

    fn download(&mut self, id: PackageId) -> CargoResult<MaybePackage> {
        (**self).download(id)
    }

    fn finish_download(&mut self, id: PackageId, data: Vec<u8>) -> CargoResult<Package> {
        (**self).finish_download(id, data)
    }

    fn fingerprint(&self, pkg: &Package) -> CargoResult<String> {
        (**self).fingerprint(pkg)
    }

    fn verify(&self, pkg: PackageId) -> CargoResult<()> {
        (**self).verify(pkg)
    }

    fn describe(&self) -> String {
        (**self).describe()
    }

    fn is_replaced(&self) -> bool {
        (**self).is_replaced()
    }

    fn add_to_yanked_whitelist(&mut self, pkgs: &[PackageId]) {
        (**self).add_to_yanked_whitelist(pkgs);
    }

    fn is_yanked(&mut self, pkg: PackageId) -> Poll<CargoResult<bool>> {
        (**self).is_yanked(pkg)
    }

    fn block_until_ready(&mut self) -> CargoResult<()> {
        (**self).block_until_ready()
    }
}

/// A [`HashMap`] of [`SourceId`] to `Box<Source>`.
#[derive(Default)]
pub struct SourceMap<'src> {
    map: HashMap<SourceId, Box<dyn Source + 'src>>,
}

// `impl Debug` on source requires specialization, if even desirable at all.
impl<'src> fmt::Debug for SourceMap<'src> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SourceMap ")?;
        f.debug_set().entries(self.map.keys()).finish()
    }
}

impl<'src> SourceMap<'src> {
    /// Creates an empty map.
    pub fn new() -> SourceMap<'src> {
        SourceMap {
            map: HashMap::new(),
        }
    }

    /// Like `HashMap::get`.
    pub fn get(&self, id: SourceId) -> Option<&(dyn Source + 'src)> {
        self.map.get(&id).map(|s| s.as_ref())
    }

    /// Like `HashMap::get_mut`.
    pub fn get_mut(&mut self, id: SourceId) -> Option<&mut (dyn Source + 'src)> {
        self.map.get_mut(&id).map(|s| s.as_mut())
    }

    /// Like `HashMap::insert`, but derives the [`SourceId`] key from the [`Source`].
    pub fn insert(&mut self, source: Box<dyn Source + 'src>) {
        let id = source.source_id();
        self.map.insert(id, source);
    }

    /// Like `HashMap::len`.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Like `HashMap::iter_mut`.
    pub fn sources_mut<'a>(
        &'a mut self,
    ) -> impl Iterator<Item = (&'a SourceId, &'a mut (dyn Source + 'src))> {
        self.map.iter_mut().map(|(a, b)| (a, &mut **b))
    }

    /// Merge the given map into self.
    pub fn add_source_map(&mut self, other: SourceMap<'src>) {
        for (key, value) in other.map {
            self.map.entry(key).or_insert(value);
        }
    }
}
