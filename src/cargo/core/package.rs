use std::slice;
use std::fmt;
use std::fmt::{Show,Formatter};
use std::path::Path;
use core::{
    Dependency,
    Manifest,
    Registry,
    Target,
    Summary
};
use core::dependency::SerializedDependency;
use util::graph;
use serialize::{Encoder,Encodable};

#[deriving(Clone,Eq)]
pub struct Package {
    // The package's manifest
    manifest: Manifest,
    // The root of the package
    root: Path,
}

#[deriving(Encodable)]
struct SerializedPackage {
    name: ~str,
    version: ~str,
    dependencies: Vec<SerializedDependency>,
    authors: Vec<~str>,
    targets: Vec<Target>,
    root: ~str
}

impl<E, S: Encoder<E>> Encodable<S, E> for Package {
    fn encode(&self, s: &mut S) -> Result<(), E> {
        let manifest = self.get_manifest();
        let summary = manifest.get_summary();
        let name_ver = summary.get_name_ver();

        SerializedPackage {
            name: name_ver.get_name().to_owned(),
            version: name_ver.get_version().to_str(),
            dependencies: summary.get_dependencies().iter().map(|d| SerializedDependency::from_dependency(d)).collect(),
            authors: Vec::from_slice(manifest.get_authors()),
            targets: Vec::from_slice(manifest.get_targets()),
            root: self.root.as_str().unwrap().to_owned()
        }.encode(s)
    }
}

impl Package {
    pub fn new(manifest: &Manifest, root: &Path) -> Package {
        Package {
            manifest: manifest.clone(),
            root: root.clone()
        }
    }

    pub fn to_dependency(&self) -> Dependency {
        Dependency::exact(self.manifest.get_name(), self.manifest.get_version())
    }

    pub fn get_manifest<'a>(&'a self) -> &'a Manifest {
        &self.manifest
    }

    pub fn get_summary<'a>(&'a self) -> &'a Summary {
        self.manifest.get_summary()
    }

    pub fn get_name<'a>(&'a self) -> &'a str {
        self.get_manifest().get_name()
    }

    pub fn get_dependencies<'a>(&'a self) -> &'a [Dependency] {
        self.get_manifest().get_dependencies()
    }

    pub fn get_targets<'a>(&'a self) -> &'a [Target] {
        self.get_manifest().get_targets()
    }

    pub fn get_root<'a>(&'a self) -> &'a Path {
        &self.root
    }

    pub fn get_target_dir<'a>(&'a self) -> &'a Path {
        self.manifest.get_target_dir()
    }

    pub fn get_absolute_target_dir(&self) -> Path {
        self.get_root().join(self.get_target_dir())
    }
}

impl Show for Package {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f.buf, "{}", self.get_summary().get_name_ver())
    }
}

#[deriving(Eq,Clone,Show)]
pub struct PackageSet {
    packages: Vec<Package>
}

impl PackageSet {
    pub fn new(packages: &[Package]) -> PackageSet {
        assert!(packages.len() > 0, "PackageSet must be created with at least one package")
        PackageSet { packages: Vec::from_slice(packages) }
    }

    pub fn len(&self) -> uint {
        self.packages.len()
    }

    pub fn pop(&mut self) -> Package {
        self.packages.pop().unwrap()
    }

    /**
     * Get a package by name out of the set
     */
    pub fn get<'a>(&'a self, name: &str) -> &'a Package {
        self.packages.iter().find(|pkg| name == pkg.get_name()).unwrap()
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

        let pkgs = some!(graph.sort()).iter().map(|name| self.get(*name).clone()).collect();

        Some(PackageSet {
            packages: pkgs
        })
    }

    pub fn iter<'a>(&'a self) -> slice::Items<'a, Package> {
        self.packages.iter()
    }
}

impl Registry for PackageSet {
  fn query<'a>(&'a self, name: &str) -> Vec<&'a Summary> {
    self.packages.iter()
      .filter(|pkg| name == pkg.get_name())
      .map(|pkg| pkg.get_summary())
      .collect()
  }
}
