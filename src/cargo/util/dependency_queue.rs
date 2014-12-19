//! A graph-like structure used to represent a set of dependencies and in what
//! order they should be built.
//!
//! This structure is used to store the dependency graph and dynamically update
//! it to figure out when a dependency should be built.

use std::collections::{HashMap, HashSet};
use std::collections::hash_map::{Occupied, Vacant};
use std::hash::Hash;

pub use self::Freshness::{Fresh, Dirty};

pub struct DependencyQueue<K, V> {
    /// A list of all known keys to build.
    ///
    /// The value of the hash map is list of dependencies which still need to be
    /// built before the package can be built. Note that the set is dynamically
    /// updated as more dependencies are built.
    dep_map: HashMap<K, (HashSet<K>, V)>,

    /// A reverse mapping of a package to all packages that depend on that
    /// package.
    ///
    /// This map is statically known and does not get updated throughout the
    /// lifecycle of the DependencyQueue.
    reverse_dep_map: HashMap<K, HashSet<K>>,

    /// A set of dirty packages.
    ///
    /// Packages may become dirty over time if their dependencies are rebuilt.
    dirty: HashSet<K>,

    /// The packages which are currently being built, waiting for a call to
    /// `finish`.
    pending: HashSet<K>,
}

/// Indication of the freshness of a package.
///
/// A fresh package does not necessarily need to be rebuilt (unless a dependency
/// was also rebuilt), and a dirty package must always be rebuilt.
#[deriving(PartialEq, Eq, Show, Copy)]
pub enum Freshness {
    Fresh,
    Dirty,
}

/// A trait for discovering the dependencies of a piece of data.
pub trait Dependency<C>: Hash + Eq + Clone {
    fn dependencies(&self, cx: &C) -> Vec<Self>;
}

impl Freshness {
    pub fn combine(&self, other: Freshness) -> Freshness {
        match *self { Fresh => other, Dirty => Dirty }
    }
}

impl<C, K: Dependency<C>, V> DependencyQueue<K, V> {
    /// Creates a new dependency queue with 0 packages.
    pub fn new() -> DependencyQueue<K, V> {
        DependencyQueue {
            dep_map: HashMap::new(),
            reverse_dep_map: HashMap::new(),
            dirty: HashSet::new(),
            pending: HashSet::new(),
        }
    }

    /// Adds a new package to this dependency queue.
    ///
    /// It is assumed that any dependencies of this package will eventually also
    /// be added to the dependency queue.
    pub fn enqueue(&mut self, cx: &C, fresh: Freshness, key: K, value: V) {
        // ignore self-deps
        if self.dep_map.contains_key(&key) { return }

        if fresh == Dirty {
            self.dirty.insert(key.clone());
        }

        let mut my_dependencies = HashSet::new();
        for dep in key.dependencies(cx).into_iter() {
            assert!(my_dependencies.insert(dep.clone()));
            let rev = match self.reverse_dep_map.entry(dep) {
                Occupied(entry) => entry.into_mut(),
                Vacant(entry) => entry.set(HashSet::new()),
            };
            assert!(rev.insert(key.clone()));
        }
        assert!(self.dep_map.insert(key, (my_dependencies, value)).is_none());
    }

    /// Dequeues a package that is ready to be built.
    ///
    /// A package is ready to be built when it has 0 un-built dependencies. If
    /// `None` is returned then no packages are ready to be built.
    pub fn dequeue(&mut self) -> Option<(Freshness, K, V)> {
        let key = match self.dep_map.iter()
                                    .find(|&(_, &(ref deps, _))| deps.len() == 0)
                                    .map(|(key, _)| key.clone()) {
            Some(key) => key,
            None => return None
        };
        let (_, data) = self.dep_map.remove(&key).unwrap();
        let fresh = if self.dirty.contains(&key) {Dirty} else {Fresh};
        self.pending.insert(key.clone());
        Some((fresh, key, data))
    }

    /// Returns the number of remaining packages to be built.
    pub fn len(&self) -> uint {
        self.dep_map.len() + self.pending.len()
    }

    /// Indicate that a package has been built.
    ///
    /// This function will update the dependency queue with this information,
    /// possibly allowing the next invocation of `dequeue` to return a package.
    pub fn finish(&mut self, key: &K, fresh: Freshness) {
        assert!(self.pending.remove(key));
        let reverse_deps = match self.reverse_dep_map.get(key) {
            Some(deps) => deps,
            None => return,
        };
        for dep in reverse_deps.iter() {
            if fresh == Dirty {
                self.dirty.insert(dep.clone());
            }
            assert!(self.dep_map[*dep].0.remove(key));
        }
    }
}
