use std::cmp;
use std::fmt::{Show,Formatter};
use std::fmt;
use std::slice;
use semver::Version;

use core::{
    Dependency,
    Manifest,
    PackageId,
    Registry,
    Target,
    Summary
};
use core::dependency::SerializedDependency;
use util::{CargoResult, graph, Config};
use serialize::{Encoder,Encodable};
use core::source::{SourceId, SourceSet, Source};

// TODO: Is manifest_path a relic?
#[deriving(Clone)]
pub struct Package {
    // The package's manifest
    manifest: Manifest,
    // The root of the package
    manifest_path: Path,
    // Where this package came from
    source_id: SourceId,
}

#[deriving(Encodable)]
struct SerializedPackage {
    name: String,
    version: String,
    dependencies: Vec<SerializedDependency>,
    authors: Vec<String>,
    targets: Vec<Target>,
    manifest_path: String,
}

impl<E, S: Encoder<E>> Encodable<S, E> for Package {
    fn encode(&self, s: &mut S) -> Result<(), E> {
        let manifest = self.get_manifest();
        let summary = manifest.get_summary();
        let package_id = summary.get_package_id();

        SerializedPackage {
            name: package_id.get_name().to_string(),
            version: package_id.get_version().to_string(),
            dependencies: summary.get_dependencies().iter().map(|d| {
                SerializedDependency::from_dependency(d)
            }).collect(),
            authors: Vec::from_slice(manifest.get_authors()),
            targets: Vec::from_slice(manifest.get_targets()),
            manifest_path: self.manifest_path.display().to_string()
        }.encode(s)
    }
}

impl Package {
    pub fn new(manifest: Manifest,
               manifest_path: &Path,
               source_id: &SourceId) -> Package {
        Package {
            manifest: manifest,
            manifest_path: manifest_path.clone(),
            source_id: source_id.clone(),
        }
    }

    pub fn get_manifest<'a>(&'a self) -> &'a Manifest {
        &self.manifest
    }

    pub fn get_summary<'a>(&'a self) -> &'a Summary {
        self.manifest.get_summary()
    }

    pub fn get_package_id<'a>(&'a self) -> &'a PackageId {
        self.manifest.get_package_id()
    }

    pub fn get_name<'a>(&'a self) -> &'a str {
        self.get_package_id().get_name()
    }

    pub fn get_version<'a>(&'a self) -> &'a Version {
        self.get_package_id().get_version()
    }

    pub fn get_dependencies<'a>(&'a self) -> &'a [Dependency] {
        self.get_manifest().get_dependencies()
    }

    pub fn get_targets<'a>(&'a self) -> &'a [Target] {
        self.get_manifest().get_targets()
    }

    pub fn get_manifest_path<'a>(&'a self) -> &'a Path {
        &self.manifest_path
    }

    pub fn get_root<'a>(&'a self) -> Path {
        self.manifest_path.dir_path()
    }

    pub fn get_target_dir<'a>(&'a self) -> &'a Path {
        self.manifest.get_target_dir()
    }

    pub fn get_absolute_target_dir(&self) -> Path {
        self.get_root().join(self.get_target_dir())
    }

    pub fn get_source_ids(&self) -> Vec<SourceId> {
        let mut ret = vec!(self.source_id.clone());
        ret.push_all(self.manifest.get_source_ids());
        ret
    }

    pub fn get_fingerprint(&self, config: &mut Config) -> CargoResult<String> {
        let mut sources = self.get_source_ids();
        // Sort the sources just to make sure we have a consistent fingerprint.
        sources.sort_by(|a, b| {
            cmp::lexical_ordering(a.kind.cmp(&b.kind),
                                  a.location.to_string().cmp(&b.location.to_string()))
        });
        let sources = sources.iter().map(|source_id| {
            source_id.load(config)
        }).collect::<Vec<_>>();
        SourceSet::new(sources).fingerprint(self)
    }
}

impl Show for Package {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.get_summary().get_package_id())
    }
}

impl PartialEq for Package {
    fn eq(&self, other: &Package) -> bool {
        self.get_package_id() == other.get_package_id()
    }
}

#[deriving(PartialEq,Clone,Show)]
pub struct PackageSet {
    packages: Vec<Package>,
}

impl PackageSet {
    pub fn new(packages: &[Package]) -> PackageSet {
        //assert!(packages.len() > 0,
        //        "PackageSet must be created with at least one package")
        PackageSet { packages: Vec::from_slice(packages) }
    }

    pub fn len(&self) -> uint {
        self.packages.len()
    }

    pub fn pop(&mut self) -> Package {
        self.packages.pop().expect("PackageSet.pop: empty set")
    }

    /// Get a package by name out of the set
    pub fn get<'a>(&'a self, name: &str) -> &'a Package {
        self.packages.iter().find(|pkg| name == pkg.get_name())
            .expect("PackageSet.get: empty set")
    }

    pub fn get_all<'a>(&'a self, names: &[&str]) -> Vec<&'a Package> {
        names.iter().map(|name| self.get(*name) ).collect()
    }

    pub fn get_packages<'a>(&'a self) -> &'a [Package] {
        self.packages.as_slice()
    }

    // For now, assume that the package set contains only one package with a
    // given name
    pub fn sort(&self) -> Option<PackageSet> {
        let mut graph = graph::Graph::new();

        for pkg in self.packages.iter() {
            let deps: Vec<&str> = pkg.get_dependencies().iter()
                .map(|dep| dep.get_name())
                .collect();

            graph.add(pkg.get_name(), deps.as_slice());
        }

        let pkgs = some!(graph.sort()).iter().map(|name| {
            self.get(*name).clone()
        }).collect();

        Some(PackageSet {
            packages: pkgs
        })
    }

    pub fn iter<'a>(&'a self) -> slice::Items<'a, Package> {
        self.packages.iter()
    }
}

impl Registry for PackageSet {
    fn query(&mut self, name: &Dependency) -> CargoResult<Vec<Summary>> {
        Ok(self.packages.iter()
            .filter(|pkg| name.get_name() == pkg.get_name())
            .map(|pkg| pkg.get_summary().clone())
            .collect())
    }
}
