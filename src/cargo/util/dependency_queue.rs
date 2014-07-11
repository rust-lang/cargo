//! A graph-like structure used to represent a set of dependencies and in what
//! order they should be built.
//!
//! This structure is used to store the dependency graph and dynamically update
//! it to figure out when a dependency should be built.

use std::collections::{HashMap, HashSet};

use core::{Package, PackageId};

pub struct DependencyQueue<'a, T> {
    /// A list of all known packages to build.
    ///
    /// The value of the hash map is list of dependencies which still need to be
    /// built before the package can be built. Note that the set is dynamically
    /// updated as more dependencies are built.
    pkgs: HashMap<&'a PackageId, (HashSet<&'a PackageId>, T)>,

    /// A reverse mapping of a package to all packages that depend on that
    /// package.
    ///
    /// This map is statically known and does not get updated throughout the
    /// lifecycle of the DependencyQueue.
    reverse_dep_map: HashMap<&'a PackageId, HashSet<&'a PackageId>>,

    /// A set of dirty packages.
    ///
    /// Packages may become dirty over time if their dependencies are rebuilt.
    dirty: HashSet<&'a PackageId>,

    /// The packages which are currently being built, waiting for a call to
    /// `finish`.
    pending: HashSet<&'a PackageId>,
}

/// Indication of the freshness of a package.
///
/// A fresh package does not necessarily need to be rebuilt (unless a dependency
/// was also rebuilt), and a dirty package must always be rebuilt.
#[deriving(PartialEq)]
pub enum Freshness {
    Fresh,
    Dirty,
}

impl<'a, T> DependencyQueue<'a, T> {
    /// Creates a new dependency queue with 0 packages.
    pub fn new() -> DependencyQueue<'a, T> {
        DependencyQueue {
            pkgs: HashMap::new(),
            reverse_dep_map: HashMap::new(),
            dirty: HashSet::new(),
            pending: HashSet::new(),
        }
    }

    /// Registers a package with this queue.
    ///
    /// Only registered packages will be returned from dequeue().
    pub fn register(&mut self, pkg: &'a Package) {
        self.reverse_dep_map.insert(pkg.get_package_id(), HashSet::new());
    }

    /// Adds a new package to this dependency queue.
    ///
    /// It is assumed that any dependencies of this package will eventually also
    /// be added to the dependency queue.
    pub fn enqueue(&mut self, pkg: &'a Package, deps: Vec<&'a PackageId>,
                   fresh: Freshness, data: T) {
        // ignore self-deps
        if self.pkgs.contains_key(&pkg.get_package_id()) { return }

        if fresh == Dirty {
            self.dirty.insert(pkg.get_package_id());
        }

        let mut my_dependencies = HashSet::new();
        for &dep in deps.iter() {
            if dep == pkg.get_package_id() { continue }
            // skip deps which were filtered out as part of resolve
            if !self.reverse_dep_map.find(&dep).is_some() {
                continue
            }

            assert!(my_dependencies.insert(dep));
            let rev = self.reverse_dep_map.find_or_insert(dep, HashSet::new());
            assert!(rev.insert(pkg.get_package_id()));
        }
        assert!(self.pkgs.insert(pkg.get_package_id(),
                                 (my_dependencies, data)));
    }

    /// Dequeues a package that is ready to be built.
    ///
    /// A package is ready to be built when it has 0 un-built dependencies. If
    /// `None` is returned then no packages are ready to be built.
    pub fn dequeue(&mut self) -> Option<(&'a PackageId, Freshness, T)> {
        let pkg = match self.pkgs.iter()
                                 .find(|&(_, &(ref deps, _))| deps.len() == 0)
                                 .map(|(name, _)| *name) {
            Some(pkg) => pkg,
            None => return None
        };
        let (_, data) = self.pkgs.pop(&pkg).unwrap();
        self.pending.insert(pkg);
        let fresh = if self.dirty.contains(&pkg) {Dirty} else {Fresh};
        Some((pkg, fresh, data))
    }

    /// Returns the number of remaining packages to be built.
    pub fn len(&self) -> uint {
        self.pkgs.len() + self.pending.len()
    }

    /// Indicate that a package has been built.
    ///
    /// This function will update the dependency queue with this information,
    /// possibly allowing the next invocation of `dequeue` to return a package.
    ///
    /// The `fresh` parameter is used to indicate whether the package was
    /// actually rebuilt or not. If no action was taken, then the parameter
    /// should be `Fresh`. If a package was rebuilt, `Dirty` should be
    /// specified, and the dirtiness will be propagated properly to all
    /// dependencies.
    pub fn finish(&mut self, pkg: &'a PackageId, fresh: Freshness) {
        assert!(self.pending.remove(&pkg));
        let reverse_deps = match self.reverse_dep_map.find(&pkg) {
            Some(deps) => deps,
            None => return,
        };
        for dep in reverse_deps.iter() {
            if fresh == Dirty {
                self.dirty.insert(dep.clone());
            }
            assert!(self.pkgs.get_mut(dep).mut0().remove(&pkg));
        }
    }
}
