//! This module implements some Graph data structures optimized for Cargo's uses.
//! The module documentation here will try to justify why Cargo needs the odd set of requirements
//! that the structures provide, but the how is documented on each type.
//!
//! Cargo uses these types for several things in the codebase, but by far the most important is
//! the "dependency graph". It is a graph that records for each package all the packages that it
//! depends on. It is used to decide what order to build the packages, for what targets, and when
//! they need to be rebuilt (among many other things). One small complication is that we may have
//! more then one edge between the same two packages because the same package might be present
//! in both `[dependencies]` and `[build-dependencies]`.
//! This module must provide a way to read it efficiently with no fuss nor complications. The
//! "dependency graph" is constructed in two ways, when reading a lockfile and by running the resolver.
//! When reading a lockfile the only complication is that the lockfile has what is depended on but
//! not why. This module must provide a way to link two packages with out providing an example
//! of the type of the edge.
//! Running the resolver is the source of our most unusual requirements.
//! The fundamental pattern of the resolver is like:
//!
//! ```ignore
//! fn resolve_next(dependency_graph_so_far: _, mut unresolved_dependencies: _) -> Result<DependencyGraph, Problem> {
//!     if let Some(unresolved) = unresolved_dependencies.pop() {
//!         let combined_problem = Problem::new();
//!         for candidate in unresolved {
//!             let mut dgsf = dependency_graph_so_far.clone();
//!             let mut ud = unresolved_dependencies.clone();
//!             if let Err(prob) = activate(&mut dgsf, &mut ud, candidate) {
//!                     combined_problem.extend(prob);
//!                     continue
//!                 };
//!             match resolve_next(dgsf, ud) {
//!                 Ok(dg) => return Ok(dg),
//!                 Err(prob) => {
//!                     if dependency_graph_so_far.has(prob) {
//!                         return Err(prob)
//!                     }
//!                 }
//!             }
//!         }
//!         Err(combined_problem)
//!     } else {
//!         Ok(finalize(dependency_graph_so_far))
//!     }
//! }
//! ```
//!
//! The real resolver is not recursive to avoid blowing the stack, and has lots of other state to
//! maintain. The most expensive (non recursive) call in this algorithm is the
//! `dependency_graph_so_far.clone();`. To make this more annoying the first thing we try will
//! probably work, and any work we do to prepare for the next iteration is wasted. If we had a
//! `undo_activate` we could be much more efficient, completely remove the `.clone()` and just
//! `undo_activate` if things tern out to not work. Unfortunately, making sure `undo_activate`
//! handles all the corner cases correctly is not practical for the resolver. However, this is
//! possible for a graph like thing, a `clone` means record the size of the graph. a `&mut self`
//! method means undo everything newer and do the mutation, a `&self` method means only look at the
//! older part. This module provides a `StackGraph` type to encapsulate this pattern.

use indexmap::IndexMap;
use std::borrow::Borrow;
use std::cell::{Ref, RefCell, RefMut};
use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::hash::Hash;
use std::rc::Rc;

type EdgeIndex = usize;

#[derive(Clone, Debug)]
struct EdgeLink<E: Clone> {
    value: Option<E>,
    /// the index into the edge list of the next edge related to the same (from, to) nodes
    next: Option<EdgeIndex>, // can be `NonZeroUsize` but not worth the boilerplate
    /// the index into the edge list of the previous edge related to the same (from, to) nodes
    previous: Option<EdgeIndex>,
}

/// This is a directed Graph structure. Each edge can have an `E` associated with it,
/// but may have more then one or none. Furthermore, it is designed to be "append only" so that
/// it can be queried as it would have been when it was smaller. This allows a `reset_to` method
/// that efficiently undoes the most reason modifications.
#[derive(Clone)]
pub struct Graph<N: Clone, E: Clone> {
    /// an index based linked list of the edge data for links. This maintains insertion order.
    edges: Vec<EdgeLink<E>>,
    /// a hashmap that stores the set of nodes. This is an `IndexMap` so it maintains insertion order.
    /// For each node it stores all the other nodes that it links to.
    /// For each link it stores the first index into `edges`.
    nodes: indexmap::IndexMap<N, indexmap::IndexMap<N, EdgeIndex>>,
}

/// All the data needed to query the prefix of a `Graph`. The only way for eny of the `len_` in
/// this to decrease is to call `reset_to`. All other modifications of a graph will increase at
/// least one of the `len_`.
#[derive(Copy, Clone, PartialEq, Debug, Ord, PartialOrd, Eq)]
struct GraphAge {
    /// The number of stored edges, increased when `add_edge` or `link` is called.
    len_edges: usize,
    /// The number of stored nodes, increased when `add` is called or
    /// if `add_edge` or `link` is called with a previously unseen node.
    len_nodes: usize,
}

