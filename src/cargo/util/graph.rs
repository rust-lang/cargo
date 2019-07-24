use indexmap::IndexMap;
use std::borrow::Borrow;
use std::cell::{Ref, RefCell, RefMut};
use std::collections::HashSet;
use std::fmt;
use std::hash::Hash;
use std::num::NonZeroUsize;
use std::rc::Rc;

#[derive(Clone, Debug)]
struct EdgeLink<E: Clone> {
    value: E,
    next: Option<NonZeroUsize>,
    previous: Option<usize>,
}

/// This is a directed Graph structure. Each edge can have an `E` associated with it,
/// but may have more then one or none. Furthermore, it is designed to be "append only" so that
/// it can be queried as it would have bean when it was smaller. This allows a `reset_to` method
/// that efficiently undoes the most reason modifications.
#[derive(Clone)]
pub struct Graph<N: Clone, E: Clone> {
    /// an index based linked list of the edge data for links. This maintains insertion order.
    edges: Vec<EdgeLink<E>>,
    /// The total number of links in the `Graph`. This needs to be kept separately as a link may
    /// not have any edge data associated with it.
    link_count: usize,
    /// a hashmap that stores the set of nodes. This is an `IndexMap` so it maintains insertion order.
    /// For etch node it stores all the other nodes that it links to.
    /// For etch link it stores the link number when it was inserted and optionally a first index into `edges`.   
    nodes: indexmap::IndexMap<N, indexmap::IndexMap<N, (usize, Option<usize>)>>,
}

/// All the data needed to query the prefix of a `Graph`
#[derive(Copy, Clone, PartialEq, Debug)]
struct GraphAge {
    link_count: usize,
    len_edges: usize,
    len_nodes: usize,
}

impl<N: Clone, E: Clone> Graph<N, E> {
    fn len(&self) -> GraphAge {
        GraphAge {
            link_count: self.link_count,
            len_edges: self.edges.len(),
            len_nodes: self.nodes.len(),
        }
    }
}

impl<N: Clone + Eq + Hash, E: Clone + PartialEq> Graph<N, E> {
    pub fn new() -> Graph<N, E> {
        Graph {
            edges: vec![],
            link_count: 0,
            nodes: Default::default(),
        }
    }

    fn reset_to(&mut self, age: GraphAge) {
        // the prefix we are resetting to had `age.len_nodes`, so remove all newer nodes
        assert!(self.nodes.len() >= age.len_nodes);
        while self.nodes.len() > age.len_nodes {
            self.nodes.pop();
        }

        // the prefix we are resetting to had `age.link_count`, so remove all newer links
        assert!(self.link_count >= age.link_count);
        for (_, lookup) in self.nodes.iter_mut() {
            // todo: this dose not need to look at every link to remove edges
            //       but this is only called when a lockfile is not being used
            //       so it dose not need to be the fastest
            lookup.retain(|_, idx| {
                if idx.1 >= Some(age.len_edges) {
                    idx.1 = None;
                }
                idx.0 <= age.link_count
            });
        }
        self.link_count = age.link_count;

        // the prefix we are resetting to had `age.len_edges`, so
        assert!(self.edges.len() >= age.len_edges);
        // remove all newer edges
        let to_fix: Vec<usize> = self
            .edges
            .drain(age.len_edges..)
            .filter_map(|e| e.previous)
            .filter(|idx| idx <= &age.len_edges)
            .collect();

        // fix references into the newer edges we are about to remove
        for idx in to_fix {
            self.edges[idx].next = None;
        }
        assert_eq!(self.len(), age);
    }

    pub fn add(&mut self, node: N) {
        // IndexMap happens to do exactly what we need to keep the ordering correct.
        self.nodes.entry(node).or_insert_with(IndexMap::new);
    }

    pub fn link(&mut self, node: N, child: N) {
        use indexmap::map::Entry;
        // IndexMap happens to do exactly what we need to keep the ordering correct.
        match self
            .nodes
            .entry(node.clone())
            .or_insert_with(IndexMap::new)
            .entry(child.clone())
        {
            Entry::Vacant(entry) => {
                // add the new link at the end and record the new link count
                self.link_count += 1;
                entry.insert((self.link_count, None));
            }
            Entry::Occupied(_) => {
                // this pare is already linked
            }
        };
    }

