use std::ops::Range;

use crate::ops::cargo_tree::tui::dependency::{DependencyTree, NodeId};

use super::state::{VirtualPos, VisIdx, VisibleNode};

/// Cached render state for a single view of the dependency tree.
///
/// The widget maintains two [`ViewCache`]s in parallel: one for the normal view
/// and one for the search-filtered view, so toggling search doesn't require
/// re-materializing from scratch.
///
/// Conceptually, the cache straddles two coordinate spaces.
///
/// `subtree_sizes` and `total_virtual_lines` describe the *full virtual
/// stream*: every row that would exist if the tree were flattened end-to-end,
/// indexed by `NodeId`.
///
/// Computing them is the slow pass, but it only re-runs when expansion or
/// filter state changes.
///
/// `nodes` describes the *small materialized window* around the viewport,
/// plus the ancestor chain needed for breadcrumbs and indentation, indexed by
/// `VisIdx`.
///
/// That window is cheap to rebuild and is refilled on every scroll, because
/// [`materialize_window`] uses the precomputed sizes to skip whole subtrees in
/// O(1) and emit only the rows actually on screen.
///
/// # Example
///
/// For a tree like:
///
/// root
/// |- a
/// |  `- c
/// `- b
///
/// the full virtual stream is:
///
///   0: root
///   1: a
///   2: c
///   3: b
///
/// If the viewport wants rows `[1, 3)`, `nodes` only stores the small
/// materialized slice:
///
///   0: root  (ancestor prefix kept for context)
///   1: a
///   2: c
///
/// So `total_virtual_lines` is `4`, while `nodes.len()` is only `3`.
///
/// This is the optimization: keep the full stream virtual, precompute subtree
/// sizes once, and rebuild only the small materialized slice needed for the
/// current viewport.
#[derive(Debug, Default)]
pub(super) struct ViewCache {
    /// Materialized slice of visible nodes covering the current viewport plus
    /// the ancestor prefix needed for lineage rendering.
    pub(super) nodes: Vec<VisibleNode>, // indexed by VisIdx

    /// `NodeId`-indexed memoization of visible-subtree sizes.
    ///
    /// Enables O(1) "skip this whole subtree" checks during materialization.
    ///
    /// Example:
    ///
    /// root
    /// |- a
    /// |  `- many hundreds of rows...
    /// `- b
    ///
    /// If `subtree_sizes[a] = 500` and the viewport starts after those 500
    /// rows, the widget does not need to walk all of `a`'s descendants. It can
    /// skip the entire subtree in O(1) and continue from `b`.
    pub(super) subtree_sizes: Vec<usize>,

    /// Total visible line count across all roots.
    ///
    /// This equals the height of the fully-flattened virtual stream. Used as the scrollbar extent.
    pub(super) total_virtual_lines: usize,
}

impl ViewCache {
    /// Clears all cached data, resetting to empty state.
    pub(super) fn clear(&mut self) {
        self.nodes.clear();
        self.subtree_sizes.clear();
        self.total_virtual_lines = 0;
    }

    /// Recomputes subtree sizes for the given filter.
    ///
    /// Must be called whenever `open` or `filter` changes; pure scrolls can skip
    /// straight to [`ViewCache::rematerialize`].
    pub(super) fn refresh_sizes(
        &mut self,
        tree: &DependencyTree,
        open: &[bool],
        filter: Option<&[bool]>,
    ) {
        self.total_virtual_lines =
            compute_subtree_sizes(tree, open, filter, &mut self.subtree_sizes);
    }

    /// Refills the materialized window using the cache's existing `subtree_sizes`.
    ///
    /// The caller must have invoked [`ViewCache::refresh_sizes`] with the same
    /// `open`/`filter` since the last mutation to either, or the emitted rows
    /// will not match the current view.
    pub(super) fn rematerialize(
        &mut self,
        tree: &DependencyTree,
        open: &[bool],
        filter: Option<&[bool]>,
        roots: &[NodeId],
        window: Range<usize>,
    ) {
        self.nodes = materialize_window(tree, open, &self.subtree_sizes, filter, roots, window);
    }
}