impl<N: Clone, E: Clone> Graph<N, E> {
    fn len(&self) -> GraphAge {
        GraphAge {
            len_edges: self.edges.len(),
            len_nodes: self.nodes.len(),
        }
    }
}

impl<N: Clone + Eq + Hash, E: Clone + PartialEq> Graph<N, E> {
    pub fn new() -> Graph<N, E> {
        Graph {
            edges: vec![],
            nodes: Default::default(),
        }
    }

    /// This resets a Graph to the same state as it was when the passed `age` was made.
    /// All the other `&mut` methods are guaranteed to increase the `len` of the Graph.
    /// So the reset can be accomplished by throwing out all newer items and fixing internal pointers.
    fn reset_to(&mut self, age: GraphAge) {
        // the prefix we are resetting to had `age.len_nodes`, so remove all newer nodes
        assert!(self.nodes.len() >= age.len_nodes);
        // IndexMap dose not have a `truncate` so we roll our own
        while self.nodes.len() > age.len_nodes {
            self.nodes.pop();
        }

        // the prefix we are resetting to had `age.len_edges`, so remove all links pointing to newer edges
        for (_, lookup) in self.nodes.iter_mut() {
            // IndexMap dose not have a `last` so we roll our own
            while lookup.len() >= 1
                && lookup
                    .get_index(lookup.len() - 1)
                    .filter(|(_, idx)| idx >= &&age.len_edges)
                    .is_some()
            {
                lookup.pop();
            }
        }

        // the prefix we are resetting to had `age.len_edges`, so
        assert!(self.edges.len() >= age.len_edges);
        // remove all newer edges and record the references that need to be fixed
        let to_fix: Vec<EdgeIndex> = self
            .edges
            .drain(age.len_edges..)
            .filter_map(|e| e.previous)
            .filter(|idx| idx < &age.len_edges)
            .collect();

        // fix references into the newer edges we remove
        for idx in to_fix {
            self.edges[idx].next = None;
        }
        assert_eq!(self.len(), age);
    }

    pub fn add(&mut self, node: N) {
        // IndexMap happens to do exactly what we need to keep the ordering correct.
        // This can be undone as it will increase the `len_nodes` if `node` is new.
        self.nodes.entry(node).or_insert_with(IndexMap::new);
    }

    /// connect `node`to `child` with out associating any data.
    pub fn link(&mut self, node: N, child: N) {
        use indexmap::map::Entry;
        // IndexMap happens to do exactly what we need to keep the ordering correct.
        // This can be undone as it will increase the `len_nodes` if `node` is new.
        match self
            .nodes
            .entry(node)
            .or_insert_with(IndexMap::new)
            .entry(child)
        {
            Entry::Vacant(entry) => {
                // add the new edge and link
                // This can be undone as it will increase the `len_edges`.
                self.edges.push(EdgeLink {
                    value: None,
                    next: None,
                    previous: None,
                });
                let edge_index: EdgeIndex = self.edges.len() - 1;
                entry.insert(edge_index);
            }
            Entry::Occupied(_) => {
                // this pair is already linked
            }
        };
    }

