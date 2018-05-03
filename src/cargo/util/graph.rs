use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::Hash;

pub struct Graph<N, E> {
    nodes: HashMap<N, HashMap<N, E>>,
}

impl<N: Eq + Hash + Clone, E: Default> Graph<N, E> {
    pub fn new() -> Graph<N, E> {
        Graph {
            nodes: HashMap::new(),
        }
    }

    pub fn add(&mut self, node: N) {
        self.nodes.entry(node).or_insert_with(HashMap::new);
    }

    pub fn link(&mut self, node: N, child: N) -> &mut E {
        self.nodes
            .entry(node)
            .or_insert_with(HashMap::new)
            .entry(child)
            .or_insert_with(Default::default)
    }

    pub fn contains<Q: ?Sized>(&self, k: &Q) -> bool
    where
        N: Borrow<Q>,
        Q: Hash + Eq,
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

        for child in self.nodes[node].keys() {
            self.sort_inner_visit(child, dst, marks);
        }

        dst.push(node.clone());
    }

    pub fn iter(&self) -> impl Iterator<Item = &N> {
        self.nodes.keys()
    }

    /// Resolves one of the paths from the given dependent package up to
    /// the root.
    pub fn path_to_top<'s: 'q, 'q>(&'s self, mut pkg: &'q N) -> Vec<(&'s N, &'s E)> {
        // Note that this implementation isn't the most robust per se, we'll
        // likely have to tweak this over time. For now though it works for what
        // it's used for!
        let mut result: Vec<(&'s N, &'s E)> = vec![];
        let first_pkg_depending_on = |pkg: &N, res: &[(&'s N, &'s E)]| {
            self.nodes
                .iter()
                .filter(|&(_, adjacent)| adjacent.contains_key(pkg))
                // Note that we can have "cycles" introduced through dev-dependency
                // edges, so make sure we don't loop infinitely.
                .find(|&(node, _)| res.iter().find(|p| p.0 == node).is_none())
                // TODO: find_map would be clearer
                .map(|(node, adjacent)| (node, adjacent.get(pkg).unwrap()))
        };
        while let Some(p) = first_pkg_depending_on(pkg, &result) {
            result.push(p);
            pkg = p.0;
        }
        result
    }
}

impl<N: Eq + Hash + Clone, E: Default> Default for Graph<N, E> {
    fn default() -> Graph<N, E> {
        Graph::new()
    }
}

impl<N: fmt::Display + Eq + Hash, E> fmt::Debug for Graph<N, E> {
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

impl<N: Eq + Hash, E: Eq> PartialEq for Graph<N, E> {
    fn eq(&self, other: &Graph<N, E>) -> bool {
        self.nodes.eq(&other.nodes)
    }
}
impl<N: Eq + Hash, E: Eq> Eq for Graph<N, E> {}

impl<N: Eq + Hash + Clone, E: Clone> Clone for Graph<N, E> {
    fn clone(&self) -> Graph<N, E> {
        Graph {
            nodes: self.nodes.clone(),
        }
    }
}
