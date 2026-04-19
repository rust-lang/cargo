//! [`Source`] trait for sources of Cargo packages.

use std::collections::hash_map::HashMap;
use std::fmt;
use std::rc::Rc;

use crate::core::SourceId;
use crate::core::{Dependency, Package, PackageId};
use crate::sources::IndexSummary;
use crate::util::CargoResult;

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
#[async_trait::async_trait(?Send)]
pub trait Source {
    /// Returns the [`SourceId`] corresponding to this source.
    fn source_id(&self) -> SourceId;

    /// Returns the replaced [`SourceId`] corresponding to this source.
    fn replaced_source_id(&self) -> SourceId {
        self.source_id()
    }

    /// Returns whether or not this source will return [`IndexSummary`] items with
    /// checksums listed.
    fn supports_checksums(&self) -> bool;

    /// Returns whether or not this source will return [`IndexSummary`] items with
    /// the `precise` field in the [`SourceId`] listed.
    fn requires_precise(&self) -> bool;

    /// Attempts to find the packages that match a dependency request.
    ///
    /// The `f` argument is expected to get called when any [`IndexSummary`] becomes available.
    async fn query(
        &self,
        dep: &Dependency,
        kind: QueryKind,
        f: &mut dyn FnMut(IndexSummary),
    ) -> CargoResult<()>;

    /// Gathers the result from [`Source::query`] as a list of [`IndexSummary`] items
    /// when they become available.
    async fn query_vec(&self, dep: &Dependency, kind: QueryKind) -> CargoResult<Vec<IndexSummary>> {
        let mut ret = Vec::new();
        self.query(dep, kind, &mut |s| ret.push(s))
            .await
            .map(|()| ret)
    }

    /// Ensure that the source is fully up-to-date for the current session on the next query.
    fn invalidate_cache(&self);

    /// If quiet, the source should not display any progress or status messages.
    fn set_quiet(&mut self, quiet: bool);

    /// Starts the process to fetch a [`Package`] for the given [`PackageId`].
    ///
    /// If the source already has the package available on disk, then it
    /// should return immediately with [`MaybePackage::Ready`] with the
    /// [`Package`]. Otherwise it should return a [`MaybePackage::Download`]
    /// to indicate the URL to download the package (this is for remote
    /// registry sources only).
    ///
    /// In the case where [`MaybePackage::Download`] is returned, then the
    /// package downloader will call [`Source::finish_download`] after the
    /// download has finished.
    async fn download(&self, package: PackageId) -> CargoResult<MaybePackage>;

    /// Gives the source the downloaded `.crate` file.
    ///
    /// When a source has returned [`MaybePackage::Download`] in the
    /// [`Source::download`] method, then this function will be called with
    /// the results of the download of the given URL. The source is
    /// responsible for saving to disk, and returning the appropriate
    /// [`Package`].
    async fn finish_download(&self, pkg_id: PackageId, contents: Vec<u8>) -> CargoResult<Package>;

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
    ///
    /// Builtin replacement of `crates.io` doesn't count as replacement here.
    fn is_replaced(&self) -> bool {
        false
    }

    /// Add a number of crates that should be whitelisted for showing up during
    /// queries, even if they are yanked. Currently only applies to registry
    /// sources.
    fn add_to_yanked_whitelist(&self, pkgs: &[PackageId]);

    /// Query if a package is yanked. Only registry sources can mark packages
    /// as yanked. This ignores the yanked whitelist.
    async fn is_yanked(&self, pkg: PackageId) -> CargoResult<bool>;
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
    /// whereas an `Registry` source may return dependencies that are yanked or invalid.
    RejectedVersions,
    /// A query for packages close to the given dependency requirement.
    ///
    /// Each source gets to define what `close` means for it.
    ///
    /// Path/Git sources may return all dependencies that are at that URI,
    /// whereas an `Registry` source may return dependencies that have the same
    /// canonicalization.
    AlternativeNames,
    /// Match a dependency in all ways and will normalize the package name.
    /// Each source defines what normalizing means.
    Normalized,
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
#[async_trait::async_trait(?Send)]
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

    async fn query(
        &self,
        dep: &Dependency,
        kind: QueryKind,
        f: &mut dyn FnMut(IndexSummary),
    ) -> CargoResult<()> {
        (**self).query(dep, kind, f).await
    }

    fn invalidate_cache(&self) {
        (**self).invalidate_cache()
    }

    fn set_quiet(&mut self, quiet: bool) {
        (**self).set_quiet(quiet)
    }

    async fn download(&self, id: PackageId) -> CargoResult<MaybePackage> {
        (**self).download(id).await
    }

    async fn finish_download(&self, id: PackageId, data: Vec<u8>) -> CargoResult<Package> {
        (**self).finish_download(id, data).await
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

    fn add_to_yanked_whitelist(&self, pkgs: &[PackageId]) {
        (**self).add_to_yanked_whitelist(pkgs);
    }

    async fn is_yanked(&self, pkg: PackageId) -> CargoResult<bool> {
        (**self).is_yanked(pkg).await
    }
}

/// A [`HashMap`] of [`SourceId`] to `Box<Source>`.
#[derive(Default)]
pub struct SourceMap<'src> {
    map: HashMap<SourceId, Rc<dyn Source + 'src>>,
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
    pub fn get(&self, id: SourceId) -> Option<&Rc<dyn Source + 'src>> {
        self.map.get(&id)
    }

    /// Like `HashMap::insert`, but derives the [`SourceId`] key from the [`Source`].
    pub fn insert(&mut self, source: Box<dyn Source + 'src>) {
        let id = source.source_id();
        self.map.insert(id, source.into());
    }

    /// Like `HashMap::len`.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Like `HashMap::iter`.
    pub fn iter<'a>(&'a self) -> impl Iterator<Item = (&'a SourceId, &'a (dyn Source + 'src))> {
        self.map.iter().map(|(a, b)| (a, &**b))
    }

    /// Merge the given map into self.
    pub fn add_source_map(&mut self, other: SourceMap<'src>) {
        for (key, value) in other.map {
            self.map.entry(key).or_insert(value);
        }
    }
}
