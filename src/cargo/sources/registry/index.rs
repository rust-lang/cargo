use std::collections::HashMap;
use std::io::prelude::*;
use std::fs::File;
use std::path::Path;

use rustc_serialize::json;

use core::dependency::{Dependency, DependencyInner, Kind};
use core::{SourceId, Summary, PackageId, Registry};
use sources::registry::{RegistryPackage, RegistryDependency, INDEX_LOCK};
use util::{CargoResult, ChainError, internal, Filesystem, Config};

pub struct RegistryIndex<'cfg> {
    source_id: SourceId,
    path: Filesystem,
    cache: HashMap<String, Vec<(Summary, bool)>>,
    hashes: HashMap<(String, String), String>, // (name, vers) => cksum
    config: &'cfg Config,
    locked: bool,
}

impl<'cfg> RegistryIndex<'cfg> {
    pub fn new(id: &SourceId,
               path: &Filesystem,
               config: &'cfg Config,
               locked: bool) -> RegistryIndex<'cfg> {
        RegistryIndex {
            source_id: id.clone(),
            path: path.clone(),
            cache: HashMap::new(),
            hashes: HashMap::new(),
            config: config,
            locked: locked,
        }
    }

    /// Return the hash listed for a specified PackageId.
    pub fn hash(&mut self, pkg: &PackageId) -> CargoResult<String> {
        let key = (pkg.name().to_string(), pkg.version().to_string());
        if let Some(s) = self.hashes.get(&key) {
            return Ok(s.clone())
        }
        // Ok, we're missing the key, so parse the index file to load it.
        try!(self.summaries(pkg.name()));
        self.hashes.get(&key).chain_error(|| {
            internal(format!("no hash listed for {}", pkg))
        }).map(|s| s.clone())
    }

    /// Parse the on-disk metadata for the package provided
    ///
    /// Returns a list of pairs of (summary, yanked) for the package name
    /// specified.
    pub fn summaries(&mut self, name: &str) -> CargoResult<&Vec<(Summary, bool)>> {
        if self.cache.contains_key(name) {
            return Ok(self.cache.get(name).unwrap());
        }
        let summaries = try!(self.load_summaries(name));
        let summaries = summaries.into_iter().filter(|summary| {
            summary.0.package_id().name() == name
        }).collect();
        self.cache.insert(name.to_string(), summaries);
        Ok(self.cache.get(name).unwrap())
    }

    fn load_summaries(&mut self, name: &str) -> CargoResult<Vec<(Summary, bool)>> {
        let (path, _lock) = if self.locked {
            let lock = self.path.open_ro(Path::new(INDEX_LOCK),
                                         self.config,
                                         "the registry index");
            match lock {
                Ok(lock) => {
                    (lock.path().parent().unwrap().to_path_buf(), Some(lock))
                }
                Err(_) => return Ok(Vec::new()),
            }
        } else {
            (self.path.clone().into_path_unlocked(), None)
        };

        let fs_name = name.chars().flat_map(|c| {
            c.to_lowercase()
        }).collect::<String>();

        // see module comment for why this is structured the way it is
        let path = match fs_name.len() {
            1 => path.join("1").join(&fs_name),
            2 => path.join("2").join(&fs_name),
            3 => path.join("3").join(&fs_name[..1]).join(&fs_name),
            _ => path.join(&fs_name[0..2])
                     .join(&fs_name[2..4])
                     .join(&fs_name),
        };
        match File::open(&path) {
            Ok(mut f) => {
                let mut contents = String::new();
                try!(f.read_to_string(&mut contents));
                let ret: CargoResult<Vec<(Summary, bool)>>;
                ret = contents.lines().filter(|l| l.trim().len() > 0)
                              .map(|l| self.parse_registry_package(l))
                              .collect();
                ret.chain_error(|| {
                    internal(format!("failed to parse registry's information \
                                      for: {}", name))
                })
            }
            Err(..) => Ok(Vec::new()),
        }
    }

    /// Parse a line from the registry's index file into a Summary for a
    /// package.
    ///
    /// The returned boolean is whether or not the summary has been yanked.
    fn parse_registry_package(&mut self, line: &str)
                              -> CargoResult<(Summary, bool)> {
        let RegistryPackage {
            name, vers, cksum, deps, features, yanked
        } = try!(json::decode::<RegistryPackage>(line));
        let pkgid = try!(PackageId::new(&name, &vers, &self.source_id));
        let deps: CargoResult<Vec<Dependency>> = deps.into_iter().map(|dep| {
            self.parse_registry_dependency(dep)
        }).collect();
        let deps = try!(deps);
        let summary = try!(Summary::new(pkgid, deps, features));
        let summary = summary.set_checksum(cksum.clone());
        self.hashes.insert((name, vers), cksum);
        Ok((summary, yanked.unwrap_or(false)))
    }

    /// Converts an encoded dependency in the registry to a cargo dependency
    fn parse_registry_dependency(&self, dep: RegistryDependency)
                                 -> CargoResult<Dependency> {
        let RegistryDependency {
            name, req, features, optional, default_features, target, kind
        } = dep;

        let dep = try!(DependencyInner::parse(&name, Some(&req), &self.source_id, None));
        let kind = match kind.as_ref().map(|s| &s[..]).unwrap_or("") {
            "dev" => Kind::Development,
            "build" => Kind::Build,
            _ => Kind::Normal,
        };

        let platform = match target {
            Some(target) => Some(try!(target.parse())),
            None => None,
        };

        // Unfortunately older versions of cargo and/or the registry ended up
        // publishing lots of entries where the features array contained the
        // empty feature, "", inside. This confuses the resolution process much
        // later on and these features aren't actually valid, so filter them all
        // out here.
        let features = features.into_iter().filter(|s| !s.is_empty()).collect();

        Ok(dep.set_optional(optional)
              .set_default_features(default_features)
              .set_features(features)
              .set_platform(platform)
              .set_kind(kind)
              .into_dependency())
    }
}

impl<'cfg> Registry for RegistryIndex<'cfg> {
    fn query(&mut self, dep: &Dependency) -> CargoResult<Vec<Summary>> {
        let mut summaries = {
            let summaries = try!(self.summaries(dep.name()));
            summaries.iter().filter(|&&(_, yanked)| {
                dep.source_id().precise().is_some() || !yanked
            }).map(|s| s.0.clone()).collect::<Vec<_>>()
        };

        // Handle `cargo update --precise` here. If specified, our own source
        // will have a precise version listed of the form `<pkg>=<req>` where
        // `<pkg>` is the name of a crate on this source and `<req>` is the
        // version requested (agument to `--precise`).
        summaries.retain(|s| {
            match self.source_id.precise() {
                Some(p) if p.starts_with(dep.name()) &&
                           p[dep.name().len()..].starts_with('=') => {
                    let vers = &p[dep.name().len() + 1..];
                    s.version().to_string() == vers
                }
                _ => true,
            }
        });
        summaries.query(dep)
    }

    fn supports_checksums(&self) -> bool {
        true
    }
}
