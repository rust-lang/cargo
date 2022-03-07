use std::collections::hash_map::HashMap;
use std::fmt;

use crate::core::package::PackageSet;
use crate::core::{Dependency, Package, PackageId, Summary};
use crate::util::{CargoResult, Config};

mod source_id;

pub use self::source_id::{GitReference, SourceId};

/// Something that finds and downloads remote packages based on names and versions.
pub trait Source {
    /// Returns the `SourceId` corresponding to this source.
    fn source_id(&self) -> SourceId;

    /// Returns the replaced `SourceId` corresponding to this source.
    fn replaced_source_id(&self) -> SourceId {
        self.source_id()
    }

    /// Returns whether or not this source will return summaries with
    /// checksums listed.
    fn supports_checksums(&self) -> bool;

    /// Returns whether or not this source will return summaries with
    /// the `precise` field in the source id listed.
    fn requires_precise(&self) -> bool;

    /// Attempts to find the packages that match a dependency request.
    fn query(&mut self, dep: &Dependency, f: &mut dyn FnMut(Summary)) -> CargoResult<()>;

    /// Attempts to find the packages that are close to a dependency request.
    /// Each source gets to define what `close` means for it.
    /// Path/Git sources may return all dependencies that are at that URI,
    /// whereas an `Index` source may return dependencies that have the same canonicalization.
    fn fuzzy_query(&mut self, dep: &Dependency, f: &mut dyn FnMut(Summary)) -> CargoResult<()>;

    fn query_vec(&mut self, dep: &Dependency) -> CargoResult<Vec<Summary>> {
        let mut ret = Vec::new();
        self.query(dep, &mut |s| ret.push(s))?;
        Ok(ret)
    }

    /// Performs any network operations required to get the entire list of all names,
    /// versions and dependencies of packages managed by the `Source`.
    fn update(&mut self) -> CargoResult<()>;

    /// Fetches the full package for each name and version specified.
    fn download(&mut self, package: PackageId) -> CargoResult<MaybePackage>;

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

    fn finish_download(&mut self, package: PackageId, contents: Vec<u8>) -> CargoResult<Package>;

    /// Generates a unique string which represents the fingerprint of the
    /// current state of the source.
    ///
    /// This fingerprint is used to determine the "fresheness" of the source
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
    fn is_yanked(&mut self, _pkg: PackageId) -> CargoResult<bool>;
}

pub enum MaybePackage {
    Ready(Package),
    Download { url: String, descriptor: String },
}

impl<'a, T: Source + ?Sized + 'a> Source for Box<T> {
    /// Forwards to `Source::source_id`.
    fn source_id(&self) -> SourceId {
        (**self).source_id()
    }

    /// Forwards to `Source::replaced_source_id`.
    fn replaced_source_id(&self) -> SourceId {
        (**self).replaced_source_id()
    }

    /// Forwards to `Source::supports_checksums`.
    fn supports_checksums(&self) -> bool {
        (**self).supports_checksums()
    }

    /// Forwards to `Source::requires_precise`.
    fn requires_precise(&self) -> bool {
        (**self).requires_precise()
    }

    /// Forwards to `Source::query`.
    fn query(&mut self, dep: &Dependency, f: &mut dyn FnMut(Summary)) -> CargoResult<()> {
        (**self).query(dep, f)
    }

    /// Forwards to `Source::query`.
    fn fuzzy_query(&mut self, dep: &Dependency, f: &mut dyn FnMut(Summary)) -> CargoResult<()> {
        (**self).fuzzy_query(dep, f)
    }

    /// Forwards to `Source::update`.
    fn update(&mut self) -> CargoResult<()> {
        (**self).update()
    }

    /// Forwards to `Source::download`.
    fn download(&mut self, id: PackageId) -> CargoResult<MaybePackage> {
        (**self).download(id)
    }

    fn finish_download(&mut self, id: PackageId, data: Vec<u8>) -> CargoResult<Package> {
        (**self).finish_download(id, data)
    }

    /// Forwards to `Source::fingerprint`.
    fn fingerprint(&self, pkg: &Package) -> CargoResult<String> {
        (**self).fingerprint(pkg)
    }

    /// Forwards to `Source::verify`.
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

    fn is_yanked(&mut self, pkg: PackageId) -> CargoResult<bool> {
        (**self).is_yanked(pkg)
    }
}

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

    fn query(&mut self, dep: &Dependency, f: &mut dyn FnMut(Summary)) -> CargoResult<()> {
        (**self).query(dep, f)
    }

    fn fuzzy_query(&mut self, dep: &Dependency, f: &mut dyn FnMut(Summary)) -> CargoResult<()> {
        (**self).fuzzy_query(dep, f)
    }

    fn update(&mut self) -> CargoResult<()> {
        (**self).update()
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

    fn is_yanked(&mut self, pkg: PackageId) -> CargoResult<bool> {
        (**self).is_yanked(pkg)
    }
}

/// A `HashMap` of `SourceId` -> `Box<Source>`.
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

    /// Like `HashMap::insert`, but derives the `SourceId` key from the `Source`.
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
