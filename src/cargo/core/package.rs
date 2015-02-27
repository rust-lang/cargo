use std::fmt::{self, Formatter};
use std::hash;
use std::slice;
use std::path::{Path, PathBuf};
use semver::Version;

use core::{
    Dependency,
    Manifest,
    PackageId,
    Registry,
    Target,
    Summary,
};
use core::dependency::SerializedDependency;
use util::{CargoResult, graph};
use rustc_serialize::{Encoder,Encodable};
use core::source::{SourceId, Source};

/// Informations about a package that is available somewhere in the file system.
///
/// A package is a `Cargo.toml` file, plus all the files that are part of it.
// TODO: Is manifest_path a relic?
#[derive(Clone, Debug)]
pub struct Package {
    // The package's manifest
    manifest: Manifest,
    // The root of the package
    manifest_path: PathBuf,
    // Where this package came from
    source_id: SourceId,
}

#[derive(RustcEncodable)]
struct SerializedPackage {
    name: String,
    version: String,
    dependencies: Vec<SerializedDependency>,
    targets: Vec<Target>,
    manifest_path: String,
}

impl Encodable for Package {
    fn encode<S: Encoder>(&self, s: &mut S) -> Result<(), S::Error> {
        let manifest = self.manifest();
        let summary = manifest.summary();
        let package_id = summary.package_id();

        SerializedPackage {
            name: package_id.name().to_string(),
            version: package_id.version().to_string(),
            dependencies: summary.dependencies().iter().map(|d| {
                SerializedDependency::from_dependency(d)
            }).collect(),
            targets: manifest.targets().to_vec(),
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
            manifest_path: manifest_path.to_path_buf(),
            source_id: source_id.clone(),
        }
    }

    pub fn dependencies(&self) -> &[Dependency] { self.manifest.dependencies() }
    pub fn manifest(&self) -> &Manifest { &self.manifest }
    pub fn manifest_path(&self) -> &Path { &self.manifest_path }
    pub fn name(&self) -> &str { self.package_id().name() }
    pub fn package_id(&self) -> &PackageId { self.manifest.package_id() }
    pub fn root(&self) -> &Path { self.manifest_path.parent().unwrap() }
    pub fn summary(&self) -> &Summary { self.manifest.summary() }
    pub fn target_dir(&self) -> &Path { self.manifest.target_dir() }
    pub fn targets(&self) -> &[Target] { self.manifest().targets() }
    pub fn version(&self) -> &Version { self.package_id().version() }

    pub fn absolute_target_dir(&self) -> PathBuf {
        self.root().join(self.target_dir())
    }

    pub fn has_custom_build(&self) -> bool {
        self.targets().iter().any(|t| t.profile().is_custom_build())
    }
}

impl fmt::Display for Package {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.summary().package_id())
    }
}

impl PartialEq for Package {
    fn eq(&self, other: &Package) -> bool {
        self.package_id() == other.package_id()
    }
}

impl Eq for Package {}

impl hash::Hash for Package {
    fn hash<H: hash::Hasher>(&self, into: &mut H) {
        self.package_id().hash(into)
    }
}

#[derive(PartialEq,Clone,Debug)]
pub struct PackageSet {
    packages: Vec<Package>,
}

impl PackageSet {
    pub fn new(packages: &[Package]) -> PackageSet {
        //assert!(packages.len() > 0,
        //        "PackageSet must be created with at least one package")
        PackageSet { packages: packages.to_vec() }
    }

    pub fn len(&self) -> usize {
        self.packages.len()
    }

    pub fn pop(&mut self) -> Package {
        self.packages.pop().expect("PackageSet.pop: empty set")
    }

    /// Get a package by name out of the set
    pub fn get(&self, name: &str) -> &Package {
        self.packages.iter().find(|pkg| name == pkg.name())
            .expect("PackageSet.get: empty set")
    }

    pub fn get_all(&self, names: &[&str]) -> Vec<&Package> {
        names.iter().map(|name| self.get(*name) ).collect()
    }

    pub fn packages(&self) -> &[Package] { &self.packages }

    // For now, assume that the package set contains only one package with a
    // given name
    pub fn sort(&self) -> Option<PackageSet> {
        let mut graph = graph::Graph::new();

        for pkg in self.packages.iter() {
            let deps: Vec<&str> = pkg.dependencies().iter()
                .map(|dep| dep.name())
                .collect();

            graph.add(pkg.name(), &deps);
        }

        let pkgs = match graph.sort() {
            Some(pkgs) => pkgs,
            None => return None,
        };
        let pkgs = pkgs.iter().map(|name| {
            self.get(*name).clone()
        }).collect();

        Some(PackageSet {
            packages: pkgs
        })
    }

    pub fn iter(&self) -> slice::Iter<Package> {
        self.packages.iter()
    }
}

impl Registry for PackageSet {
    fn query(&mut self, name: &Dependency) -> CargoResult<Vec<Summary>> {
        Ok(self.packages.iter()
            .filter(|pkg| name.name() == pkg.name())
            .map(|pkg| pkg.summary().clone())
            .collect())
    }
}
