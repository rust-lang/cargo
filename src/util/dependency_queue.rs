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

use crate::util::data_structures::{HashMap, HashSet};
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::hash::Hash;

#[derive(Debug)]
struct Ready<N> {
    key: N,
    priority: usize,
    order: usize,
}

impl<N> PartialEq for Ready<N> {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority && self.order == other.order
    }
}

impl<N> Eq for Ready<N> {}

impl<N> PartialOrd for Ready<N> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<N> Ord for Ready<N> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority
            .cmp(&other.priority)
            .then_with(|| self.order.cmp(&other.order))
    }
}

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
    /// lifecycle of the `DependencyQueue`.
    ///
    /// This is sort of like a `HashMap<(N, E), HashSet<N>>` map, but more
    /// easily indexable with just an `N`
    reverse_dep_map: HashMap<N, HashMap<E, HashSet<N>>>,

    /// The relative priority and original map order of this package.
    priority: HashMap<N, (usize, usize)>,

    /// An expected cost for building this package. Used to determine priority.
    cost: HashMap<N, usize>,

    /// Nodes with no remaining dependencies.
    ready: BinaryHeap<Ready<N>>,
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
            dep_map: HashMap::default(),
            reverse_dep_map: HashMap::default(),
            priority: HashMap::default(),
            cost: HashMap::default(),
            ready: BinaryHeap::new(),
        }
    }
}

impl<N: Hash + Eq + Clone, E: Eq + Hash + Clone, V> DependencyQueue<N, E, V> {
    /// Adds a new node and its dependencies to this queue.
    ///
    /// The `key` specified is a new node in the dependency graph, and the node
    /// depend on all the dependencies iterated by `dependencies`. Each
    /// dependency is a node/edge pair, where edges can be thought of as
    /// productions from nodes (aka if it's just `()` it's just waiting for the
    /// node to finish).
    ///
    /// An optional `value` can also be associated with `key` which is reclaimed
    /// when the node is ready to go.
    ///
    /// The cost parameter can be used to hint at the relative cost of building
    /// this node. This implementation does not care about the units of this value, so
    /// the calling code is free to use whatever they'd like. In general, higher cost
    /// nodes are expected to take longer to build.
    pub fn queue(
        &mut self,
        key: N,
        value: V,
        dependencies: impl IntoIterator<Item = (N, E)>,
        cost: usize,
    ) {
        assert!(!self.dep_map.contains_key(&key));

        let mut my_dependencies = HashSet::default();
        for (dep, edge) in dependencies {
            my_dependencies.insert((dep.clone(), edge.clone()));
            self.reverse_dep_map
                .entry(dep)
                .or_insert_with(HashMap::default)
                .entry(edge)
                .or_insert_with(HashSet::default)
                .insert(key.clone());
        }
        self.dep_map.insert(key.clone(), (my_dependencies, value));
        self.cost.insert(key, cost);
    }

    /// All nodes have been added, calculate some internal metadata and prepare
    /// for `dequeue`.
    pub fn queue_finished(&mut self) {
        let mut out = HashMap::default();
        for key in self.dep_map.keys() {
            depth(key, &self.reverse_dep_map, &mut out);
        }

        self.priority = self
            .dep_map
            .keys()
            .enumerate()
            .map(|(order, n)| {
                let set = out.remove(n).unwrap();
                let total_cost = self.cost[n] + set.iter().map(|key| self.cost[key]).sum::<usize>();
                (n.clone(), (total_cost, order))
            })
            .collect();

        self.ready.clear();
        self.ready.extend(
            self.dep_map
                .iter()
                .filter(|(_, (deps, _))| deps.is_empty())
                .map(|(key, _)| {
                    let &(priority, order) = &self.priority[key];
                    Ready {
                        key: key.clone(),
                        priority,
                        order,
                    }
                }),
        );

        /// Creates a flattened reverse dependency list. For a given key, finds the
        /// set of nodes which depend on it, including transitively. This is different
        /// from `self.reverse_dep_map` because `self.reverse_dep_map` only maps one level
        /// of reverse dependencies.
        fn depth<'a, N: Hash + Eq + Clone, E: Hash + Eq + Clone>(
            key: &N,
            map: &HashMap<N, HashMap<E, HashSet<N>>>,
            results: &'a mut HashMap<N, HashSet<N>>,
        ) -> &'a HashSet<N> {
            if results.contains_key(key) {
                let depth = &results[key];
                assert!(!depth.is_empty(), "cycle in DependencyQueue");
                return depth;
            }
            results.insert(key.clone(), HashSet::default());

