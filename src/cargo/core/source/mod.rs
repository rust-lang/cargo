use std::collections::hash_map::{HashMap, Values, IterMut};

use core::{Package, PackageId, Registry};
use util::CargoResult;

mod source_id;

pub use self::source_id::{SourceId, GitReference};

/// A Source finds and downloads remote packages based on names and
/// versions.
pub trait Source: Registry {
    /// Returns the `SourceId` corresponding to this source
    fn source_id(&self) -> &SourceId;

    /// The update method performs any network operations required to
    /// get the entire list of all names, versions and dependencies of
    /// packages managed by the Source.
    fn update(&mut self) -> CargoResult<()>;

    /// The download method fetches the full package for each name and
    /// version specified.
    fn download(&mut self, package: &PackageId) -> CargoResult<Package>;

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
    fn verify(&self, _pkg: &PackageId) -> CargoResult<()> {
        Ok(())
    }
}

impl<'a, T: Source + ?Sized + 'a> Source for Box<T> {
    /// Forwards to `Source::source_id`
    fn source_id(&self) -> &SourceId {
        (**self).source_id()
    }

    /// Forwards to `Source::update`
    fn update(&mut self) -> CargoResult<()> {
        (**self).update()
    }

    /// Forwards to `Source::download`
    fn download(&mut self, id: &PackageId) -> CargoResult<Package> {
        (**self).download(id)
    }

    /// Forwards to `Source::fingerprint`
    fn fingerprint(&self, pkg: &Package) -> CargoResult<String> {
        (**self).fingerprint(pkg)
    }

    /// Forwards to `Source::verify`
    fn verify(&self, pkg: &PackageId) -> CargoResult<()> {
        (**self).verify(pkg)
    }
}

/// A `HashMap` of `SourceId` -> `Box<Source>`
#[derive(Default)]
pub struct SourceMap<'src> {
    map: HashMap<SourceId, Box<Source + 'src>>,
}

/// A `std::collection::hash_map::Values` for `SourceMap`
pub type Sources<'a, 'src> = Values<'a, SourceId, Box<Source + 'src>>;

/// A `std::collection::hash_map::IterMut` for `SourceMap`
pub struct SourcesMut<'a, 'src: 'a> {
    inner: IterMut<'a, SourceId, Box<Source + 'src>>,
}

impl<'src> SourceMap<'src> {
    /// Create an empty map
    pub fn new() -> SourceMap<'src> {
        SourceMap { map: HashMap::new() }
    }

    /// Like `HashMap::contains_key`
    pub fn contains(&self, id: &SourceId) -> bool {
        self.map.contains_key(id)
    }

    /// Like `HashMap::get`
    pub fn get(&self, id: &SourceId) -> Option<&(Source + 'src)> {
        let source = self.map.get(id);

        source.map(|s| {
            let s: &(Source + 'src) = &**s;
            s
        })
    }

    /// Like `HashMap::get_mut`
    pub fn get_mut(&mut self, id: &SourceId) -> Option<&mut (Source + 'src)> {
        self.map.get_mut(id).map(|s| {
            let s: &mut (Source + 'src) = &mut **s;
            s
        })
    }

    /// Like `HashMap::get`, but first calculates the `SourceId` from a
    /// `PackageId`
    pub fn get_by_package_id(&self, pkg_id: &PackageId) -> Option<&(Source + 'src)> {
        self.get(pkg_id.source_id())
    }

    /// Like `HashMap::insert`, but derives the SourceId key from the Source
    pub fn insert(&mut self, source: Box<Source + 'src>) {
        let id = source.source_id().clone();
        self.map.insert(id, source);
    }

    /// Like `HashMap::is_empty`
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Like `HashMap::len`
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Like `HashMap::values`
    pub fn sources<'a>(&'a self) -> Sources<'a, 'src> {
        self.map.values()
    }

    /// Like `HashMap::iter_mut`
    pub fn sources_mut<'a>(&'a mut self) -> SourcesMut<'a, 'src> {
        SourcesMut { inner: self.map.iter_mut() }
    }
}

impl<'a, 'src> Iterator for SourcesMut<'a, 'src> {
    type Item = (&'a SourceId, &'a mut (Source + 'src));
    fn next(&mut self) -> Option<(&'a SourceId, &'a mut (Source + 'src))> {
        self.inner.next().map(|(a, b)| (a, &mut **b))
    }
}

