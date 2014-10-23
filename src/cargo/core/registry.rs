use std::collections::hashmap::{HashSet, HashMap, Occupied, Vacant};

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
        debug!("querying for {}, summaries={}", dep,
            self.iter().map(|s| s.get_package_id().to_string()).collect::<Vec<String>>());

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
pub struct PackageRegistry<'a> {
    sources: SourceMap<'a>,
    config: &'a mut Config<'a>,

    // A list of sources which are considered "overrides" which take precedent
    // when querying for packages.
    overrides: Vec<SourceId>,
    locked: HashMap<SourceId, HashMap<String, Vec<(PackageId, Vec<PackageId>)>>>,
}

impl<'a> PackageRegistry<'a> {
    pub fn new<'a>(config: &'a mut Config<'a>) -> PackageRegistry<'a> {
        PackageRegistry {
            sources: SourceMap::new(),
            overrides: vec!(),
            config: config,
            locked: HashMap::new(),
        }
    }

    pub fn get(&mut self, package_ids: &[PackageId]) -> CargoResult<Vec<Package>> {
        log!(5, "getting packags; sources={}; ids={}", self.sources.len(),
             package_ids);

        // TODO: Only call source with package ID if the package came from the
        // source
        let mut ret = Vec::new();

        for source in self.sources.sources_mut() {
            try!(source.download(package_ids));
            let packages = try!(source.get(package_ids));

            ret.extend(packages.into_iter());
        }

        // TODO: Return earlier if fail
        assert!(package_ids.len() == ret.len(),
                "could not get packages from registry; ids={}; ret={}",
                package_ids, ret);

        Ok(ret)
    }

    pub fn move_sources(self) -> SourceMap<'a> {
        self.sources
    }

    fn ensure_loaded(&mut self, namespace: &SourceId) -> CargoResult<()> {
        if self.sources.contains(namespace) { return Ok(()); }

        try!(self.load(namespace, false));
        Ok(())
    }

    pub fn add_sources(&mut self, ids: &[SourceId]) -> CargoResult<()> {
        for id in ids.iter() {
            try!(self.load(id, false));
        }
        Ok(())
    }

    pub fn add_overrides(&mut self, ids: Vec<SourceId>) -> CargoResult<()> {
        for id in ids.iter() {
            try!(self.load(id, true));
        }
        Ok(())
    }

    pub fn register_lock(&mut self, id: PackageId, deps: Vec<PackageId>) {
        let sub_map = match self.locked.entry(id.get_source_id().clone()) {
            Occupied(e) => e.into_mut(),
            Vacant(e) => e.set(HashMap::new()),
        };
        let sub_vec = match sub_map.entry(id.get_name().to_string()) {
            Occupied(e) => e.into_mut(),
            Vacant(e) => e.set(Vec::new()),
        };
        sub_vec.push((id, deps));
    }

    fn load(&mut self, source_id: &SourceId, is_override: bool) -> CargoResult<()> {
        (|| {
            let mut source = source_id.load(self.config);

            // Ensure the source has fetched all necessary remote data.
            let p = profile::start(format!("updating: {}", source_id));
            try!(source.update());
            drop(p);

            if is_override {
                self.overrides.push(source_id.clone());
            }

            // Save off the source
            self.sources.insert(source_id, source);

            Ok(())
        }).chain_error(|| human(format!("Unable to update {}", source_id)))
    }

    fn query_overrides(&mut self, dep: &Dependency)
                       -> CargoResult<Vec<Summary>> {
        let mut seen = HashSet::new();
        let mut ret = Vec::new();
        for s in self.overrides.iter() {
            let src = self.sources.get_mut(s).unwrap();
            let dep = Dependency::new_override(dep.get_name(), s);
            ret.extend(try!(src.query(&dep)).into_iter().filter(|s| {
                seen.insert(s.get_name().to_string())
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
        let pair = self.locked.find(summary.get_source_id()).and_then(|map| {
            map.find_equiv(&summary.get_name())
        }).and_then(|vec| {
            vec.iter().find(|&&(ref id, _)| id == summary.get_package_id())
        });

        // Lock the summary's id if possible
        let summary = match pair {
            Some(&(ref precise, _)) => summary.override_id(precise.clone()),
            None => summary,
        };
        summary.map_dependencies(|dep| {
            match pair {
                // If this summary has a locked version, then we need to lock
                // this dependency. If this dependency doesn't have a locked
                // version, then it was likely an optional dependency which
                // wasn't included and we just pass it through anyway.
                Some(&(_, ref deps)) => {
                    match deps.iter().find(|d| d.get_name() == dep.get_name()) {
                        Some(lock) => dep.lock_to(lock),
                        None => dep,
                    }
                }

                // If this summary did not have a locked version, then we query
                // all known locked packages to see if they match this
                // dependency. If anything does then we lock it to that and move
                // on.
                None => {
                    let v = self.locked.find(dep.get_source_id()).and_then(|map| {
                        map.find_equiv(&dep.get_name())
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

impl<'a> Registry for PackageRegistry<'a> {
    fn query(&mut self, dep: &Dependency) -> CargoResult<Vec<Summary>> {
        let overrides = try!(self.query_overrides(dep));

        let ret = if overrides.len() == 0 {
            // Ensure the requested source_id is loaded
            try!(self.ensure_loaded(dep.get_source_id()));
            let mut ret = Vec::new();
            for src in self.sources.sources_mut() {
                ret.extend(try!(src.query(dep)).into_iter());
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
                .filter(|s| s.get_name() == dep.get_name())
                .map(|s| s.clone())
                .collect()
        }
    }

    impl Registry for RegistryBuilder {
        fn query(&mut self, dep: &Dependency) -> CargoResult<Vec<Summary>> {
            debug!("querying; dep={}", dep);

            let overrides = self.query_overrides(dep);

            if overrides.is_empty() {
                self.summaries.query(dep)
            } else {
                Ok(overrides)
            }
        }
    }
}
