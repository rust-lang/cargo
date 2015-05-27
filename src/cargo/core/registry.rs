use std::collections::HashSet;
use std::collections::hash_map::HashMap;

use core::{Source, SourceId, SourceMap, Summary, Dependency, PackageId, Package};
use util::{CargoResult, ChainError, Config, human, profile};

/// Source of informations about a group of packages.
///
/// See also `core::Source`.
pub trait Registry {
    /// Attempt to find the packages that match a dependency request.
    fn query(&mut self, name: &Dependency) -> CargoResult<Vec<Summary>>;
}

impl Registry for Vec<Summary> {
    fn query(&mut self, dep: &Dependency) -> CargoResult<Vec<Summary>> {
        Ok(self.iter().filter(|summary| dep.matches(*summary))
               .map(|summary| summary.clone()).collect())
    }
}

/// This structure represents a registry of known packages. It internally
/// contains a number of `Box<Source>` instances which are used to load a
/// `Package` from.
///
/// The resolution phase of Cargo uses this to drive knowledge about new
/// packages as well as querying for lists of new packages. It is here that
/// sources are updated and (e.g. network operations) as well as overrides are
/// handled.
///
/// The general idea behind this registry is that it is centered around the
/// `SourceMap` structure contained within which is a mapping of a `SourceId` to
/// a `Source`. Each `Source` in the map has been updated (using network
/// operations if necessary) and is ready to be queried for packages.
pub struct PackageRegistry<'cfg> {
    sources: SourceMap<'cfg>,
    config: &'cfg Config,

    // A list of sources which are considered "overrides" which take precedent
    // when querying for packages.
    overrides: Vec<SourceId>,

    // Note that each SourceId does not take into account its `precise` field
    // when hashing or testing for equality. When adding a new `SourceId`, we
    // want to avoid duplicates in the `SourceMap` (to prevent re-updating the
    // same git repo twice for example), but we also want to ensure that the
    // loaded source is always updated.
    //
    // Sources with a `precise` field normally don't need to be updated because
    // their contents are already on disk, but sources without a `precise` field
    // almost always need to be updated. If we have a cached `Source` for a
    // precise `SourceId`, then when we add a new `SourceId` that is not precise
    // we want to ensure that the underlying source is updated.
    //
    // This is basically a long-winded way of saying that we want to know
    // precisely what the keys of `sources` are, so this is a mapping of key to
    // what exactly the key is.
    source_ids: HashMap<SourceId, (SourceId, Kind)>,

    locked: HashMap<SourceId, HashMap<String, Vec<(PackageId, Vec<PackageId>)>>>,
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum Kind {
    Override,
    Locked,
    Normal,
}

impl<'cfg> PackageRegistry<'cfg> {
    pub fn new(config: &'cfg Config) -> PackageRegistry<'cfg> {
        PackageRegistry {
            sources: SourceMap::new(),
            source_ids: HashMap::new(),
            overrides: vec!(),
            config: config,
            locked: HashMap::new(),
        }
    }

    pub fn get(&mut self, package_ids: &[PackageId]) -> CargoResult<Vec<Package>> {
        trace!("getting packages; sources={}", self.sources.len());

        // TODO: Only call source with package ID if the package came from the
        // source
        let mut ret = Vec::new();

        for (_, source) in self.sources.sources_mut() {
            try!(source.download(package_ids));
            let packages = try!(source.get(package_ids));

            ret.extend(packages.into_iter());
        }

        // TODO: Return earlier if fail
        assert!(package_ids.len() == ret.len(),
                "could not get packages from registry; ids={:?}; ret={:?}",
                package_ids, ret);

        Ok(ret)
    }

    pub fn move_sources(self) -> SourceMap<'cfg> {
        self.sources
    }

