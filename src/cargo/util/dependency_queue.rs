//! A graph-like structure used to represent a set of dependencies and in what
//! order they should be built.
//!
//! This structure is used to store the dependency graph and dynamically update
//! it to figure out when a dependency should be built.
//!
//! Dependencies in this queue are represented as a (node, edge) pair. This is
//! used to model nodes which produce multiple outputs at different times but
//! some nodes may only require one of the outputs and can start before the
//! whole node is finished.

use std::collections::{HashMap, HashSet};
use std::hash::Hash;

#[derive(Debug)]
pub struct DependencyQueue<N: Hash + Eq, E: Hash + Eq, V> {
    /// A list of all known keys to build.
    ///
    /// The value of the hash map is list of dependencies which still need to be
    /// built before the package can be built. Note that the set is dynamically
    /// updated as more dependencies are built.
    dep_map: HashMap<N, (HashSet<(N, E)>, V)>,

    /// A reverse mapping of a package to all packages that depend on that
    /// package.
    ///
    /// This map is statically known and does not get updated throughout the
    /// lifecycle of the DependencyQueue.
    ///
    /// This is sort of like a `HashMap<(N, E), HashSet<N>>` map, but more
    /// easily indexable with just an `N`
    reverse_dep_map: HashMap<N, HashMap<E, HashSet<N>>>,

    /// Topological depth of each key
    depth: HashMap<N, usize>,
}

impl<N: Hash + Eq, E: Hash + Eq, V> Default for DependencyQueue<N, E, V> {
    fn default() -> DependencyQueue<N, E, V> {
        DependencyQueue::new()
    }
}

impl<N: Hash + Eq, E: Hash + Eq, V> DependencyQueue<N, E, V> {
    /// Creates a new dependency queue with 0 packages.
    pub fn new() -> DependencyQueue<N, E, V> {
        DependencyQueue {
            dep_map: HashMap::new(),
            reverse_dep_map: HashMap::new(),
            depth: HashMap::new(),
        }
    }
}

impl<N: Hash + Eq + Clone, E: Eq + Hash + Clone, V> DependencyQueue<N, E, V> {
    /// Adds a new ndoe and its dependencies to this queue.
    ///
    /// The `key` specified is a new node in the dependency graph, and the node
    /// depend on all the dependencies iterated by `dependencies`. Each
    /// dependency is a node/edge pair, where edges can be thought of as
    /// productions from nodes (aka if it's just `()` it's just waiting for the
    /// node to finish).
    ///
    /// An optional `value` can also be associated with `key` which is reclaimed
    /// when the node is ready to go.
    pub fn queue(&mut self, key: N, value: V, dependencies: impl IntoIterator<Item = (N, E)>) {
        assert!(!self.dep_map.contains_key(&key));

        let mut my_dependencies = HashSet::new();
        for (dep, edge) in dependencies {
            my_dependencies.insert((dep.clone(), edge.clone()));
            self.reverse_dep_map
                .entry(dep)
                .or_insert_with(HashMap::new)
                .entry(edge)
                .or_insert_with(HashSet::new)
                .insert(key.clone());
        }
        self.dep_map.insert(key, (my_dependencies, value));
    }

    /// All nodes have been added, calculate some internal metadata and prepare
    /// for `dequeue`.
    pub fn queue_finished(&mut self) {
        for key in self.dep_map.keys() {
            depth(key, &self.reverse_dep_map, &mut self.depth);
        }

        fn depth<N: Hash + Eq + Clone, E: Hash + Eq + Clone>(
            key: &N,
            map: &HashMap<N, HashMap<E, HashSet<N>>>,
            results: &mut HashMap<N, usize>,
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
                .flat_map(|it| it.values())
                .flat_map(|set| set)
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
    pub fn dequeue(&mut self) -> Option<(N, V)> {
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
            .filter(|(_, (deps, _))| deps.is_empty())
            .map(|(key, _)| key.clone())
            .max_by_key(|k| self.depth[k]);
        let key = match next {
            Some(key) => key,
            None => return None,
        };
        let (_, data) = self.dep_map.remove(&key).unwrap();
        Some((key, data))
    }

    /// Returns `true` if there are remaining packages to be built.
    pub fn is_empty(&self) -> bool {
        self.dep_map.is_empty()
    }

    /// Returns the number of remaining packages to be built.
    pub fn len(&self) -> usize {
        self.dep_map.len()
    }

    /// Indicate that something has finished.
    ///
    /// Calling this function indicates that the `node` has produced `edge`. All
    /// remaining work items which only depend on this node/edge pair are now
    /// candidates to start their job.
    pub fn finish(&mut self, node: &N, edge: &E) {
        let reverse_deps = self.reverse_dep_map.get(node).and_then(|map| map.get(edge));
        let reverse_deps = match reverse_deps {
            Some(deps) => deps,
            None => return,
        };
        let key = (node.clone(), edge.clone());
        for dep in reverse_deps.iter() {
            assert!(self.dep_map.get_mut(dep).unwrap().0.remove(&key));
        }
    }
}

#[cfg(test)]
mod test {
    use super::DependencyQueue;

    #[test]
    fn deep_first() {
        let mut q = DependencyQueue::new();

        q.queue(1, (), vec![]);
        q.queue(2, (), vec![(1, ())]);
        q.queue(3, (), vec![]);
        q.queue(4, (), vec![(2, ()), (3, ())]);
        q.queue(5, (), vec![(4, ()), (3, ())]);
        q.queue_finished();

        assert_eq!(q.dequeue(), Some((1, ())));
        assert_eq!(q.dequeue(), Some((3, ())));
        assert_eq!(q.dequeue(), None);
        q.finish(&3, &());
        assert_eq!(q.dequeue(), None);
        q.finish(&1, &());
        assert_eq!(q.dequeue(), Some((2, ())));
        assert_eq!(q.dequeue(), None);
        q.finish(&2, &());
        assert_eq!(q.dequeue(), Some((4, ())));
        assert_eq!(q.dequeue(), None);
        q.finish(&4, &());
        assert_eq!(q.dequeue(), Some((5, ())));
    }
}