    pub fn add_edge(&mut self, node: N, child: N, edge: E) {
        use indexmap::map::Entry;
        // IndexMap happens to do exactly what we need to keep the ordering correct.
        match self
            .nodes
            .entry(node.clone())
            .or_insert_with(IndexMap::new)
            .entry(child.clone())
        {
            Entry::Vacant(entry) => {
                // add the new edge, and link and fix the new link count
                self.edges.push(EdgeLink {
                    value: edge,
                    next: None,
                    previous: None,
                });
                let edge_index = self.edges.len() - 1;
                self.link_count += 1;
                entry.insert((self.link_count, Some(edge_index)));
            }
            Entry::Occupied(mut entry) => {
                // this pare is already linked
                let edge_index = *entry.get();
                match edge_index {
                    (link_count, None) => {
                        // but when it was linked before it did not have edge data, so add it.
                        self.edges.push(EdgeLink {
                            value: edge,
                            next: None,
                            previous: None,
                        });
                        let edge_index = self.edges.len() - 1;
                        entry.insert((link_count, Some(edge_index)));
                    }
                    (_, Some(mut edge_index)) => loop {
                        // follow the linked list
                        if self.edges[edge_index].value == edge {
                            return;
                        }
                        if self.edges[edge_index].next.is_none() {
                            // we found the end, add the new edge
                            self.edges.push(EdgeLink {
                                value: edge,
                                next: None,
                                previous: Some(edge_index),
                            });
                            let new_index = NonZeroUsize::new(self.edges.len() - 1).unwrap();
                            // make the list point to the new edge
                            self.edges[edge_index].next = Some(new_index);
                            return;
                        }
                        edge_index = self.edges[edge_index].next.unwrap().get();
                    },
                }
            }
        }
    }

    pub fn contains<Q: ?Sized>(&self, k: &Q) -> bool
    where
        N: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.nodes.contains_key(k)
    }

    pub fn iter(&self) -> impl Iterator<Item = &N> {
        self.borrow().nodes.keys()
    }

    pub fn edge(&self, from: &N, to: &N) -> Edges<'_, E> {
        Edges {
            graph: &self.edges,
            index: self
                .nodes
                .get(from)
                .and_then(|x| x.get(to).copied())
                .and_then(|x| x.1),
        }
    }

    pub fn edges(&self, from: &N) -> impl Iterator<Item = (&N, Edges<'_, E>)> {
        let edges = &self.edges;
        self.nodes.get(from).into_iter().flat_map(move |x| {
            x.iter().map(move |(to, (_, idx))| {
                (
                    to,
                    Edges {
                        graph: edges,
                        index: *idx,
                    },
                )
            })
        })
    }

    /// A topological sort of the `Graph`
    pub fn sort(&self) -> Vec<N> {
        let mut ret = Vec::new();
        let mut marks = HashSet::new();

        for node in self.nodes.keys() {
            self.sort_inner_visit(node, &mut ret, &mut marks);
        }

        ret
    }

    fn sort_inner_visit(&self, node: &N, dst: &mut Vec<N>, marks: &mut HashSet<N>) {
        if !marks.insert(node.clone()) {
            return;
        }

        if let Some(nodes) = self.nodes.get(node) {
            for child in nodes.keys().rev() {
                self.sort_inner_visit(child, dst, marks);
            }
        }

        dst.push(node.clone());
    }

    /// Checks if there is a path from `from` to `to`.
    pub fn is_path_from_to<'a>(&'a self, from: &'a N, to: &'a N) -> bool {
        let mut stack = vec![from];
        let mut seen = HashSet::new();
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

    /// Resolves one of the paths from the given dependent package down to
    /// a leaf.
    pub fn path_to_bottom<'a>(&'a self, mut pkg: &'a N) -> Vec<&'a N> {
        let mut result = vec![pkg];
        while let Some(p) = self.nodes.get(pkg).and_then(|p| {
            p.keys()
                // Note that we can have "cycles" introduced through dev-dependency
                // edges, so make sure we don't loop infinitely.
                .find(|node| !result.contains(node))
        }) {
            result.push(p);
            pkg = p;
        }
        result
    }

    /// Resolves one of the paths from the given dependent package up to
    /// the root.
    pub fn path_to_top<'a>(&'a self, mut pkg: &'a N) -> Vec<&'a N> {
        // Note that this implementation isn't the most robust per se, we'll
        // likely have to tweak this over time. For now though it works for what
        // it's used for!
        let mut result = vec![pkg];
        let first_pkg_depending_on = |pkg: &N, res: &[&N]| {
            self.borrow()
                .nodes
                .iter()
                .filter(|&(_, adjacent)| adjacent.contains_key(pkg))
                // Note that we can have "cycles" introduced through dev-dependency
                // edges, so make sure we don't loop infinitely.
                .find(|&(node, _)| !res.contains(&node))
                .map(|(p, _)| p)
        };
        while let Some(p) = first_pkg_depending_on(pkg, &result) {
            result.push(p);
            pkg = p;
        }
        result
    }
}

