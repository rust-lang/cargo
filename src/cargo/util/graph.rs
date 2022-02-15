use std::borrow::Borrow;
use std::collections::BTreeSet;
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

    /// Resolves one of the paths from the given dependent package down to
    /// a leaf.
    ///
    /// Each element contains a node along with an edge except the first one.
    /// The representation would look like:
    ///
    /// (Node0,) -> (Node1, Edge01) -> (Node2, Edge12)...
    pub fn path_to_bottom<'a>(&'a self, mut pkg: &'a N) -> Vec<(&'a N, Option<&'a E>)> {
        let mut result = vec![(pkg, None)];
        while let Some(p) = self.nodes.get(pkg).and_then(|p| {
            p.iter()
                // Note that we can have "cycles" introduced through dev-dependency
                // edges, so make sure we don't loop infinitely.
                .find(|&(node, _)| result.iter().all(|p| p.0 != node))
                .map(|(node, edge)| (node, Some(edge)))
        }) {
            result.push(p);
            pkg = p.0;
        }
        result
    }

    /// Resolves one of the paths from the given dependent package up to
    /// the root.
    ///
    /// Each element contains a node along with an edge except the first one.
    /// The representation would look like:
    ///
    /// (Node0,) -> (Node1, Edge01) -> (Node2, Edge12)...
    pub fn path_to_top<'a>(&'a self, mut pkg: &'a N) -> Vec<(&'a N, Option<&'a E>)> {
        // Note that this implementation isn't the most robust per se, we'll
        // likely have to tweak this over time. For now though it works for what
        // it's used for!
        let mut result = vec![(pkg, None)];
        let first_pkg_depending_on = |pkg, res: &[(&N, Option<&E>)]| {
            self.nodes
                .iter()
                .filter(|(_, adjacent)| adjacent.contains_key(pkg))
                // Note that we can have "cycles" introduced through dev-dependency
                // edges, so make sure we don't loop infinitely.
                .find(|&(node, _)| !res.iter().any(|p| p.0 == node))
                .map(|(p, adjacent)| (p, adjacent.get(pkg)))
        };
        while let Some(p) = first_pkg_depending_on(pkg, &result) {
            result.push(p);
            pkg = p.0;
        }
        result
    }
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
