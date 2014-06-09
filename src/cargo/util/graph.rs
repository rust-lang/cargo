use std::hash::Hash;
use std::collections::HashMap;

pub struct Graph<N> {
    nodes: HashMap<N, ~[N]>
}

enum Mark {
    InProgress,
    Done
}

impl<N: Eq + Hash + Clone> Graph<N> {
    pub fn new() -> Graph<N> {
        Graph { nodes: HashMap::new() }
    }

    pub fn add(&mut self, node: N, children: &[N]) {
        self.nodes.insert(node, children.to_owned());
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

        for child in self.nodes.get(node).iter() {
            self.visit(child, dst, marks);
        }

        dst.push(node.clone());
        marks.insert(node.clone(), Done);
    }
}
