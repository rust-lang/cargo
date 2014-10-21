use std::collections::HashSet;

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

pub struct PackageRegistry<'a> {
    sources: SourceMap<'a>,
    overrides: Vec<SourceId>,
    config: &'a mut Config<'a>
}

impl<'a> PackageRegistry<'a> {
    pub fn new<'a>(config: &'a mut Config<'a>) -> PackageRegistry<'a> {
        PackageRegistry {
            sources: SourceMap::new(),
            overrides: vec!(),
            config: config
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

    pub fn add_sources(&mut self, ids: Vec<SourceId>) -> CargoResult<()> {
        for id in dedup(ids).iter() {
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
}

fn dedup(ids: Vec<SourceId>) -> Vec<SourceId> {
    let mut seen = vec!();

    for id in ids.into_iter() {
        if seen.contains(&id) { continue; }
        seen.push(id);
    }

    seen
}

impl<'a> Registry for PackageRegistry<'a> {
    fn query(&mut self, dep: &Dependency) -> CargoResult<Vec<Summary>> {
        let overrides = try!(self.query_overrides(dep));

        if overrides.len() == 0 {
            // Ensure the requested source_id is loaded
            try!(self.ensure_loaded(dep.get_source_id()));
            let mut ret = Vec::new();
            for src in self.sources.sources_mut() {
                ret.extend(try!(src.query(dep)).into_iter());
            }
            Ok(ret)
        } else {
            Ok(overrides)
        }
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
