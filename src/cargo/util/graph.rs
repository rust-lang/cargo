use collections::HashMap;

trait Node<'a, I: Iterator<&'a Self>> {
    fn children(&'a self) -> I;
}

trait Graph<'a, N: Node<'a, I>, I: Iterator<&'a N>> {
    fn nodes(&'a self) -> I;
}

#[deriving(Clone)]
enum Mark {
    InProgress,
    Done
}

/**
 * Returns None in the event of a cycle
 */
pub fn topsort<'a, N: Node<'a, I>, G: Graph<'a, N, I>, I: Iterator<&'a N>>(graph: &'a G) -> Option<Vec<&'a N>> {
    let mut ret = Vec::new();
    let mut iter: I = graph.nodes();
    let mut stack = Vec::<&'a N>::new();
    let mut marks: HashMap<*N, Mark> = HashMap::new();

    // Prime the stack
    for node in iter {
        visit(node, &mut ret, &mut marks);
    }

    Some(ret)
}

fn visit<'a, N: Node<'a, I>, I: Iterator<&'a N>>(curr: &'a N, dst: &mut Vec<&'a N>, marks: &mut HashMap<*N, Mark>) {
    let ident = curr as *N;

    if marks.contains_key(&ident) {
        return;
    }

    marks.insert(ident, InProgress);

    let mut iter: I = curr.children();

    for child in iter {
        visit::<'a, N, I>(child, dst, marks);
    }

    dst.push(curr);
    marks.insert(ident, Done);
}

#[cfg(test)]
mod test {
    // TODO: tests
}
