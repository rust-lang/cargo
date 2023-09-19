use std::borrow::Borrow;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

pub struct Graph<N: Clone, E: Clone> {
    nodes: im_rc::OrdMap<N, im_rc::OrdMap<N, E>>,
}

impl<N: Eq + Ord + Clone, E: Default + Clone> Graph<N, E> {
    pub fn new() -> Graph<N, E> {
        Graph {
            nodes: im_rc::OrdMap::new(),
        }
    }

    pub fn add(&mut self, node: N) {
        self.nodes.entry(node).or_insert_with(im_rc::OrdMap::new);
    }

    pub fn link(&mut self, node: N, child: N) -> &mut E {
        self.nodes
            .entry(node)
            .or_insert_with(im_rc::OrdMap::new)
            .entry(child)
            .or_insert_with(Default::default)
    }

    pub fn contains<Q: ?Sized>(&self, k: &Q) -> bool
    where
        N: Borrow<Q>,
        Q: Ord + Eq,
    {
        self.nodes.contains_key(k)
    }

    pub fn edge(&self, from: &N, to: &N) -> Option<&E> {
        self.nodes.get(from)?.get(to)
    }

    pub fn edges(&self, from: &N) -> impl Iterator<Item = (&N, &E)> {
        self.nodes.get(from).into_iter().flat_map(|x| x.iter())
    }

    /// A topological sort of the `Graph`
    pub fn sort(&self) -> Vec<N> {
        let mut ret = Vec::new();
        let mut marks = BTreeSet::new();

        for node in self.nodes.keys() {
            self.sort_inner_visit(node, &mut ret, &mut marks);
        }

        ret
    }

    fn sort_inner_visit(&self, node: &N, dst: &mut Vec<N>, marks: &mut BTreeSet<N>) {
        if !marks.insert(node.clone()) {
            return;
        }

        for child in self.nodes[node].keys() {
            self.sort_inner_visit(child, dst, marks);
        }

        dst.push(node.clone());
    }

    pub fn iter(&self) -> impl Iterator<Item = &N> {
        self.nodes.keys()
    }

    /// Checks if there is a path from `from` to `to`.
    pub fn is_path_from_to<'a>(&'a self, from: &'a N, to: &'a N) -> bool {
        let mut stack = vec![from];
        let mut seen = BTreeSet::new();
        seen.insert(from);
        while let Some(iter) = stack.pop().and_then(|p| self.nodes.get(p)) {
            for p in iter.keys() {
                if p == to {
                    return true;
                }
                if seen.insert(p) {
                    stack.push(p);
                }
            }
        }
        false
    }

    /// Resolves one of the paths from the given dependent package down to a leaf.
    ///
    /// The path return will be the shortest path, or more accurately one of the paths with the shortest length.
    ///
    /// Each element contains a node along with an edge except the first one.
    /// The representation would look like:
    ///
    /// (Node0,) -> (Node1, Edge01) -> (Node2, Edge12)...
    pub fn path_to_bottom<'a>(&'a self, pkg: &'a N) -> Vec<(&'a N, Option<&'a E>)> {
        self.path_to(pkg, |s, p| s.edges(p))
    }

    /// Resolves one of the paths from the given dependent package up to the root.
    ///
    /// The path return will be the shortest path, or more accurately one of the paths with the shortest length.
    ///
    /// Each element contains a node along with an edge except the first one.
    /// The representation would look like:
    ///
    /// (Node0,) -> (Node1, Edge01) -> (Node2, Edge12)...
    pub fn path_to_top<'a>(&'a self, pkg: &'a N) -> Vec<(&'a N, Option<&'a E>)> {
        self.path_to(pkg, |s, pk| {
            // Note that this implementation isn't the most robust per se, we'll
            // likely have to tweak this over time. For now though it works for what
            // it's used for!
            s.nodes
                .iter()
                .filter_map(|(p, adjacent)| adjacent.get(pk).map(|e| (p, e)))
        })
    }
}

