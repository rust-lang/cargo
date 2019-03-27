//! A graph-like structure used to represent a set of dependencies and in what
//! order they should be built.
//!
//! This structure is used to store the dependency graph and dynamically update
//! it to figure out when a dependency should be built.

use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

pub use self::Freshness::{Dirty, Fresh};

#[derive(Debug)]
pub struct DependencyQueue<K: Eq + Hash, V> {
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

    /// Topological depth of each key
    depth: HashMap<K, usize>,
}

/// Indication of the freshness of a package.
///
/// A fresh package does not necessarily need to be rebuilt (unless a dependency
/// was also rebuilt), and a dirty package must always be rebuilt.
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum Freshness {
    Fresh,
    Dirty,
}

impl Freshness {
    pub fn combine(self, other: Freshness) -> Freshness {
        match self {
            Fresh => other,
            Dirty => Dirty,
        }
    }
}

impl<K: Hash + Eq + Clone, V> Default for DependencyQueue<K, V> {
    fn default() -> DependencyQueue<K, V> {
        DependencyQueue::new()
    }
}

impl<K: Hash + Eq + Clone, V> DependencyQueue<K, V> {
    /// Creates a new dependency queue with 0 packages.
    pub fn new() -> DependencyQueue<K, V> {
        DependencyQueue {
            dep_map: HashMap::new(),
            reverse_dep_map: HashMap::new(),
            dirty: HashSet::new(),
            pending: HashSet::new(),
            depth: HashMap::new(),
        }
    }

    /// Adds a new package to this dependency queue.
    ///
    /// It is assumed that any dependencies of this package will eventually also
    /// be added to the dependency queue.
    pub fn queue(&mut self, fresh: Freshness, key: &K, value: V, dependencies: &[K]) -> &mut V {
        let slot = match self.dep_map.entry(key.clone()) {
            Occupied(v) => return &mut v.into_mut().1,
            Vacant(v) => v,
        };

        if fresh == Dirty {
            self.dirty.insert(key.clone());
        }

        let mut my_dependencies = HashSet::new();
        for dep in dependencies {
            my_dependencies.insert(dep.clone());
            let rev = self
                .reverse_dep_map
                .entry(dep.clone())
                .or_insert_with(HashSet::new);
            rev.insert(key.clone());
        }
        &mut slot.insert((my_dependencies, value)).1
    }

    /// All nodes have been added, calculate some internal metadata and prepare
    /// for `dequeue`.
    pub fn queue_finished(&mut self) {
        for key in self.dep_map.keys() {
            depth(key, &self.reverse_dep_map, &mut self.depth);
        }

        fn depth<K: Hash + Eq + Clone>(
            key: &K,
            map: &HashMap<K, HashSet<K>>,
            results: &mut HashMap<K, usize>,
        ) -> usize {
            const IN_PROGRESS: usize = !0;

            if let Some(&depth) = results.get(key) {
                assert_ne!(depth, IN_PROGRESS, "cycle in DependencyQueue");
                return depth;
            }

            results.insert(key.clone(), IN_PROGRESS);

            let depth = 1 + map
                .get(key)
                .into_iter()
                .flat_map(|it| it)
                .map(|dep| depth(dep, map, results))
                .max()
                .unwrap_or(0);

            *results.get_mut(key).unwrap() = depth;

            depth
        }
    }

    /// Dequeues a package that is ready to be built.
    ///
    /// A package is ready to be built when it has 0 un-built dependencies. If
    /// `None` is returned then no packages are ready to be built.
    pub fn dequeue(&mut self) -> Option<(Freshness, K, V)> {
        // Look at all our crates and find everything that's ready to build (no
        // deps). After we've got that candidate set select the one which has
        // the maximum depth in the dependency graph. This way we should
        // hopefully keep CPUs hottest the longest by ensuring that long
        // dependency chains are scheduled early on in the build process and the
        // leafs higher in the tree can fill in the cracks later.
        //
        // TODO: it'd be best here to throw in a heuristic of crate size as
        //       well. For example how long did this crate historically take to
        //       compile? How large is its source code? etc.
        let next = self
            .dep_map
            .iter()
            .filter(|&(_, &(ref deps, _))| deps.is_empty())
            .map(|(key, _)| key.clone())
            .max_by_key(|k| self.depth[k]);
        let key = match next {
            Some(key) => key,
            None => return None,
        };
        let (_, data) = self.dep_map.remove(&key).unwrap();
        let fresh = if self.dirty.contains(&key) {
            Dirty
        } else {
            Fresh
        };
        self.pending.insert(key.clone());
        Some((fresh, key, data))
    }

    /// Returns `true` if there are remaining packages to be built.
    pub fn is_empty(&self) -> bool {
        self.dep_map.is_empty() && self.pending.is_empty()
    }

    /// Returns the number of remaining packages to be built.
    pub fn len(&self) -> usize {
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
            assert!(self.dep_map.get_mut(dep).unwrap().0.remove(key));
        }
    }
}

#[cfg(test)]
mod test {
    use super::{DependencyQueue, Freshness};

    #[test]
    fn deep_first() {
        let mut q = DependencyQueue::new();

        q.queue(Freshness::Fresh, &1, (), &[]);
        q.queue(Freshness::Fresh, &2, (), &[1]);
        q.queue(Freshness::Fresh, &3, (), &[]);
        q.queue(Freshness::Fresh, &4, (), &[2, 3]);
        q.queue(Freshness::Fresh, &5, (), &[4, 3]);
        q.queue_finished();

        assert_eq!(q.dequeue(), Some((Freshness::Fresh, 1, ())));
        assert_eq!(q.dequeue(), Some((Freshness::Fresh, 3, ())));
        assert_eq!(q.dequeue(), None);
        q.finish(&3, Freshness::Fresh);
        assert_eq!(q.dequeue(), None);
        q.finish(&1, Freshness::Fresh);
        assert_eq!(q.dequeue(), Some((Freshness::Fresh, 2, ())));
        assert_eq!(q.dequeue(), None);
        q.finish(&2, Freshness::Fresh);
        assert_eq!(q.dequeue(), Some((Freshness::Fresh, 4, ())));
        assert_eq!(q.dequeue(), None);
        q.finish(&4, Freshness::Fresh);
        assert_eq!(q.dequeue(), Some((Freshness::Fresh, 5, ())));
    }
}