    /// connect `node`to `child` associating it with `edge`.
    pub fn add_edge(&mut self, node: N, child: N, edge: E) {
        use indexmap::map::Entry;
        let edge = Some(edge);
        // IndexMap happens to do exactly what we need to keep the ordering correct.
        // This can be undone as it will increase the `len_nodes` if `node` is new.
        match self
            .nodes
            .entry(node)
            .or_insert_with(IndexMap::new)
            .entry(child)
        {
            Entry::Vacant(entry) => {
                // add the new edge and link
                // This can be undone as it will increase the `len_edges`.
                self.edges.push(EdgeLink {
                    value: edge,
                    next: None,
                    previous: None,
                });
                let edge_index: EdgeIndex = self.edges.len() - 1;
                entry.insert(edge_index);
            }
            Entry::Occupied(entry) => {
                // this pare is already linked
                let mut edge_index = *entry.get();
                loop {
                    // follow the linked list
                    if self.edges[edge_index].value == edge {
                        return;
                    }
                    if self.edges[edge_index].next.is_none() {
                        // we found the end, add the new edge
                        // This can be undone as it will increase the `len_edges`.
                        self.edges.push(EdgeLink {
                            value: edge,
                            next: None,
                            previous: Some(edge_index),
                        });
                        let new_index: EdgeIndex = self.edges.len() - 1;
                        // make the list point to the new edge
                        self.edges[edge_index].next = Some(new_index);
                        return;
                    }
                    edge_index = self.edges[edge_index].next.unwrap();
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
            index: self.nodes.get(from).and_then(|x| x.get(to).copied()),
        }
    }

    pub fn edges(&self, from: &N) -> impl Iterator<Item = (&N, Edges<'_, E>)> {
        let edges = &self.edges;
        self.nodes.get(from).into_iter().flat_map(move |x| {
            x.iter().map(move |(to, idx)| {
                (
                    to,
                    Edges {
                        graph: edges,
                        index: Some(*idx),
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

/// This is a directed Graph structure, that builds on the `Graph`'s "append only" internals
/// to provide:
///  - `O(ln(number of clones))` clone, no dependence on the size of the graph
///  - the clone has no overhead to read the `Graph` as it was
///  - no overhead over using the `Graph` directly when modifying the biggest clone
/// Is this too good to be true?
///  - If a modification (`&mut` method) is done to a smaller older clone then a full `O(N)`
///    deep clone will happen internally.
pub struct StackGraph<N: Clone, E: Clone> {
    /// The `Graph` shared by all clones, this `StackGraph` refers to only the prefix of size `age`
    inner: Rc<RefCell<Graph<N, E>>>,
    /// The shared list of all extant references to the same `Graph`.
    /// The largest one is allowed to reset and expend the `Graph` without doing a deep clone.
    /// There can be more then one clone at the same age, so each age is associated with a count.
    other_refs: Rc<RefCell<BTreeMap<GraphAge, usize>>>,
    /// The size of the `Graph` that this `StackGraph` refers to.
    age: GraphAge,
}

#[test]
fn demonstrate_stack_graph_can_read_all_clones() {
    let mut graph = StackGraph::new();
    let mut stack = Vec::new();
    graph.add_edge(1, 2, 1);
    stack.push(graph.clone());
    graph.add_edge(2, 3, 2);
    stack.push(graph.clone());
    graph.add_edge(2, 3, 3);
    stack.push(graph.clone());
    // violate stack discipline, so a deep clone is needed
    graph = stack[0].clone();
    graph.add_edge(2, 3, 4);
    assert_eq!(
        stack.iter().map(|g| g.contains(&2)).collect::<Vec<bool>>(),
        [false, true, true]
    );
    assert_eq!(stack[1].borrow().edge(&2, &3).collect::<Vec<_>>(), [&2]);
    assert_eq!(stack[2].borrow().edge(&2, &3).collect::<Vec<_>>(), [&2, &3]);
    assert_eq!(graph.borrow().edge(&2, &3).collect::<Vec<_>>(), [&4]);
}

impl<N: Clone, E: Clone> Clone for StackGraph<N, E> {
    fn clone(&self) -> Self {
        *RefCell::borrow_mut(&self.other_refs)
            .entry(self.age)
            .or_insert(0) += 1;
        StackGraph {
            inner: Rc::clone(&self.inner),
            other_refs: Rc::clone(&self.other_refs),
            age: self.age,
        }
    }
}

impl<N: Clone, E: Clone> Drop for StackGraph<N, E> {
    fn drop(&mut self) {
        let mut borrow = RefCell::borrow_mut(&self.other_refs);
        if let std::collections::btree_map::Entry::Occupied(mut val) = borrow.entry(self.age) {
            *val.get_mut() -= 1;
            if val.get() == &0 {
                val.remove();
            }
        }
    }
}

impl<N: Eq + Hash + Clone, E: Clone + PartialEq> StackGraph<N, E> {
    pub fn new() -> StackGraph<N, E> {
        let inner = Graph::new();
        let age = inner.len();
        let mut other_refs = BTreeMap::new();
        other_refs.insert(age, 1);
        StackGraph {
            inner: Rc::new(RefCell::new(inner)),
            other_refs: Rc::new(RefCell::new(other_refs)),
            age,
        }
    }

    pub fn borrow(&self) -> StackGraphView<'_, N, E> {
        let inner = RefCell::borrow(&self.inner);
        assert!(self.age.len_edges <= inner.len().len_edges, "The internal Graph was reset to something smaller before you tried to read from the StackGraphView");
        assert!(self.age.len_nodes <= inner.len().len_nodes, "The internal Graph was reset to something smaller before you tried to read from the StackGraphView");
        StackGraphView {
            inner,
            age: self.age,
        }
    }

    /// Gets mutable access to the inner `Graph`. Uses `other_refs` to determine if a deep clone is
    /// needed, and runs `reset_to` to get the `Graph` into the state it was in.
    ///
    /// It is the responsibility of the caller to re-add this clone to `other_refs` after the new age is determined.
    fn activate(&mut self) -> RefMut<'_, Graph<N, E>> {
        let inner = if {
            let mut borrow = RefCell::borrow_mut(&self.other_refs);
            if let std::collections::btree_map::Entry::Occupied(mut val) = borrow.entry(self.age) {
                *val.get_mut() -= 1;
                if val.get() == &0 {
                    val.remove();
                }
            }
            borrow
                .keys()
                .rev()
                .next()
                .map(|a| a <= &self.age)
                .unwrap_or(true)
        } {
            // we are the biggest clone, so we can add to inner directly.
            let mut inner = RefCell::borrow_mut(&mut self.inner);
            if inner.len() != self.age {
                // clean up after the larger clone that has since been dropped
                inner.reset_to(self.age);
            }
            inner
        } else {
            // a bigger clone still exists so do a deep clone.
            self.other_refs = Rc::new(RefCell::new(BTreeMap::new()));
            let new = Rc::make_mut(&mut self.inner);
            let mut inner = RefCell::borrow_mut(new);
            inner.reset_to(self.age);
            inner
        };
        inner
    }

    pub fn add(&mut self, node: N) {
        self.age = {
            let mut g = self.activate();
            g.add(node);
            g.len()
        };
        *RefCell::borrow_mut(&self.other_refs)
            .entry(self.age)
            .or_insert(0) += 1;
    }

    /// connect `node`to `child` with out associating any data.
    pub fn link(&mut self, node: N, child: N) {
        self.age = {
            let mut g = self.activate();
            g.link(node, child);
            g.len()
        };
        *RefCell::borrow_mut(&self.other_refs)
            .entry(self.age)
            .or_insert(0) += 1;
    }

    /// connect `node`to `child` associating it with `edge`.
    pub fn add_edge(&mut self, node: N, child: N, edge: E) {
        self.age = {
            let mut g = self.activate();
            g.add_edge(node, child, edge);
            g.len()
        };
        *RefCell::borrow_mut(&self.other_refs)
            .entry(self.age)
            .or_insert(0) += 1;
    }
}

/// Methods for viewing the prefix of a `Graph` as stored in a `StackGraph`.
/// Other views of the inner `Graph` may have added things after this `StackGraph` was created.
/// So, we need to filter everything to only the prefix recorded by `self.age`.
impl<N: Eq + Hash + Clone, E: Clone + PartialEq> StackGraph<N, E> {
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
            for (p, idx) in iter.iter() {
                if *idx < self.age.len_edges {
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

            for (to, idx) in e.iter() {
                if *idx < self.age.len_edges {
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

/// And Iterator of the edges related to one link in a `Graph` or a `StackGraphView`.
#[derive(Clone, Debug)]
pub struct Edges<'a, E: Clone> {
    /// The arena the linked list is stored in. If we are used with a `StackGraphView`
    /// then this is only the related prefix of the arena. So we need to filter out
    /// pointers past the part we are given.
    graph: &'a [EdgeLink<E>],
    index: Option<EdgeIndex>,
}

impl<'a, E: Clone> Edges<'a, E> {
    pub fn is_empty(&self) -> bool {
        self.index
            .and_then(|idx| self.graph.get(idx))
            .and_then(|l| l.value.as_ref())
            .is_none()
    }
}

impl<'a, E: Clone> Iterator for Edges<'a, E> {
    type Item = &'a E;

    fn next(&mut self) -> Option<&'a E> {
        while let Some(edge_link) = self.index.and_then(|idx| {
            // Check that the `idx` points to something in `self.graph`. It may not if we are
            // looking at a smaller prefix of a larger graph.
            self.graph.get(idx)
        }) {
            self.index = edge_link.next;
            if let Some(value) = edge_link.value.as_ref() {
                return Some(value);
            }
        }
        self.index = None;
        None
    }
}

/// A RAII gard that allows getting `&` references to the prefix of a `Graph` as stored in a `StackGraph`.
/// Other clones of the inner `StackGraph` may have added things after this `StackGraph` was created
/// and before this `StackGraphView` was `.borrow()`ed.
/// So, we need to filter everything to only the prefix recorded by `self.age`.
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
                .filter(|idx| idx < &self.age.len_edges),
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
                x.iter().map(move |(to, idx)| {
                    (
                        to,
                        Edges {
                            graph: edges,
                            index: Some(*idx),
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
                    .filter(|(_, idx)| **idx < self.age.len_edges)
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
