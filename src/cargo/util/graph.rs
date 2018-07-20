use std::fmt;
use std::hash::Hash;
use std::collections::hash_map::{HashMap, Iter, Keys};

pub struct Graph<N, E> {
    nodes: HashMap<N, HashMap<N, E>>,
}

enum Mark {
    InProgress,
    Done,
}

pub type Nodes<'a, N, E> = Keys<'a, N, HashMap<N, E>>;
pub type Edges<'a, N, E> = Iter<'a, N, E>;

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

    pub fn edge(&self, from: &N, to: &N) -> Option<&E> {
        self.nodes.get(from)?.get(to)
    }

    pub fn edges(&self, from: &N) -> Option<Edges<N, E>> {
        self.nodes.get(from).map(|set| set.iter())
    }

    pub fn sort(&self) -> Option<Vec<N>> {
        let mut ret = Vec::new();
        let mut marks = HashMap::new();

        for node in self.nodes.keys() {
            self.visit(node, &mut ret, &mut marks);
        }

        Some(ret)
    }

    fn visit(&self, node: &N, dst: &mut Vec<N>, marks: &mut HashMap<N, Mark>) {
        if marks.contains_key(node) {
            return;
        }

        marks.insert(node.clone(), Mark::InProgress);

        for child in self.nodes[node].keys() {
            self.visit(child, dst, marks);
        }

        dst.push(node.clone());
        marks.insert(node.clone(), Mark::Done);
    }

    pub fn iter(&self) -> Nodes<N, E> {
        self.nodes.keys()
    }

    /// Resolves one of the paths from the given dependent package up to
    /// the root.
    pub fn path_to_top<'a>(&'a self, mut pkg: &'a N) -> Vec<&'a N> {
        // Note that this implementation isn't the most robust per se, we'll
        // likely have to tweak this over time. For now though it works for what
        // it's used for!
        let mut result = vec![pkg];
        let first_pkg_depending_on = |pkg: &N, res: &[&N]| {
            self.nodes
                .iter()
                .filter(|&(_node, adjacent)| adjacent.contains_key(pkg))
                // Note that we can have "cycles" introduced through dev-dependency
                // edges, so make sure we don't loop infinitely.
                .find(|&(node, _)| !res.contains(&node))
                .map(|p| p.0)
        };
        while let Some(p) = first_pkg_depending_on(pkg, &result) {
            result.push(p);
            pkg = p;
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
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
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