/// One frame on the DFS ancestor stack inside [`MaterializeCtx`].
///
/// We keep ancestors on a stack (rather than re-walking the tree) because every
/// emitted node needs three things from its parent that the raw [`DependencyTree`]
/// doesn't carry: the parent's [`VisIdx`] in the output buffer, the most recently
/// emitted sibling (for `prev/next_sibling` wiring), and the parent's depth.
///
/// `output_idx` is `None` while the DFS is still above the viewport — in that
/// case the ancestor hasn't been emitted yet. It's filled in either when the
/// node itself enters the window, or when [`MaterializeCtx::emit_ancestor_prefix`]
/// flushes the whole ancestor chain on the first in-window emission.
struct Ancestor {
    id: NodeId,
    depth: usize,
    virtual_pos: usize,
    /// Index into `MaterializeCtx::output` once this ancestor has been emitted
    /// (either as part of the prefix or as a normal window node). `None` until
    /// then. Used by children to set their `parent_vis_idx` in O(1).
    output_idx: Option<usize>,
    /// Output index of the most recently emitted child of this ancestor.
    ///
    /// Lets the next emitted child wire `prev_sibling` and back-patch the
    /// previous child's `next_sibling` without scanning `output`.
    last_child_output_idx: Option<usize>,
    /// `NodeId` of this ancestor's last non-group child that passes the filter,
    /// computed once at push time from the *full* child list (not just the
    /// in-window subset). A child compares its own `id` against this to set
    /// its `is_last_non_group_child` flag, so prefix-emitted ancestors and
    /// in-window nodes whose later siblings fall past `window.end` still
    /// render the correct `└─` vs `├─` connector.
    last_non_group_child_id: Option<NodeId>,
}

/// Mutable working state for one [`materialize_window`] call.
///
/// Bundling everything into a struct keeps the recursive helpers
/// ([`materialize_node`], [`emit_node`], [`emit_ancestor_prefix`]) cheap to
/// call. They take `&mut self` instead of a long parameter list, and the
/// shared cycle guard / ancestor stack stay live across the whole DFS.
///
/// [`materialize_node`]: MaterializeCtx::materialize_node
/// [`emit_node`]: MaterializeCtx::emit_node
/// [`emit_ancestor_prefix`]: MaterializeCtx::emit_ancestor_prefix
struct MaterializeCtx<'a> {
    tree: &'a DependencyTree,
    /// Per-`NodeId` expansion state. Closed nodes don't recurse into children and fill one row.
    open: &'a [bool],
    /// Memoized subtree sizes from [`compute_subtree_sizes`]. Lets the DFS
    /// skip entire subtrees that fall before the window in O(1).
    sizes: &'a [usize],
    /// Optional `NodeId` mask for the search-filtered view. `None` means no filter.
    filter: Option<&'a [bool]>,
    /// Running position in the fully-flattened virtual line stream. Advances
    /// once per node visited (or jumps by `subtree_size` when skipping).
    virtual_pos: usize,
    /// Viewport window in virtual-line space (half-open: start inclusive, end exclusive).
    window: Range<usize>,
    /// Stack of ancestors on the current DFS path, deepest at the back.
    /// Drives parent / sibling resolution for emitted nodes.
    ancestor_stack: Vec<Ancestor>,
    /// Cycle guard: `true` for nodes currently on the DFS path. Mirrors
    /// `in_progress` in [`compute_size_recursive`] so back-edges in cyclic
    /// dep graphs (e.g. dev-dep cycles) are emitted as leaves rather than
    /// recursed into. The two passes MUST agree on which edges are leaves,
    /// otherwise sizes and emitted-node counts diverge.
    in_progress: Vec<bool>,
    /// Emitted nodes, in DFS order. Returned from [`materialize_window`].
    output: Vec<VisibleNode>,
}