impl<N: Eq + Hash + Clone, E: Clone + PartialEq> Default for Graph<N, E> {
    fn default() -> Graph<N, E> {
        Graph::new()
    }
}

impl<N: fmt::Display + Eq + Hash + Clone, E: Clone + fmt::Debug + PartialEq> fmt::Debug
    for Graph<N, E>
{
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(fmt, "Graph {{")?;

        for (from, e) in self.nodes.iter() {
            writeln!(fmt, "  - {}", from)?;

            for to in e.keys() {
                writeln!(fmt, "    - {}", to)?;
                for edge in self.edge(from, to) {
                    writeln!(fmt, "      - {:?}", edge)?;
                }
            }
        }

        write!(fmt, "}}")?;

        Ok(())
    }
}

impl<N: Eq + Hash + Clone, E: Eq + Clone + PartialEq> PartialEq for Graph<N, E> {
    fn eq(&self, other: &Graph<N, E>) -> bool {
        self.nodes.eq(&other.nodes)
    }
}
impl<N: Eq + Hash + Clone, E: Eq + Clone> Eq for Graph<N, E> {}

#[derive(Clone, Debug)]
pub struct Edges<'a, E: Clone> {
    graph: &'a [EdgeLink<E>],
    index: Option<usize>,
}

impl<'a, E: Clone> Edges<'a, E> {
    pub fn is_empty(&self) -> bool {
        self.index.filter(|&idx| idx < self.graph.len()).is_none()
    }
}

impl<'a, E: Clone> Iterator for Edges<'a, E> {
    type Item = &'a E;

    fn next(&mut self) -> Option<&'a E> {
        let old_index = self.index;
        old_index
            .and_then(|old_index| self.graph.get(old_index))
            .map(|edge_link| {
                self.index = edge_link.next.map(|i| i.get());
                &edge_link.value
            })
    }
}

/// This is a directed Graph structure, that builds on the `Graph`'s "append only" internals
/// to provide:
///  - `O(1)` clone
///  - the clone has no overhead to read the `Graph` as it was
///  - no overhead over using the `Graph` directly when modifying
/// Is this too good to be true? There are two caveats:
///  - It can only be modified using a strict "Stack Discipline", only modifying the biggest clone
///    of the graph.
///  - You can drop bigger modified clones, to allow a smaller clone to be activated for modifying.
///    this "backtracking" operation can be `O(n)`
#[derive(Clone)]
pub struct StackGraph<N: Clone, E: Clone> {
    inner: Rc<RefCell<Graph<N, E>>>,
    age: GraphAge,
}

impl<N: Eq + Hash + Clone, E: Clone + PartialEq> StackGraph<N, E> {
    pub fn new() -> StackGraph<N, E> {
        let inner = Graph::new();
        let age = inner.len();
        StackGraph {
            inner: Rc::new(RefCell::new(inner)),
            age,
        }
    }

