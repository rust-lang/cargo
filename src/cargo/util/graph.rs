use std::fmt;
use std::hash::Hash;
use std::collections::{HashMap, HashSet};
use std::collections::hashmap::{Keys, Occupied, SetItems, Vacant};

pub struct Graph<N> {
    nodes: HashMap<N, HashSet<N>>
}

enum Mark {
    InProgress,
    Done
}

pub type Nodes<'a, N> = Keys<'a, N, HashSet<N>>;
pub type Edges<'a, N> = SetItems<'a, N>;

impl<N: Eq + Hash + Clone> Graph<N> {
    pub fn new() -> Graph<N> {
        Graph { nodes: HashMap::new() }
    }

    pub fn add(&mut self, node: N, children: &[N]) {
        self.nodes.insert(node, children.iter().map(|n| n.clone()).collect());
    }

    pub fn link(&mut self, node: N, child: N) {
        match self.nodes.entry(node) {
            Occupied(entry) => entry.into_mut(),
            Vacant(entry) => entry.set(HashSet::new()),
        }.insert(child);
    }

    pub fn get_nodes(&self) -> &HashMap<N, HashSet<N>> {
        &self.nodes
    }

    pub fn edges(&self, node: &N) -> Option<Edges<N>> {
        self.nodes.find(node).map(|set| set.iter())
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

        marks.insert(node.clone(), InProgress);

        for child in self.nodes[*node].iter() {
            self.visit(child, dst, marks);
        }

        dst.push(node.clone());
        marks.insert(node.clone(), Done);
    }

    pub fn iter(&self) -> Nodes<N> {
        self.nodes.keys()
    }
}

impl<N: fmt::Show + Eq + Hash> fmt::Show for Graph<N> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        try!(writeln!(fmt, "Graph {{"));

        for (n, e) in self.nodes.iter() {
            try!(writeln!(fmt, "  - {}", n));

            for n in e.iter() {
                try!(writeln!(fmt, "    - {}", n));
            }
        }

        try!(write!(fmt, "}}"));

        Ok(())
    }
}

impl<N: Eq + Hash> PartialEq for Graph<N> {
    fn eq(&self, other: &Graph<N>) -> bool { self.nodes.eq(&other.nodes) }
}
impl<N: Eq + Hash> Eq for Graph<N> {}

impl<N: Eq + Hash + Clone> Clone for Graph<N> {
    fn clone(&self) -> Graph<N> {
        Graph { nodes: self.nodes.clone() }
    }
}