            let mut set = HashSet::default();
            set.insert(key.clone());

            for dep in map
                .get(key)
                .into_iter()
                .flat_map(|it| it.values())
                .flatten()
            {
                set.extend(depth(dep, map, results).iter().cloned())
            }

            let slot = results.get_mut(key).unwrap();
            *slot = set;
            &*slot
        }
    }

    /// Dequeues a package that is ready to be built.
    ///
    /// A package is ready to be built when it has 0 un-built dependencies. If
    /// `None` is returned then no packages are ready to be built.
    pub fn dequeue(&mut self) -> Option<(N, V, usize)> {
        let Ready { key, priority, .. } = self.ready.pop()?;
        let (_, data) = self.dep_map.remove(&key).unwrap();
        Some((key, data, priority))
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
    ///
    /// Returns the nodes that are now allowed to be dequeued as a result of
    /// finishing this node.
    pub fn finish(&mut self, node: &N, edge: &E) -> Vec<&N> {
        let reverse_deps = self.reverse_dep_map.get(node).and_then(|map| map.get(edge));
        let Some(reverse_deps) = reverse_deps else {
            return Vec::new();
        };
        let key = (node.clone(), edge.clone());
        let mut result = Vec::new();
        for dep in reverse_deps.iter() {
            let edges = &mut self.dep_map.get_mut(dep).unwrap().0;
            assert!(edges.remove(&key));
            if edges.is_empty() {
                result.push(dep);
            }
        }
        self.ready.reserve(result.len());
        for dep in &result {
            let &(priority, order) = &self.priority[*dep];
            self.ready.push(Ready {
                key: (**dep).clone(),
                priority,
                order,
            });
        }
        result
    }
}

#[cfg(test)]
mod test {
    use super::{DependencyQueue, HashSet};

    #[test]
    fn deep_first_equal_cost() {
        let mut q = DependencyQueue::new();

        q.queue(1, (), vec![], 1);
        q.queue(2, (), vec![(1, ())], 1);
        q.queue(3, (), vec![], 1);
        q.queue(4, (), vec![(2, ()), (3, ())], 1);
        q.queue(5, (), vec![(4, ()), (3, ())], 1);
        q.queue_finished();

        assert_eq!(q.dequeue(), Some((1, (), 5)));
        assert_eq!(q.dequeue(), Some((3, (), 4)));
        assert_eq!(q.dequeue(), None);
        q.finish(&3, &());
        assert_eq!(q.dequeue(), None);
        q.finish(&1, &());
        assert_eq!(q.dequeue(), Some((2, (), 4)));
        assert_eq!(q.dequeue(), None);
        q.finish(&2, &());
        assert_eq!(q.dequeue(), Some((4, (), 3)));
        assert_eq!(q.dequeue(), None);
        q.finish(&4, &());
        assert_eq!(q.dequeue(), Some((5, (), 2)));
    }

    #[test]
    fn sort_by_highest_cost() {
        let mut q = DependencyQueue::new();

        q.queue(1, (), vec![], 1);
        q.queue(2, (), vec![(1, ())], 1);
        q.queue(3, (), vec![], 4);
        q.queue(4, (), vec![(2, ()), (3, ())], 1);
        q.queue_finished();

        assert_eq!(q.dequeue(), Some((3, (), 9)));
        assert_eq!(q.dequeue(), Some((1, (), 4)));
        assert_eq!(q.dequeue(), None);
        q.finish(&3, &());
        assert_eq!(q.dequeue(), None);
        q.finish(&1, &());
        assert_eq!(q.dequeue(), Some((2, (), 3)));
        assert_eq!(q.dequeue(), None);
        q.finish(&2, &());
        assert_eq!(q.dequeue(), Some((4, (), 2)));
        assert_eq!(q.dequeue(), None);
        q.finish(&4, &());
        assert_eq!(q.dequeue(), None);
    }