impl<'s, N: Eq + Ord + Clone + 's, E: Default + Clone + 's> Graph<N, E> {
    fn path_to<'a, F, I>(&'s self, pkg: &'a N, fn_edge: F) -> Vec<(&'a N, Option<&'a E>)>
    where
        I: Iterator<Item = (&'a N, &'a E)>,
        F: Fn(&'s Self, &'a N) -> I,
        'a: 's,
    {
        let mut back_link = BTreeMap::new();
        let mut queue = VecDeque::from([pkg]);
        let mut bottom = None;

        while let Some(p) = queue.pop_front() {
            bottom = Some(p);
            for (child, edge) in fn_edge(&self, p) {
                bottom = None;
                back_link.entry(child).or_insert_with(|| {
                    queue.push_back(child);
                    (p, edge)
                });
            }
            if bottom.is_some() {
                break;
            }
        }

        let mut result = Vec::new();
        let mut next =
            bottom.expect("the only path was a cycle, no dependency graph has this shape");
        while let Some((p, e)) = back_link.remove(&next) {
            result.push((next, Some(e)));
            next = p;
        }
        result.push((next, None));
        result.reverse();
        #[cfg(debug_assertions)]
        {
            for x in result.windows(2) {
                let [(n2, _), (n1, Some(e12))] = x else {
                    unreachable!()
                };
                assert!(std::ptr::eq(
                    self.edge(n1, n2).or(self.edge(n2, n1)).unwrap(),
                    *e12
                ));
            }
            let last = result.last().unwrap().0;
            // fixme: this may sometimes be wrong when there are cycles.
            if !fn_edge(&self, last).next().is_none() {
                self.print_for_test();
                unreachable!("The last element in the path should not have outgoing edges");
            }
        }
        result
    }
}

#[test]
fn path_to_case() {
    let mut new = Graph::new();
    new.link(0, 3);
    new.link(1, 0);
    new.link(2, 0);
    new.link(2, 1);
    assert_eq!(
        new.path_to_bottom(&2),
        vec![(&2, None), (&0, Some(&())), (&3, Some(&()))]
    );
}

impl<N: Eq + Ord + Clone, E: Default + Clone> Default for Graph<N, E> {
    fn default() -> Graph<N, E> {
        Graph::new()
    }
}

impl<N: fmt::Display + Eq + Ord + Clone, E: Clone> fmt::Debug for Graph<N, E> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(fmt, "Graph {{")?;

        for (n, e) in &self.nodes {
            writeln!(fmt, "  - {}", n)?;

            for n in e.keys() {
                writeln!(fmt, "    - {}", n)?;
            }
        }

        write!(fmt, "}}")?;

        Ok(())
    }
}

impl<N: Eq + Ord + Clone, E: Clone> Graph<N, E> {
    /// Prints the graph for constructing unit tests.
    ///
    /// For purposes of graph traversal algorithms the edge values do not matter,
    /// and the only value of the node we care about is the order it gets compared in.
    /// This constructs a graph with the same topology but with integer keys and unit edges.
    #[cfg(debug_assertions)]
    #[allow(clippy::print_stderr)]
    fn print_for_test(&self) {
        // Isolate and print a test case.
        let names = self
            .nodes
            .keys()
            .chain(self.nodes.values().flat_map(|vs| vs.keys()))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let mut new = Graph::new();
        for n1 in self.nodes.keys() {
            let name1 = names.binary_search(&n1).unwrap();
            new.add(name1);
            for n2 in self.nodes[n1].keys() {
                let name2 = names.binary_search(&n2).unwrap();
                *new.link(name1, name2) = ();
            }
        }
        eprintln!("Graph for tests = {new:#?}");
    }
}

impl<N: Eq + Ord + Clone, E: Eq + Clone> PartialEq for Graph<N, E> {
    fn eq(&self, other: &Graph<N, E>) -> bool {
        self.nodes.eq(&other.nodes)
    }
}
impl<N: Eq + Ord + Clone, E: Eq + Clone> Eq for Graph<N, E> {}

impl<N: Eq + Ord + Clone, E: Clone> Clone for Graph<N, E> {
    fn clone(&self) -> Graph<N, E> {
        Graph {
            nodes: self.nodes.clone(),
        }
    }
}