impl MaterializeCtx<'_> {
    /// Walk one subtree in DFS order and emit rows only if that subtree
    /// overlaps the current viewport window.
    ///
    /// `subtree_sizes` lets this fast-path whole branches that fall entirely
    /// before the window, while `ancestor_stack` carries the parent/sibling
    /// context needed when a row is actually emitted.
    fn materialize_node(&mut self, id: NodeId, depth: usize, parent_ancestor_idx: Option<usize>) {
        // Filtered-out nodes don't exist in the virtual stream — don't advance.
        if self.filter.is_some_and(|f| !f[id.0]) {
            return;
        }

        let current_vpos = self.virtual_pos;
        let subtree_size = self.sizes[id.0];

        // Entirely before window — skip subtree
        if current_vpos + subtree_size <= self.window.start {
            self.virtual_pos += subtree_size;
            return;
        }

        // Entirely past window — stop
        if current_vpos >= self.window.end {
            return;
        }

        // This node is in or overlaps the window.
        self.virtual_pos += 1;

        let in_window = current_vpos >= self.window.start;
        if in_window {
            // Flush the ancestor prefix on the first in-window emission.
            if self.output.is_empty() {
                self.emit_ancestor_prefix();
            }
            self.emit_node(id, depth, current_vpos, parent_ancestor_idx);
        }

        // Recurse into children if open. Skip recursion on a back-edge;
        // a node already on the current DFS path is a cycle, and the size
        // accounting in `compute_size_recursive` treats it as a leaf.
        if self.open[id.0]
            && !self.in_progress[id.0]
            && let Some(node) = self.tree.node(id)
        {
            let my_ancestor_idx = self.ancestor_stack.len();
            // If this node was emitted, child sibling-linking will resolve
            // its output_idx via `ancestor_stack[my_ancestor_idx].output_idx`.
            let output_idx = if in_window {
                Some(self.output.len() - 1)
            } else {
                None
            };
            let last_non_group_child_id = self.last_non_group_child_of(node);
            self.ancestor_stack.push(Ancestor {
                id,
                depth,
                virtual_pos: current_vpos,
                output_idx,
                last_child_output_idx: None,
                last_non_group_child_id,
            });
            self.in_progress[id.0] = true;

            for &child in node.children() {
                if self.virtual_pos >= self.window.end {
                    break;
                }
                self.materialize_node(child, depth + 1, Some(my_ancestor_idx));
            }

            self.in_progress[id.0] = false;
            self.ancestor_stack.pop();
        }
    }

    /// Pushes a node into `output` and wires up parent / sibling metadata.
    fn emit_node(
        &mut self,
        id: NodeId,
        depth: usize,
        my_vpos: usize,
        parent_ancestor_idx: Option<usize>,
    ) {
        let my_output_idx = self.output.len();
        let my_vis_idx = VisIdx(my_output_idx);

        // Resolve parent + wire sibling links from the parent's last child.
        let (parent_vis_idx, prev_sibling, is_last_non_group_child) = match parent_ancestor_idx {
            Some(idx) => {
                let parent = &mut self.ancestor_stack[idx];
                let parent_output_idx = parent
                    .output_idx
                    .expect("parent ancestor must be emitted before its in-window child");
                let prev = parent.last_child_output_idx.map(VisIdx);
                parent.last_child_output_idx = Some(my_output_idx);
                let is_last = parent.last_non_group_child_id == Some(id);
                (Some(VisIdx(parent_output_idx)), prev, is_last)
            }
            None => (None, None, true),
        };

        if let Some(prev) = prev_sibling {
            self.output[prev.0].next_sibling = Some(my_vis_idx);
        }

        self.output.push(VisibleNode {
            id,
            depth,
            virtual_pos: VirtualPos(my_vpos),
            parent_vis_idx,
            next_sibling: None,
            prev_sibling,
            is_last_non_group_child,
        });
    }

    /// Returns the `NodeId` of `parent`'s last non-group child that passes the
    /// filter, or `None` if it has no such child. Walks the full child list
    /// (not just the in-window subset), so the result is stable across
    /// scrolling and reflects the true visible tree.
    fn last_non_group_child_of(
        &self,
        parent: &crate::ops::cargo_tree::tui::dependency::DependencyNode,
    ) -> Option<NodeId> {
        parent.children().iter().rev().copied().find(|&c| {
            self.filter.is_none_or(|f| f[c.0]) && self.tree.node(c).is_some_and(|n| !n.is_group())
        })
    }

    /// Emits ancestor prefix nodes for lineage/breadcrumb rendering. The prefix
    /// is a single chain (each ancestor is the only emitted child of the one
    /// above it), so sibling links stay None and parent links walk the chain.
    fn emit_ancestor_prefix(&mut self) {
        for i in 0..self.ancestor_stack.len() {
            let ancestor = &self.ancestor_stack[i];
            let id = ancestor.id;
            let depth = ancestor.depth;
            let vpos = ancestor.virtual_pos;

            let (parent_vis_idx, is_last_non_group_child) = if i == 0 {
                (None, true)
            } else {
                let parent = &self.ancestor_stack[i - 1];
                (
                    parent.output_idx.map(VisIdx),
                    parent.last_non_group_child_id == Some(id),
                )
            };

            let my_output_idx = self.output.len();
            self.output.push(VisibleNode {
                id,
                depth,
                virtual_pos: VirtualPos(vpos),
                parent_vis_idx,
                next_sibling: None,
                prev_sibling: None,
                is_last_non_group_child,
            });
            self.ancestor_stack[i].output_idx = Some(my_output_idx);
            self.ancestor_stack[i].last_child_output_idx = None;
            if i > 0 {
                self.ancestor_stack[i - 1].last_child_output_idx = Some(my_output_idx);
            }
        }
    }
}