    fn ensure_loaded(&mut self, namespace: &SourceId) -> CargoResult<()> {
        match self.source_ids.get(namespace) {
            // We've previously loaded this source, and we've already locked it,
            // so we're not allowed to change it even if `namespace` has a
            // slightly different precise version listed.
            Some(&(_, Kind::Locked)) => {
                debug!("load/locked   {}", namespace);
                return Ok(())
            }

            // If the previous source was not a precise source, then we can be
            // sure that it's already been updated if we've already loaded it.
            Some(&(ref previous, _)) if previous.precise().is_none() => {
                debug!("load/precise  {}", namespace);
                return Ok(())
            }

            // If the previous source has the same precise version as we do,
            // then we're done, otherwise we need to need to move forward
            // updating this source.
            Some(&(ref previous, _)) => {
                if previous.precise() == namespace.precise() {
                    debug!("load/match    {}", namespace);
                    return Ok(())
                }
                debug!("load/mismatch {}", namespace);
            }
            None => {
                debug!("load/missing  {}", namespace);
            }
        }

        try!(self.load(namespace, Kind::Normal));
        Ok(())
    }

    pub fn preload(&mut self, id: &SourceId, source: Box<Source + 'cfg>) {
        self.sources.insert(id, source);
        self.source_ids.insert(id.clone(), (id.clone(), Kind::Locked));
    }

    pub fn add_sources(&mut self, ids: &[SourceId]) -> CargoResult<()> {
        for id in ids.iter() {
            try!(self.load(id, Kind::Locked));
        }
        Ok(())
    }

    pub fn add_overrides(&mut self, ids: Vec<SourceId>) -> CargoResult<()> {
        for id in ids.iter() {
            try!(self.load(id, Kind::Override));
        }
        Ok(())
    }

    pub fn register_lock(&mut self, id: PackageId, deps: Vec<PackageId>) {
        let sub_map = self.locked.entry(id.source_id().clone())
                                 .or_insert(HashMap::new());
        let sub_vec = sub_map.entry(id.name().to_string())
                             .or_insert(Vec::new());
        sub_vec.push((id, deps));
    }

    fn load(&mut self, source_id: &SourceId, kind: Kind) -> CargoResult<()> {
        (|| {
            let mut source = source_id.load(self.config);

            // Ensure the source has fetched all necessary remote data.
            let p = profile::start(format!("updating: {}", source_id));
            try!(source.update());
            drop(p);

            if kind == Kind::Override {
                self.overrides.push(source_id.clone());
            }

            // Save off the source
            self.sources.insert(source_id, source);
            self.source_ids.insert(source_id.clone(), (source_id.clone(), kind));

            Ok(())
        }).chain_error(|| human(format!("Unable to update {}", source_id)))
    }

    fn query_overrides(&mut self, dep: &Dependency)
                       -> CargoResult<Vec<Summary>> {
        let mut seen = HashSet::new();
        let mut ret = Vec::new();
        for s in self.overrides.iter() {
            let src = self.sources.get_mut(s).unwrap();
            let dep = Dependency::new_override(dep.name(), s);
            ret.extend(try!(src.query(&dep)).into_iter().filter(|s| {
                seen.insert(s.name().to_string())
            }));
        }
        Ok(ret)
    }

