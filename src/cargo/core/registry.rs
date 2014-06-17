use std::vec::Vec;
use core::{Source, SourceId, SourceSet, Summary, Dependency, PackageSet};
use util::CargoResult;

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
    pub fn new(sources: Vec<Box<Source>>, overrides: SourceSet) -> CargoResult<PackageRegistry> {
        Ok(PackageRegistry {
            sources: sources,
            overrides: try!(overrides.list()),
            summaries: vec!(),
            searched: vec!()
        })
    }

    fn ensure_loaded(&mut self, namespace: &SourceId) -> CargoResult<()> {
        if self.searched.contains(namespace) { return Ok(()); }
        self.load(namespace);
        Ok(())
    }

    fn load(&mut self, namespace: &SourceId) -> CargoResult<()> {
        let source = namespace.load();

        // Ensure the source has fetched all necessary remote data.
        try!(source.update());

        // Get the summaries
        for summary in (try!(source.list())).iter() {
            assert!(!self.summaries.contains(summary), "duplicate summaries");
            self.summaries.push(summary.clone());
        }

        // Track that the source has been searched
        self.searched.push(namespace.clone());

        Ok(())
    }
}

impl Registry for PackageRegistry {
    fn query(&mut self, dep: &Dependency) -> CargoResult<Vec<Summary>> {
        let overrides = try!(self.overrides.query(dep));

        if overrides.is_empty() {
            // Ensure the requested namespace is loaded
            try!(self.ensure_loaded(dep.get_namespace()));
            self.summaries.query(dep)
        } else {
            Ok(overrides)
        }
    }
}