/// Build the small visible slice of the tree for the current viewport.
///
/// The result starts with any ancestor rows needed for context rendering, then
/// contains the nodes whose virtual positions fall inside `window`.
///
/// Parent, sibling, and "last child" metadata are filled in as the rows are
/// emitted, so the renderer can use the result directly.
///
/// # Parameters
///
/// - `tree`: the arena being walked. Read-only; only its node/children
///   accessors are used.
/// - `open`: per-`NodeId` expansion state. A closed node is emitted but its
///   children are skipped.
/// - `sizes`: precomputed visible-subtree sizes from [`compute_subtree_sizes`].
///   Must have been built with the same `open` and `filter` as this call,
///   otherwise the skip-subtree fast path emits the wrong rows. This is the
///   table that makes the walk O(window) instead of O(tree).
/// - `filter`: optional `NodeId` mask for the search-filtered view; `None`
///   means no filter. Filtered-out nodes are treated as if they didn't exist
///   (skipped without advancing `virtual_pos`).
/// - `roots`: the top-level nodes to walk, in order. Typically `tree.roots()`.
/// - `window`: viewport range in virtual-line coordinates (start inclusive,
///   end exclusive; 0 = first line of the flattened tree).
fn materialize_window(
    tree: &DependencyTree,
    open: &[bool],
    sizes: &[usize],
    filter: Option<&[bool]>,
    roots: &[NodeId],
    window: Range<usize>,
) -> Vec<VisibleNode> {
    let cap = window.len() + 64;
    let mut ctx = MaterializeCtx {
        tree,
        open,
        sizes,
        filter,
        virtual_pos: 0,
        window,
        ancestor_stack: Vec::with_capacity(64),
        in_progress: vec![false; tree.nodes.len()],
        output: Vec::with_capacity(cap),
    };

    for &root in roots {
        if ctx.virtual_pos >= ctx.window.end {
            break;
        }
        ctx.materialize_node(root, 0, None);
    }

    ctx.output
}

/// Computes memoized visible-subtree sizes for all nodes.
fn compute_subtree_sizes(
    tree: &DependencyTree,
    open: &[bool],
    filter: Option<&[bool]>,
    sizes: &mut Vec<usize>,
) -> usize {
    sizes.clear();
    sizes.resize(tree.nodes.len(), 0);
    // prevents recomputing already-visited nodes
    let mut computed = vec![false; tree.nodes.len()];
    // avoid infinite graphs by breaking hypothetical cycles;
    // in-progress nodes are treated as leaves to avoid infinite recursion
    let mut in_progress = vec![false; tree.nodes.len()];

    let mut total = 0usize;
    for &root in tree.roots() {
        if filter.is_some_and(|f| !f[root.0]) {
            continue;
        }
        total += compute_size_recursive(
            tree,
            open,
            filter,
            root,
            sizes,
            &mut computed,
            &mut in_progress,
        );
    }
    total
}

