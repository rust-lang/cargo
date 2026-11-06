use std::ops::Deref;

use rustc_hash::FxHashSet;

use crate::ops::cargo_tree::tui::dependency::{DependencyNode, DependencyTree, NodeId};

/// TUI-owned dependency tree with Cargo-style duplicate display state.
#[derive(Debug)]
pub struct TuiTree {
    tree: DependencyTree,
    duplicates: FxHashSet<NodeId>,
}

impl TuiTree {
    pub fn new(mut tree: DependencyTree, no_dedupe: bool) -> Self {
        let duplicates = if no_dedupe {
            FxHashSet::default()
        } else {
            deduplicate_occurrences(&mut tree)
        };
        Self { tree, duplicates }
    }

    pub fn is_duplicate(&self, id: NodeId) -> bool {
        self.duplicates.contains(&id)
    }
}

impl Deref for TuiTree {
    type Target = DependencyTree;

    fn deref(&self) -> &Self::Target {
        &self.tree
    }
}

/// Marks repeated package occurrences as leaves, preserving Cargo's default
/// tree behavior. The iterative walk avoids recursion on deep dependency chains.
fn deduplicate_occurrences(tree: &mut DependencyTree) -> FxHashSet<NodeId> {
    struct Frame {
        id: NodeId,
        source_children: Vec<NodeId>,
        next_child: usize,
    }

    fn enter(tree: &mut DependencyTree, id: NodeId) -> Frame {
        let source_children = match &mut tree.nodes[id.0] {
            DependencyNode::Crate(dependency) => std::mem::take(&mut dependency.children),
            DependencyNode::Group(group) => std::mem::take(&mut group.children),
            DependencyNode::Feature(feature) => std::mem::take(&mut feature.children),
        };
        Frame {
            id,
            source_children,
            next_child: 0,
        }
    }

    let roots = std::mem::take(&mut tree.roots);
    let mut display_roots = Vec::with_capacity(roots.len());
    let mut visited = FxHashSet::default();
    let mut visited_features = FxHashSet::default();
    let mut active = FxHashSet::default();
    let mut duplicates = FxHashSet::default();
    let mut frames = Vec::new();

    for root_id in roots {
        if tree.nodes[root_id.0].is_crate() && !visited.insert(root_id) {
            let has_children = !tree.nodes[root_id.0].children().is_empty();
            let duplicate = clone_duplicate(tree, root_id);
            if has_children {
                duplicates.insert(duplicate);
            }
            display_roots.push(duplicate);
            continue;
        }

        display_roots.push(root_id);
        active.insert(root_id);
        frames.push(enter(tree, root_id));
        while !frames.is_empty() {
            let child = {
                let frame = frames.last_mut().expect("frame stack is non-empty");
                let child = frame.source_children.get(frame.next_child).copied();
                frame.next_child += usize::from(child.is_some());
                child
            };

            let Some(child_id) = child else {
                let frame = frames.pop().expect("frame stack is non-empty");
                active.remove(&frame.id);
                match &mut tree.nodes[frame.id.0] {
                    DependencyNode::Crate(dependency) => {
                        dependency.children = frame.source_children
                    }
                    DependencyNode::Group(group) => group.children = frame.source_children,
                    DependencyNode::Feature(feature) => feature.children = frame.source_children,
                }
                continue;
            };

            if tree.nodes[child_id.0].is_crate() && visited.insert(child_id) {
                active.insert(child_id);
                frames.push(enter(tree, child_id));
            } else if !tree.nodes[child_id.0].is_crate() {
                if active.contains(&child_id) {
                    let has_children = true;
                    let duplicate = clone_duplicate(tree, child_id);
                    if has_children && matches!(&tree.nodes[child_id.0], DependencyNode::Feature(_))
                    {
                        duplicates.insert(duplicate);
                    }
                    let parent = frames.last_mut().expect("parent frame is non-empty");
                    parent.source_children[parent.next_child - 1] = duplicate;
                } else if tree.nodes[child_id.0].is_group() || visited_features.insert(child_id) {
                    active.insert(child_id);
                    frames.push(enter(tree, child_id));
                } else {
                    let has_children = !tree.nodes[child_id.0].children().is_empty();
                    let duplicate = clone_duplicate(tree, child_id);
                    if has_children && matches!(&tree.nodes[child_id.0], DependencyNode::Feature(_))
                    {
                        duplicates.insert(duplicate);
                    }
                    let parent = frames.last_mut().expect("parent frame is non-empty");
                    parent.source_children[parent.next_child - 1] = duplicate;
                }
            } else {
                let has_children =
                    active.contains(&child_id) || !tree.nodes[child_id.0].children().is_empty();
                let duplicate = clone_duplicate(tree, child_id);
                if has_children {
                    duplicates.insert(duplicate);
                }
                let parent = frames.last_mut().expect("parent frame is non-empty");
                parent.source_children[parent.next_child - 1] = duplicate;
            }
        }
    }

    tree.roots = display_roots;
    tree.parents.resize_with(tree.nodes.len(), Vec::new);
    for parents in &mut tree.parents {
        parents.clear();
    }
    for (parent_idx, node) in tree.nodes.iter().enumerate() {
        for &child_id in node.children() {
            tree.parents[child_id.0].push(NodeId(parent_idx));
        }
    }
    duplicates
}

fn clone_duplicate(tree: &mut DependencyTree, source_id: NodeId) -> NodeId {
    let mut duplicate = tree.nodes[source_id.0].clone();
    match &mut duplicate {
        DependencyNode::Crate(dependency) => dependency.children.clear(),
        DependencyNode::Group(group) => group.children.clear(),
        DependencyNode::Feature(feature) => feature.children.clear(),
    }
    let duplicate_id = NodeId(tree.nodes.len());
    tree.nodes.push(duplicate);
    duplicate_id
}