    // This function is used to transform a summary to another locked summary if
    // possible. This is where the the concept of a lockfile comes into play.
    //
    // If a summary points at a package id which was previously locked, then we
    // override the summary's id itself as well as all dependencies to be
    // rewritten to the locked versions. This will transform the summary's
    // source to a precise source (listed in the locked version) as well as
    // transforming all of the dependencies from range requirements on imprecise
    // sources to exact requirements on precise sources.
    //
    // If a summary does not point at a package id which was previously locked,
    // we still want to avoid updating as many dependencies as possible to keep
    // the graph stable. In this case we map all of the summary's dependencies
    // to be rewritten to a locked version wherever possible. If we're unable to
    // map a dependency though, we just pass it on through.
    fn lock(&self, summary: Summary) -> Summary {
        let pair = self.locked.get(summary.source_id()).and_then(|map| {
            map.get(summary.name())
        }).and_then(|vec| {
            vec.iter().find(|&&(ref id, _)| id == summary.package_id())
        });

        // Lock the summary's id if possible
        let summary = match pair {
            Some(&(ref precise, _)) => summary.override_id(precise.clone()),
            None => summary,
        };
        summary.map_dependencies(|dep| {
            match pair {
                // If we've got a known set of overrides for this summary, then
                // one of a few cases can arise:
                //
                // 1. We have a lock entry for this dependency from the same
                //    source as its listed as coming from. In this case we make
                //    sure to lock to precisely the given package id.
                //
                // 2. We have a lock entry for this dependency, but it's from a
                //    different source than what's listed, or the version
                //    requirement has changed. In this case we must discard the
                //    locked version because the dependency needs to be
                //    re-resolved.
                //
                // 3. We don't have a lock entry for this dependency, in which
                //    case it was likely an optional dependency which wasn't
                //    included previously so we just pass it through anyway.
                Some(&(_, ref deps)) => {
                    match deps.iter().find(|d| d.name() == dep.name()) {
                        Some(lock) => {
                            if dep.matches_id(lock) {
                                dep.lock_to(lock)
                            } else {
                                dep
                            }
                        }
                        None => dep,
                    }
                }

                // If this summary did not have a locked version, then we query
                // all known locked packages to see if they match this
                // dependency. If anything does then we lock it to that and move
                // on.
                None => {
                    let v = self.locked.get(dep.source_id()).and_then(|map| {
                        map.get(dep.name())
                    }).and_then(|vec| {
                        vec.iter().find(|&&(ref id, _)| dep.matches_id(id))
                    });
                    match v {
                        Some(&(ref id, _)) => dep.lock_to(id),
                        None => dep
                    }
                }
            }
        })
    }
}

impl<'cfg> Registry for PackageRegistry<'cfg> {
    fn query(&mut self, dep: &Dependency) -> CargoResult<Vec<Summary>> {
        let overrides = try!(self.query_overrides(dep));

        let ret = if overrides.len() == 0 {
            // Ensure the requested source_id is loaded
            try!(self.ensure_loaded(dep.source_id()));
            let mut ret = Vec::new();
            for (id, src) in self.sources.sources_mut() {
                if id == dep.source_id() {
                    ret.extend(try!(src.query(dep)).into_iter());
                }
            }
            ret
        } else {
            overrides
        };

        // post-process all returned summaries to ensure that we lock all
        // relevant summaries to the right versions and sources
        Ok(ret.into_iter().map(|summary| self.lock(summary)).collect())
    }
}

#[cfg(test)]
pub mod test {
    use core::{Summary, Registry, Dependency};
    use util::{CargoResult};

    pub struct RegistryBuilder {
        summaries: Vec<Summary>,
        overrides: Vec<Summary>
    }

    impl RegistryBuilder {
        pub fn new() -> RegistryBuilder {
            RegistryBuilder { summaries: vec!(), overrides: vec!() }
        }

        pub fn summary(mut self, summary: Summary) -> RegistryBuilder {
            self.summaries.push(summary);
            self
        }

        pub fn summaries(mut self, summaries: Vec<Summary>) -> RegistryBuilder {
            self.summaries.extend(summaries.into_iter());
            self
        }

        pub fn add_override(mut self, summary: Summary) -> RegistryBuilder {
            self.overrides.push(summary);
            self
        }

        pub fn overrides(mut self, summaries: Vec<Summary>) -> RegistryBuilder {
            self.overrides.extend(summaries.into_iter());
            self
        }

        fn query_overrides(&self, dep: &Dependency) -> Vec<Summary> {
            self.overrides.iter()
                .filter(|s| s.name() == dep.name())
                .map(|s| s.clone())
                .collect()
        }
    }

    impl Registry for RegistryBuilder {
        fn query(&mut self, dep: &Dependency) -> CargoResult<Vec<Summary>> {
            debug!("querying; dep={:?}", dep);

            let overrides = self.query_overrides(dep);

            if overrides.is_empty() {
                self.summaries.query(dep)
            } else {
                Ok(overrides)
            }
        }
    }
}