    pub fn borrow(&self) -> StackGraphView<'_, N, E> {
        let inner = RefCell::borrow(&self.inner);
        assert!(self.age.len_edges <= inner.len().len_edges);
        assert!(self.age.len_nodes <= inner.len().len_nodes);
        assert!(self.age.link_count <= inner.link_count);
        StackGraphView {
            inner,
            age: self.age,
        }
    }

    fn activate(&mut self) -> RefMut<'_, Graph<N, E>> {
        let mut inner = RefCell::borrow_mut(&mut self.inner);
        if self.age != inner.len() {
            inner.reset_to(self.age);
        }
        inner
    }

    pub fn add(&mut self, node: N) {
        self.age = {
            let mut g = self.activate();
            g.add(node);
            g.len()
        };
    }

    pub fn link(&mut self, node: N, child: N) {
        self.age = {
            let mut g = self.activate();
            g.link(node, child);
            g.len()
        };
    }

    pub fn add_edge(&mut self, node: N, child: N, edge: E) {
        self.age = {
            let mut g = self.activate();
            g.add_edge(node, child, edge);
            g.len()
        };
    }

    pub fn contains<Q: ?Sized>(&self, k: &Q) -> bool
    where
        N: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.borrow()
            .inner
            .nodes
            .get_full(k)
            .filter(|(i, _, _)| *i < self.age.len_nodes)
            .is_some()
    }

    /// Checks if there is a path from `from` to `to`.
    pub fn is_path_from_to<'a>(&'a self, from: &'a N, to: &'a N) -> bool {
        let mut stack = vec![from];
        let mut seen = HashSet::new();
        let inner = &self.borrow().inner;
        seen.insert(from);
        while let Some((_, _, iter)) = stack.pop().and_then(|p| {
            inner
                .nodes
                .get_full(p)
                .filter(|(i, _, _)| *i < self.age.len_nodes)
        }) {
            for (p, (c, _)) in iter.iter() {
                if *c <= self.age.link_count {
                    if p == to {
                        return true;
                    }
                    if seen.insert(p) {
                        stack.push(p);
                    }
                }
            }
        }
        false
    }
}

impl<N: Eq + Hash + Clone, E: Clone + PartialEq> Default for StackGraph<N, E> {
    fn default() -> StackGraph<N, E> {
        StackGraph::new()
    }
}

impl<N: fmt::Display + Eq + Hash + Clone, E: Clone + fmt::Debug + PartialEq> fmt::Debug
    for StackGraph<N, E>
{
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(fmt, "Graph {{")?;

        for (from, e) in self.borrow().inner.nodes.iter().take(self.age.len_nodes) {
            writeln!(fmt, "  - {}", from)?;

            for (to, (c, _)) in e.iter() {
                if *c <= self.age.link_count {
                    writeln!(fmt, "    - {}", to)?;
                    for edge in self.borrow().edge(from, to) {
                        writeln!(fmt, "      - {:?}", edge)?;
                    }
                }
            }
        }

        write!(fmt, "}}")?;

        Ok(())
    }
}

pub struct StackGraphView<'a, N: Clone, E: Clone> {
    inner: Ref<'a, Graph<N, E>>,
    age: GraphAge,
}

impl<'a, N: Eq + Hash + Clone, E: Clone + PartialEq> StackGraphView<'a, N, E> {
    pub fn iter(&self) -> impl Iterator<Item = &N> {
        self.inner.nodes.keys().take(self.age.len_nodes)
    }

    pub fn edge(&self, from: &N, to: &N) -> Edges<'_, E> {
        Edges {
            graph: &self.inner.edges[..self.age.len_edges],
            index: self
                .inner
                .nodes
                .get(from)
                .and_then(|x| x.get(to).copied())
                .filter(|(c, _)| c <= &self.age.link_count)
                .and_then(|x| x.1),
        }
    }

    pub fn edges(&self, from: &N) -> impl Iterator<Item = (&N, Edges<'_, E>)> {
        let edges = &self.inner.edges[..self.age.len_edges];
        self.inner
            .nodes
            .get_full(from)
            .filter(|(i, _, _)| *i < self.age.len_nodes)
            .into_iter()
            .flat_map(move |(_, _, x)| {
                x.iter()
                    .filter(move |(_, (c, _))| c <= &self.age.link_count)
                    .map(move |(to, (_, idx))| {
                        (
                            to,
                            Edges {
                                graph: edges,
                                index: *idx,
                            },
                        )
                    })
            })
    }

    /// Resolves one of the paths from the given dependent package down to
    /// a leaf.
    pub fn path_to_bottom<'s>(&'s self, mut pkg: &'s N) -> Vec<&'s N> {
        let mut result = vec![pkg];
        while let Some(p) = self
            .inner
            .nodes
            .get_full(pkg)
            .filter(|(i, _, _)| *i < self.age.len_nodes)
            .and_then(|(_, _, p)| {
                p.iter()
                    .filter(|(_, (c, _))| c <= &self.age.link_count)
                    // Note that we can have "cycles" introduced through dev-dependency
                    // edges, so make sure we don't loop infinitely.
                    .find(|&(node, _)| !result.contains(&node))
                    .map(|(p, _)| p)
            })
        {
            result.push(p);
            pkg = p;
        }
        result
    }
}
