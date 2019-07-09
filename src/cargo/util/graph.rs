use indexmap::IndexMap;
use std::borrow::Borrow;
use std::collections::HashSet;
use std::fmt;
use std::hash::Hash;
use std::num::NonZeroUsize;
use std::rc::Rc;

#[derive(Clone)]
struct EdgeLink<E: Clone> {
    value: E,
    next: Option<NonZeroUsize>,
}

#[derive(Clone)]
struct GraphCore<N: Clone, E: Clone> {
    edges: Vec<EdgeLink<E>>,
    link_count: usize,
    nodes: indexmap::IndexMap<N, indexmap::IndexMap<N, (usize, Option<usize>)>>,
}
#[derive(Copy, Clone, PartialEq, Debug)]
struct GraphAge {
    link_count: usize,
    len_edges: usize,
    len_nodes: usize,
}

impl<N: Clone, E: Clone> GraphCore<N, E> {
    fn len(&self) -> GraphAge {
        GraphAge {
            link_count: self.link_count,
            len_edges: self.edges.len(),
            len_nodes: self.nodes.len(),
        }
    }
}

#[derive(Clone)]
pub struct Graph<N: Clone, E: Clone> {
    inner: Rc<GraphCore<N, E>>,
    age: GraphAge,
}

#[derive(Clone)]
pub struct Edges<'a, E: Clone> {
    graph: &'a [EdgeLink<E>],
    index: Option<usize>,
}

impl<'a, E: Clone> Edges<'a, E> {
    pub fn is_empty(&self) -> bool {
        self.index.is_none()
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

impl<N: Eq + Hash + Clone, E: Clone + PartialEq> Graph<N, E> {
    pub fn new() -> Graph<N, E> {
        Graph {
            inner: Rc::new(GraphCore {
                edges: vec![],
                link_count: 0,
                nodes: Default::default(),
            }),
            age: GraphAge {
                link_count: 0,
                len_edges: 0,
                len_nodes: 0,
            },
        }
    }

    pub fn add(&mut self, node: N) {
        assert_eq!(self.age, self.inner.len());
        let inner = Rc::make_mut(&mut self.inner);
        inner.nodes.entry(node).or_insert_with(IndexMap::new);
        self.age = inner.len();
    }

    pub fn link(&mut self, node: N, child: N) {
        use indexmap::map::Entry;
        assert_eq!(self.age, self.inner.len());
        let inner = Rc::make_mut(&mut self.inner);
        match inner
            .nodes
            .entry(node.clone())
            .or_insert_with(IndexMap::new)
            .entry(child.clone())
        {
            Entry::Vacant(entry) => {
                inner.link_count += 1;
                entry.insert((inner.link_count, None));
            }
            Entry::Occupied(_) => {}
        };
    }

    pub fn add_edge(&mut self, node: N, child: N, edge: E) {
        use indexmap::map::Entry;
        assert_eq!(self.age, self.inner.len());
        let inner = Rc::make_mut(&mut self.inner);
        match inner
            .nodes
            .entry(node.clone())
            .or_insert_with(IndexMap::new)
            .entry(child.clone())
        {
            Entry::Vacant(entry) => {
                inner.edges.push(EdgeLink {
                    value: edge,
                    next: None,
                });
                let edge_index = inner.edges.len() - 1;
                inner.link_count += 1;
                entry.insert((inner.link_count, Some(edge_index)));
            }
            Entry::Occupied(mut entry) => {
                let edge_index = *entry.get();
                match edge_index {
                    (link_count, None) => {
                        inner.edges.push(EdgeLink {
                            value: edge,
                            next: None,
                        });
                        let edge_index = inner.edges.len() - 1;
                        entry.insert((link_count, Some(edge_index)));
                    }
                    (_, Some(mut edge_index)) => loop {
                        if inner.edges[edge_index].value == edge {
                            return;
                        }
                        if inner.edges[edge_index].next.is_none() {
                            inner.edges.push(EdgeLink {
                                value: edge,
                                next: None,
                            });
                            let new_index = NonZeroUsize::new(inner.edges.len() - 1).unwrap();
                            inner.edges[edge_index].next = Some(new_index);
                            return;
                        }
                        edge_index = inner.edges[edge_index].next.unwrap().get();
                    },
                }
            }
        }
        self.age = inner.len();
    }

    pub fn contains<Q: ?Sized>(&self, k: &Q) -> bool
    where
        N: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.inner
            .nodes
            .get_full(k)
            .filter(|(i, _, _)| *i < self.age.len_nodes)
            .is_some()
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

    /// A topological sort of the `Graph`
    pub fn sort(&self) -> Vec<N> {
        let mut ret = Vec::new();
        let mut marks = HashSet::new();

        for node in self.inner.nodes.keys().take(self.age.len_nodes) {
            self.sort_inner_visit(node, &mut ret, &mut marks);
        }

        ret
    }

    fn sort_inner_visit(&self, node: &N, dst: &mut Vec<N>, marks: &mut HashSet<N>) {
        if !marks.insert(node.clone()) {
            return;
        }

        if let Some(nodes) = self.inner.nodes.get(node) {
            for (child, (c, _)) in nodes.iter().rev() {
                if *c <= self.age.link_count {
                    self.sort_inner_visit(child, dst, marks);
                }
            }
        }

        dst.push(node.clone());
    }

    pub fn iter(&self) -> impl Iterator<Item = &N> {
        self.inner.nodes.keys().take(self.age.len_nodes)
    }

    /// Checks if there is a path from `from` to `to`.
    pub fn is_path_from_to<'a>(&'a self, from: &'a N, to: &'a N) -> bool {
        let mut stack = vec![from];
        let mut seen = HashSet::new();
        seen.insert(from);
        while let Some((_, _, iter)) = stack.pop().and_then(|p| {
            self.inner
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

    /// Resolves one of the paths from the given dependent package down to
    /// a leaf.
    pub fn path_to_bottom<'a>(&'a self, mut pkg: &'a N) -> Vec<&'a N> {
        let mut result = vec![pkg];
        while let Some(p) = self
            .inner
            .nodes
            .get_full(pkg)
            .filter(|(i, _, _)| *i < self.age.len_nodes)
            .and_then(|(_, _, p)| {
                p.iter()
                    // Note that we can have "cycles" introduced through dev-dependency
                    // edges, so make sure we don't loop infinitely.
                    .filter(|(_, (c, _))| c <= &self.age.link_count)
                    .find(|&(node, _)| !result.contains(&node))
                    .map(|(p, _)| p)
            })
        {
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
            self.inner
                .nodes
                .iter()
                .take(self.age.len_nodes)
                .filter(|&(_, adjacent)| {
                    adjacent
                        .get_full(pkg)
                        .filter(|(i, _, _)| *i < self.age.len_nodes)
                        .is_some()
                })
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

        for (from, e) in self.inner.nodes.iter().take(self.age.len_nodes) {
            writeln!(fmt, "  - {}", from)?;

            for (to, (c, _)) in e.iter() {
                if *c <= self.age.link_count {
                    writeln!(fmt, "    - {}", to)?;
                    for edge in self.edge(from, to) {
                        writeln!(fmt, "      - {:?}", edge)?;
                    }
                }
            }
        }

        write!(fmt, "}}")?;

        Ok(())
    }
}

impl<N: Eq + Hash + Clone, E: Eq + Clone + PartialEq> PartialEq for Graph<N, E> {
    fn eq(&self, other: &Graph<N, E>) -> bool {
        self.inner.nodes.eq(&other.inner.nodes)
    }
}
impl<N: Eq + Hash + Clone, E: Eq + Clone> Eq for Graph<N, E> {}
