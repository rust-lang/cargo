use std::slice;
use std::path::Path;
use core::{
    Dependency,
    Manifest,
    Registry,
    Target,
    Summary
};
use util::graph;

#[deriving(Clone)]
pub struct Package {
    // The package's manifest
    manifest: Manifest,
    // The root of the package
    root: Path,
}

impl Package {
    pub fn new(manifest: &Manifest, root: &Path) -> Package {
        Package {
            manifest: manifest.clone(),
            root: root.clone()
        }
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

pub struct PackageSet {
    packages: ~[Package]
}

impl PackageSet {
    pub fn new(packages: &[Package]) -> PackageSet {
        PackageSet { packages: packages.to_owned() }
    }

    /**
     * Get a package by name out of the set
     */
    pub fn get<'a>(&'a self, name: &str) -> &'a Package {
        let opts = self.query(name);
        assert!(opts.len() == 1, "expected exactly one package named `{}`", name);
        *opts.get(0)
    }

    pub fn get_all<'a>(&'a self, names: &[&str]) -> ~[&'a Package] {
        names.iter().map(|name| self.get(*name) ).collect()
    }

    // For now, assume that the package set contains only one package with a
    // given name
    pub fn sort(&self) -> Option<PackageSet> {
        let mut graph = graph::Graph::new();

        for pkg in self.packages.iter() {
            let deps: ~[&str] = pkg.get_dependencies().iter()
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
  fn query<'a>(&'a self, name: &str) -> Vec<&'a Package> {
    self.packages.iter()
      .filter(|pkg| name == pkg.get_name())
      .collect()
  }
}
