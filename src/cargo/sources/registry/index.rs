use std::collections::HashMap;
use std::path::Path;
use std::str;

use serde_json;

use core::dependency::{Dependency, DependencyInner, Kind};
use core::{SourceId, Summary, PackageId, Registry};
use sources::registry::{RegistryPackage, RegistryDependency, INDEX_LOCK};
use sources::registry::RegistryData;
use util::{CargoError, CargoResult, internal, Filesystem, Config};

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
               locked: bool)
               -> RegistryIndex<'cfg> {
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
    pub fn hash(&mut self,
                pkg: &PackageId,
                load: &mut RegistryData)
                -> CargoResult<String> {
        let key = (pkg.name().to_string(), pkg.version().to_string());
        if let Some(s) = self.hashes.get(&key) {
            return Ok(s.clone())
        }
        // Ok, we're missing the key, so parse the index file to load it.
        self.summaries(pkg.name(), load)?;
        self.hashes.get(&key).ok_or_else(|| {
            internal(format!("no hash listed for {}", pkg))
        }).map(|s| s.clone())
    }

    /// Parse the on-disk metadata for the package provided
    ///
    /// Returns a list of pairs of (summary, yanked) for the package name
    /// specified.
    pub fn summaries(&mut self,
                     name: &str,
                     load: &mut RegistryData)
                     -> CargoResult<&Vec<(Summary, bool)>> {
        if self.cache.contains_key(name) {
            return Ok(&self.cache[name]);
        }
        let summaries = self.load_summaries(name, load)?;
        let summaries = summaries.into_iter().filter(|summary| {
            summary.0.package_id().name() == name
        }).collect();
        self.cache.insert(name.to_string(), summaries);
        Ok(&self.cache[name])
    }

    fn load_summaries(&mut self,
                      name: &str,
                      load: &mut RegistryData)
                      -> CargoResult<Vec<(Summary, bool)>> {
        let (root, _lock) = if self.locked {
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
            1 => format!("1/{}", fs_name),
            2 => format!("2/{}", fs_name),
            3 => format!("3/{}/{}", &fs_name[..1], fs_name),
            _ => format!("{}/{}/{}", &fs_name[0..2], &fs_name[2..4], fs_name),
            // 1 => Path::new("1").join(fs_name),
            // 2 => Path::new("2").join(fs_name),
            // 3 => Path::new("3").join(&fs_name[..1]).join(fs_name),
            // _ => Path::new(&fs_name[0..2]).join(&fs_name[2..4]).join(fs_name),
        };
        match load.load(&root, Path::new(&path)) {
            Ok(contents) => {
                let contents = str::from_utf8(&contents).map_err(|_| {
                    CargoError::from("registry index file was not valid utf-8")
                })?;
                let lines = contents.lines()
                                    .map(|s| s.trim())
                                    .filter(|l| !l.is_empty());

                // Attempt forwards-compatibility on the index by ignoring
                // everything that we ourselves don't understand, that should
                // allow future cargo implementations to break the
                // interpretation of each line here and older cargo will simply
                // ignore the new lines.
                Ok(lines.filter_map(|line| {
                    self.parse_registry_package(line).ok()
                }).collect())
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
        } = serde_json::from_str::<RegistryPackage>(line)?;
        let pkgid = PackageId::new(&name, &vers, &self.source_id)?;
        let deps: CargoResult<Vec<Dependency>> = deps.into_iter().map(|dep| {
            self.parse_registry_dependency(dep)
        }).collect();
        let deps = deps?;
        let summary = Summary::new(pkgid, deps, features)?;
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

        let mut dep = DependencyInner::parse(&name, Some(&req), &self.source_id, None)?;
        let kind = match kind.as_ref().map(|s| &s[..]).unwrap_or("") {
            "dev" => Kind::Development,
            "build" => Kind::Build,
            _ => Kind::Normal,
        };

        let platform = match target {
            Some(target) => Some(target.parse()?),
            None => None,
        };

        // Unfortunately older versions of cargo and/or the registry ended up
        // publishing lots of entries where the features array contained the
        // empty feature, "", inside. This confuses the resolution process much
        // later on and these features aren't actually valid, so filter them all
        // out here.
        let features = features.into_iter().filter(|s| !s.is_empty()).collect();

        dep.set_optional(optional)
           .set_default_features(default_features)
           .set_features(features)
           .set_platform(platform)
           .set_kind(kind);
        Ok(dep.into_dependency())
    }

    pub fn query(&mut self,
                 dep: &Dependency,
                 load: &mut RegistryData)
                 -> CargoResult<Vec<Summary>> {
        let mut summaries = {
            let summaries = self.summaries(dep.name(), load)?;
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
}