    #[test]
    fn preserves_equal_priority_dispatch_order() {
        let mut q: DependencyQueue<i32, (), ()> = DependencyQueue::new();

        for node in 0..16 {
            q.queue(node, (), std::iter::empty::<(i32, ())>(), 1);
        }

        let mut expected = q.dep_map.keys().copied().collect::<Vec<_>>();
        expected.reverse();
        q.queue_finished();

        let actual =
            std::iter::from_fn(|| q.dequeue().map(|(node, (), _)| node)).collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }

    #[test]
    fn heap_matches_previous_scanner() {
        for seed in 0..16 {
            let mut heap = seeded_queue(seed);
            let mut scan = seeded_queue(seed);
            heap.queue_finished();
            scan.queue_finished();

            let mut active = Vec::new();
            let mut scheduled = HashSet::default();
            let mut step = 0;
            loop {
                let ready = heap
                    .dep_map
                    .values()
                    .filter(|(dependencies, _)| dependencies.is_empty())
                    .count();
                assert_eq!(heap.ready.len(), ready, "seed {seed}, step {step}");

                let expected =
                    std::iter::from_fn(|| dequeue_by_scan(&mut scan)).collect::<Vec<_>>();
                let actual = std::iter::from_fn(|| heap.dequeue()).collect::<Vec<_>>();
                assert_eq!(actual, expected, "seed {seed}, step {step}");
                for (node, (), _) in actual {
                    assert!(scheduled.insert(node), "scheduled node {node} twice");
                    active.push(node);
                }

                if active.is_empty() {
                    break;
                }

                let completions = 1 + (seed + step * 3) % active.len();
                for completion in 0..completions {
                    let index = (seed * 17 + step * 13 + completion * 7) % active.len();
                    let node = active.swap_remove(index);
                    assert_eq!(heap.finish(&node, &()).len(), scan.finish(&node, &()).len());
                }
                step += 1;
            }

            assert!(heap.is_empty(), "seed {seed}");
            assert!(scan.is_empty(), "seed {seed}");
            assert_eq!(scheduled.len(), 64, "seed {seed}");
        }
    }

    #[test]
    fn waits_for_all_edges_before_becoming_ready() {
        for edges in [[0, 1], [1, 0]] {
            let mut queue = DependencyQueue::new();
            queue.queue(0, (), [], 1);
            queue.queue(1, (), [(0, 0), (0, 1)], 1);
            queue.queue_finished();

            assert_eq!(queue.dequeue().map(|(node, (), _)| node), Some(0));
            assert!(queue.finish(&0, &edges[0]).is_empty());
            assert_eq!(queue.dequeue(), None);
            assert_eq!(queue.finish(&0, &edges[1]).len(), 1);
            assert_eq!(queue.dequeue().map(|(node, (), _)| node), Some(1));
            assert_eq!(queue.dequeue(), None);
        }
    }

    fn seeded_queue(seed: usize) -> DependencyQueue<usize, (), ()> {
        let mut queue = DependencyQueue::new();
        for node in 0..64 {
            let dependencies = (0..node)
                .filter(|dependency| (node * 37 + dependency * 17 + seed * 13) % 11 < 2)
                .map(|dependency| (dependency, ()))
                .collect::<Vec<_>>();
            queue.queue(node, (), dependencies, (node + seed) % 5 + 1);
        }
        queue
    }

    fn dequeue_by_scan(queue: &mut DependencyQueue<usize, (), ()>) -> Option<(usize, (), usize)> {
        let (key, priority) = queue
            .dep_map
            .iter()
            .filter(|(_, (dependencies, _))| dependencies.is_empty())
            .map(|(key, _)| (*key, queue.priority[key].0))
            .max_by_key(|(_, priority)| *priority)?;
        let (_, value) = queue.dep_map.remove(&key).unwrap();
        Some((key, value, priority))
    }
}