fn compute_size_recursive(
    tree: &DependencyTree,
    open: &[bool],
    filter: Option<&[bool]>,
    id: NodeId,
    sizes: &mut [usize],
    computed: &mut [bool],
    in_progress: &mut [bool],
) -> usize {
    if in_progress[id.0] {
        return 1; // cycle break
    }
    if computed[id.0] {
        // Shared subtree: reuse the size already computed from another parent.
        return sizes[id.0];
    }

    in_progress[id.0] = true;

    // Every visible node contributes at least one row for itself.
    let mut size: usize = 1;
    if open[id.0]
        && let Some(node) = tree.node(id)
    {
        // Open nodes contribute the sizes of all visible children.
        for &child in node.children() {
            if filter.is_some_and(|f| !f[child.0]) {
                continue;
            }
            size += compute_size_recursive(tree, open, filter, child, sizes, computed, in_progress);
        }
    }

    sizes[id.0] = size;
    computed[id.0] = true;
    in_progress[id.0] = false;
    size
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::cargo_tree::tui::dependency::{Dependency, DependencyNode, DependencyTree};

    /// Builds an arena tree from a slice of `(name, children)` tuples.
    /// Node ids are positional; the first entry is the sole root.
    fn build(spec: &[(&str, &[usize])]) -> DependencyTree {
        let nodes: Vec<DependencyNode> = spec
            .iter()
            .map(|(name, children)| {
                DependencyNode::Crate(Dependency {
                    name: String::from(*name),
                    version: String::from("0.0.0"),
                    package_id: None,
                    manifest_dir: None,
                    is_proc_macro: false,
                    format_metadata: None,
                    children: children.iter().copied().map(NodeId).collect(),
                })
            })
            .collect();

        let mut parents: Vec<Vec<NodeId>> = vec![Vec::new(); nodes.len()];
        for (idx, node) in nodes.iter().enumerate() {
            for &child in node.children() {
                parents[child.0].push(NodeId(idx));
            }
        }

        DependencyTree {
            workspace_name: String::from("test"),
            nodes,
            parents,
            dependency_usages: Default::default(),
            roots: vec![NodeId(0)],
        }
    }

    /// Standard 6-node fixture:
    /// ```text
    /// root            (id 0)
    /// ├── a           (id 1)
    /// │   ├── aa      (id 2)
    /// │   └── ab      (id 3)
    /// └── b           (id 4)
    ///     └── bb      (id 5)
    /// ```
    /// Virtual positions when fully open: root=0, a=1, aa=2, ab=3, b=4, bb=5.
    fn fixture() -> DependencyTree {
        build(&[
            ("root", &[1, 4]),
            ("a", &[2, 3]),
            ("aa", &[]),
            ("ab", &[]),
            ("b", &[5]),
            ("bb", &[]),
        ])
    }

    fn all_open(tree: &DependencyTree) -> Vec<bool> {
        vec![true; tree.nodes.len()]
    }

    fn materialize(
        tree: &DependencyTree,
        open: &[bool],
        start: usize,
        count: usize,
    ) -> (Vec<usize>, Vec<VisibleNode>) {
        let mut cache = ViewCache::default();
        cache.refresh_sizes(tree, open, None);
        cache.rematerialize(tree, open, None, tree.roots(), start..start + count);
        let root_sum: usize = tree.roots().iter().map(|r| cache.subtree_sizes[r.0]).sum();
        assert_eq!(cache.total_virtual_lines, root_sum);
        (cache.subtree_sizes, cache.nodes)
    }

    #[test]
    fn subtree_sizes_all_open() {
        let tree = fixture();
        let mut sizes = Vec::new();
        let total = compute_subtree_sizes(&tree, &all_open(&tree), None, &mut sizes);
        assert_eq!(sizes, vec![6, 3, 1, 1, 2, 1]);
        assert_eq!(total, 6);
    }

    #[test]
    fn subtree_sizes_with_closed_node() {
        let tree = fixture();
        let mut open = all_open(&tree);
        open[1] = false;
        // close `a`:
        //
        // root
        // |- a
        // `- b
        //    `- bb
        let mut sizes = Vec::new();
        let total = compute_subtree_sizes(&tree, &open, None, &mut sizes);
        assert_eq!(sizes[1], 1);
        assert_eq!(sizes[0], 4); // root, a, b, bb
        assert_eq!(total, 4);
    }

    #[test]
    fn subtree_sizes_breaks_cycle() {
        // cycle:
        //
        // root
        // `- a
        //    `- b
        //       `- a   (back-edge, counted as a leaf)
        let tree = build(&[("root", &[1]), ("a", &[2]), ("b", &[1])]);
        let mut sizes = Vec::new();
        let total = compute_subtree_sizes(&tree, &all_open(&tree), None, &mut sizes);
        // sizes:
        //
        // a(back-edge leaf) = 1
        // b                = 1 + 1 = 2
        // a                = 1 + 2 = 3
        // root             = 1 + 3 = 4
        assert_eq!(sizes[0], 4);
        assert_eq!(total, 4);
    }

    #[test]
    fn subtree_sizes_with_filter() {
        let tree = fixture();
        // filter out `a` and its descendants:
        //
        // root
        // `- b
        //    `- bb
        let filter = vec![true, false, false, false, true, true];
        let mut sizes = Vec::new();
        let total = compute_subtree_sizes(&tree, &all_open(&tree), Some(&filter), &mut sizes);
        // root keeps only the `b` subtree: 1 + 2 = 3
        assert_eq!(sizes[0], 3);
        assert_eq!(total, 3);
    }

    #[test]
    fn materialize_full_tree() {
        let tree = fixture();
        let (_, nodes) = materialize(&tree, &all_open(&tree), 0, 6);
        assert_eq!(nodes.len(), 6);
        let ids: Vec<usize> = nodes.iter().map(|n| n.id.0).collect();
        assert_eq!(ids, vec![0, 1, 2, 3, 4, 5]);
        let depths: Vec<usize> = nodes.iter().map(|n| n.depth).collect();
        assert_eq!(depths, vec![0, 1, 2, 2, 1, 2]);
        let vpos: Vec<usize> = nodes.iter().map(|n| n.virtual_pos.0).collect();
        assert_eq!(vpos, vec![0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn materialize_window_in_middle_emits_ancestor_prefix() {
        let tree = fixture();
        // full tree:
        //
        // root
        // |- a
        // |  |- aa
        // |  `- ab
        // `- b
        //    `- bb
        //
        // window [3, 5):
        //
        // ab
        // b
        let (_, nodes) = materialize(&tree, &all_open(&tree), 3, 2);
        // emitted rows:
        //
        // root
        // `- a
        //    `- ab
        // b
        let ids: Vec<usize> = nodes.iter().map(|n| n.id.0).collect();
        assert_eq!(ids, vec![0, 1, 3, 4]);
        let depths: Vec<usize> = nodes.iter().map(|n| n.depth).collect();
        assert_eq!(depths, vec![0, 1, 2, 1]);

        // ab's parent is `a` at vis idx 1; b's parent is root at vis idx 0.
        assert_eq!(nodes[2].parent_vis_idx, Some(VisIdx(1)));
        assert_eq!(nodes[3].parent_vis_idx, Some(VisIdx(0)));
        assert_eq!(nodes[0].parent_vis_idx, None);
    }

    #[test]
    fn materialize_window_at_deepest_node_emits_full_prefix() {
        let tree = fixture();
        // full tree:
        //
        // root
        // |- a
        // |  |- aa
        // |  `- ab
        // `- b
        //    `- bb
        //
        // window [5, 6):
        //
        // bb
        let (_, nodes) = materialize(&tree, &all_open(&tree), 5, 1);
        // emitted rows:
        //
        // root
        // `- b
        //    `- bb
        let ids: Vec<usize> = nodes.iter().map(|n| n.id.0).collect();
        assert_eq!(ids, vec![0, 4, 5]);
        let depths: Vec<usize> = nodes.iter().map(|n| n.depth).collect();
        assert_eq!(depths, vec![0, 1, 2]);
        assert_eq!(nodes[0].parent_vis_idx, None);
        assert_eq!(nodes[1].parent_vis_idx, Some(VisIdx(0)));
        assert_eq!(nodes[2].parent_vis_idx, Some(VisIdx(1)));
    }

    #[test]
    fn materialize_window_past_end_is_empty() {
        let tree = fixture();
        let (_, nodes) = materialize(&tree, &all_open(&tree), 100, 10);
        assert!(nodes.is_empty());
    }

    #[test]
    fn materialize_window_larger_than_tree() {
        let tree = fixture();
        let (_, nodes) = materialize(&tree, &all_open(&tree), 0, 1000);
        assert_eq!(nodes.len(), 6);
    }

    #[test]
    fn materialize_with_closed_node_skips_subtree() {
        let tree = fixture();
        let mut open = all_open(&tree);
        open[1] = false;
        // close `a`:
        //
        // root
        // |- a
        // `- b
        //    `- bb
        let (_, nodes) = materialize(&tree, &open, 0, 10);
        let ids: Vec<usize> = nodes.iter().map(|n| n.id.0).collect();
        assert_eq!(ids, vec![0, 1, 4, 5]);
        let vpos: Vec<usize> = nodes.iter().map(|n| n.virtual_pos.0).collect();
        assert_eq!(vpos, vec![0, 1, 2, 3]);
    }

    #[test]
    fn materialize_with_cycle_treats_back_edge_as_leaf() {
        // cycle:
        //
        // root
        // `- a
        //    `- b
        //       `- a   (back-edge)
        //
        // The back-edge node is emitted as a leaf at the deeper depth, but its
        // children are not recursed into. This matches `compute_subtree_sizes`,
        // which also counts the back-edge as 1.
        let tree = build(&[("root", &[1]), ("a", &[2]), ("b", &[1])]);
        let (sizes, nodes) = materialize(&tree, &all_open(&tree), 0, 100);
        assert_eq!(nodes.len(), sizes[0]); // size/materialization parity
        let ids: Vec<usize> = nodes.iter().map(|n| n.id.0).collect();
        assert_eq!(ids, vec![0, 1, 2, 1]); // root, a, b, a(back-edge leaf)
        let depths: Vec<usize> = nodes.iter().map(|n| n.depth).collect();
        assert_eq!(depths, vec![0, 1, 2, 3]);
    }

    #[test]
    fn materialize_with_filter_excludes_subtree() {
        let tree = fixture();
        let filter = vec![true, false, false, false, true, true];
        let mut cache = ViewCache::default();
        cache.refresh_sizes(&tree, &all_open(&tree), Some(&filter));
        cache.rematerialize(&tree, &all_open(&tree), Some(&filter), tree.roots(), 0..10);
        let ids: Vec<usize> = cache.nodes.iter().map(|n| n.id.0).collect();
        assert_eq!(ids, vec![0, 4, 5]);
    }

    fn build_cache(tree: &DependencyTree) -> ViewCache {
        let mut cache = ViewCache::default();
        cache.refresh_sizes(tree, &all_open(tree), None);
        cache.rematerialize(
            tree,
            &all_open(tree),
            None,
            tree.roots(),
            0..tree.nodes.len(),
        );
        cache
    }

    #[test]
    fn sibling_links_round_trip() {
        let tree = fixture();
        let cache = build_cache(&tree);
        let n = &cache.nodes;

        // Sibling pairs after full materialization (by VisIdx):
        //   1: a  ↔ 4: b      (children of root)
        //   2: aa ↔ 3: ab     (children of a)
        assert_eq!(n[1].next_sibling, Some(VisIdx(4)));
        assert_eq!(n[4].prev_sibling, Some(VisIdx(1)));
        assert_eq!(n[2].next_sibling, Some(VisIdx(3)));
        assert_eq!(n[3].prev_sibling, Some(VisIdx(2)));
        assert_eq!(n[0].next_sibling, None);
        assert_eq!(n[0].prev_sibling, None);
        assert_eq!(n[5].next_sibling, None);

        // Round-trip every link.
        for (i, node) in n.iter().enumerate() {
            if let Some(next) = node.next_sibling {
                assert_eq!(n[next.0].prev_sibling, Some(VisIdx(i)));
            }
            if let Some(prev) = node.prev_sibling {
                assert_eq!(n[prev.0].next_sibling, Some(VisIdx(i)));
            }
        }
    }

    #[test]
    fn is_last_non_group_child_full_tree() {
        let tree = fixture();
        let cache = build_cache(&tree);
        let n = &cache.nodes;
        // full tree:
        //
        // root
        // |- a
        // |  |- aa
        // |  `- ab
        // `- b
        //    `- bb
        //
        // last non-group children:
        //
        // root -> b
        // a    -> ab
        // b    -> bb
        assert!(n[0].is_last_non_group_child);
        assert!(!n[1].is_last_non_group_child);
        assert!(!n[2].is_last_non_group_child);
        assert!(n[3].is_last_non_group_child);
        assert!(n[4].is_last_non_group_child);
        assert!(n[5].is_last_non_group_child);
    }

    #[test]
    fn is_last_non_group_child_reflects_full_tree_not_window() {
        // full tree:
        //
        // root
        // `- a
        //    |- aa   <- in window
        //    `- ab   <- outside window, but still the real last child
        let tree = fixture();
        let (_, nodes) = materialize(&tree, &all_open(&tree), 2, 1);
        // emitted rows:
        //
        // root
        // `- a
        //    `- aa
        let aa = nodes.iter().find(|n| n.id.0 == 2).unwrap();
        assert!(!aa.is_last_non_group_child);
        // The prefix-emitted `a` is also not last because root still has `b`.
        let a = nodes.iter().find(|n| n.id.0 == 1).unwrap();
        assert!(!a.is_last_non_group_child);
    }
}
