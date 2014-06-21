use std::vec::Vec;
use core::{Source, SourceId, Summary, Dependency, PackageId, Package};
use util::{CargoResult, ChainError, Config, human};

pub trait Registry {
    fn query(&mut self, name: &Dependency) -> CargoResult<Vec<Summary>>;
}

impl Registry for Vec<Summary> {
    fn query(&mut self, name: &Dependency) -> CargoResult<Vec<Summary>> {
        Ok(self.iter()
            .filter(|summary| name.get_name() == summary.get_name())
            .map(|summary| summary.clone())
            .collect())
    }
}

pub struct PackageRegistry {
    sources: Vec<Box<Source>>,
    overrides: Vec<Summary>,
    summaries: Vec<Summary>,
    searched: Vec<SourceId>
}

impl PackageRegistry {
    pub fn new(source_ids: Vec<SourceId>,
               override_ids: Vec<SourceId>) -> CargoResult<PackageRegistry> {
        let mut reg = PackageRegistry::empty();

        for id in source_ids.iter() {
            try!(reg.load(id, false));
        }

        for id in override_ids.iter() {
            try!(reg.load(id, true));
        }

        Ok(reg)
    }

    fn empty() -> PackageRegistry {
        PackageRegistry {
            sources: vec!(),
            overrides: vec!(),
            summaries: vec!(),
            searched: vec!()
        }
    }

    pub fn get(&self, package_ids: &[PackageId]) -> CargoResult<Vec<Package>> {
        log!(5, "getting packags; sources={}; ids={}", self.sources.len(),
             package_ids);

        // TODO: Only call source with package ID if the package came from the
        // source
        let mut ret = Vec::new();

        for source in self.sources.iter() {
            try!(source.download(package_ids));
            let packages = try!(source.get(package_ids));

            ret.push_all_move(packages);
        }

        // TODO: Return earlier if fail
        assert!(package_ids.len() == ret.len(),
                "could not get packages from registry; ids={}", package_ids);

        Ok(ret)
    }

    fn ensure_loaded(&mut self, namespace: &SourceId) -> CargoResult<()> {
        if self.searched.contains(namespace) { return Ok(()); }
        try!(self.load(namespace, false));
        Ok(())
    }

    fn load(&mut self, namespace: &SourceId,
            override: bool) -> CargoResult<()> {

        (|| {
            let mut source = namespace.load(&try!(Config::new()));
            let dst = if override {&mut self.overrides} else {&mut self.summaries};

            // Ensure the source has fetched all necessary remote data.
            try!(source.update());

            // Get the summaries
            for summary in (try!(source.list())).iter() {
                assert!(!dst.contains(summary), "duplicate summaries");
                dst.push(summary.clone());
                // self.summaries.push(summary.clone());
            }

            // Save off the source
            self.sources.push(source);

            // Track that the source has been searched
            self.searched.push(namespace.clone());

            Ok(())
        }).chain_error(|| human(format!("Unable to update {}", namespace)))
    }
}

impl Registry for PackageRegistry {
    fn query(&mut self, dep: &Dependency) -> CargoResult<Vec<Summary>> {
        let overrides = try!(self.overrides.query(dep)); // this can never fail in practice

        if overrides.is_empty() {
            // Ensure the requested namespace is loaded
            try!(self.ensure_loaded(dep.get_namespace()));
            self.summaries.query(dep)
        } else {
            Ok(overrides)
        }
    }
}
