use rustc_hash::FxHashSet;

use crate::ops::cargo_tree::tui::dependency::{DependencyNode, DependencyTree, NodeId};

use super::view_cache::ViewCache;
use super::viewport::{ScrollAlignment, Viewport};

/// The widget uses three different index spaces:
///
/// - [`NodeId`] identifies a node in the arena-backed dependency graph.
/// - [`VirtualPos`] identifies a row in the fully flattened virtual tree for
///   the current `(open, filter)` state.
/// - [`VisIdx`] identifies a row inside the small materialized window stored in
///   the active [`ViewCache`].
///
/// Keeping these separate avoids mixing graph identity, virtual scroll
/// position, and cache-local row indices.
/// Index into a flattened visible-node cache.
///
/// Distinguishes visible-cache positions from [`NodeId`] at the type level
/// so the two kinds of `usize` index cannot be accidentally mixed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VisIdx(pub usize);

/// Position in the full virtual (fully-flattened) tree under the current
/// `(open, filter)` configuration.
///
/// Distinct from [`VisIdx`] (which indexes the *materialized* window) and
/// from [`NodeId`] (which identifies a node in the arena). Selection is
/// stored as a `VirtualPos` so a shared crate appearing at multiple DAG
/// positions has unambiguous identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct VirtualPos(pub usize);

/// [`TreeWidget`] state that tracks open nodes and the current selection.
///
/// [`TreeWidget`]: super::TreeWidget
#[derive(Debug)]
pub struct TreeWidgetState {
    /// Open/closed state indexed by node id.
    pub open: Vec<bool>,
    /// Virtual position of the selected node in the full flattened tree.
    selected_virtual_pos: Option<VirtualPos>,
    /// Scroll alignment requested by sibling navigation for the current selection.
    sibling_scroll: Option<(VirtualPos, ScrollAlignment)>,
    /// Current viewport.
    pub viewport: Viewport,
    /// Whether subtree sizes need recomputation.
    subtree_dirty: bool,
    /// Whether the visible cache needs re-materialization.
    dirty: bool,
    /// Materialized state for the normal (unfiltered) view.
    normal: ViewCache,
    /// Materialized state for the search-filtered view.
    search: ViewCache,
    /// Nodes kept visible by the active search, indexed by node id.
    search_visible_nodes: Vec<bool>,
    /// Nodes whose crate names directly match the active search query, indexed by node id.
    search_matches: Vec<bool>,
    /// Node ids whose `search_visible_nodes` bit is currently set, used for cheap resets.
    search_visible_ids: Vec<NodeId>,
    /// Node ids whose `search_matches` bit is currently set, used for cheap resets and refinement.
    search_match_ids: Vec<NodeId>,
}

/// Visible node metadata used for navigation and rendering.
#[derive(Debug, Clone, Copy)]
pub struct VisibleNode {
    /// Node identifier.
    pub id: NodeId,
    /// Depth in the tree hierarchy.
    pub depth: usize,
    /// Position in the full virtual flattened list.
    pub virtual_pos: VirtualPos,
    /// Index of the parent in the same visible cache, or `None` for roots.
    pub parent_vis_idx: Option<VisIdx>,
    /// Next sibling in the same visible cache sharing the same parent.
    pub next_sibling: Option<VisIdx>,
    /// Previous sibling in the same visible cache sharing the same parent.
    pub prev_sibling: Option<VisIdx>,
    /// Whether this node is the last non-group child of its parent in the
    /// full virtual stream under the current open/filter (not just within
    /// the materialized window). Drives the `└─` vs `├─` decision.
    pub is_last_non_group_child: bool,
}

/// Search result payload computed off the UI thread.
#[derive(Debug, Clone)]
pub struct SearchState {
    /// Nodes kept visible by the active search, indexed by node id.
    pub visible_nodes: Vec<bool>,
    /// Nodes whose crate names directly match the active search query, indexed by node id.
    pub matches: Vec<bool>,
    /// Nodes visible because of the active search.
    pub visible_ids: Vec<NodeId>,
    /// Nodes that directly match the active search.
    pub match_ids: Vec<NodeId>,
}

impl SearchState {
    fn new(node_count: usize) -> Self {
        Self {
            visible_nodes: vec![false; node_count],
            matches: vec![false; node_count],
            visible_ids: Vec::new(),
            match_ids: Vec::new(),
        }
    }
}

impl Default for TreeWidgetState {
    fn default() -> Self {
        Self {
            open: Vec::new(),
            selected_virtual_pos: None,
            sibling_scroll: None,
            viewport: Viewport::default(),
            subtree_dirty: true,
            dirty: true,
            normal: ViewCache::default(),
            search: ViewCache::default(),
            search_visible_nodes: Vec::new(),
            search_matches: Vec::new(),
            search_visible_ids: Vec::new(),
            search_match_ids: Vec::new(),
        }
    }
}

impl TreeWidgetState {
    /// Finds the node at the given virtual position in the active cache.
    fn find_by_vpos(&self, vpos: VirtualPos) -> Option<(VisIdx, &VisibleNode)> {
        self.active_visible_nodes()
            .iter()
            .enumerate()
            .find(|(_, n)| n.virtual_pos == vpos)
            .map(|(i, n)| (VisIdx(i), n))
    }

    /// Returns the `NodeId` of the currently selected visible position.
    ///
    /// Returns `None` if nothing is selected or the cache doesn't contain the
    /// selected position (call [`ensure_visible_nodes`] first).
    pub fn selected_node_id(&self) -> Option<NodeId> {
        let vpos = self.selected_virtual_pos?;
        self.find_by_vpos(vpos).map(|(_, n)| n.id)
    }

    /// Sets the selection to the virtual position of the given `NodeId`.
    ///
    /// Requires a DFS walk using subtree sizes to locate the first occurrence.
    pub fn set_selected_node_id(&mut self, tree: &DependencyTree, id: NodeId) {
        self.ensure_subtree_sizes(tree);
        let sizes = self.active_subtree_sizes();
        let filter = self.active_filter();
        let roots = tree.roots();

        self.selected_virtual_pos = find_virtual_pos(tree, &self.open, sizes, filter, roots, id);
        self.dirty = true;
    }

    /// Grows all node-indexed caches to match the current tree size.
    fn ensure_node_capacity(&mut self, tree: &DependencyTree) {
        let len = tree.nodes.len();
        if self.open.len() == len {
            return;
        }

        self.open.resize(len, false);
        self.search_visible_nodes.resize(len, false);
        self.search_matches.resize(len, false);
    }

    /// Clears any active search filtering state.
    pub fn clear_search(&mut self) {
        for node_id in self.search_visible_ids.drain(..) {
            self.search_visible_nodes[node_id.0] = false;
        }
        for node_id in self.search_match_ids.drain(..) {
            self.search_matches[node_id.0] = false;
        }
        self.search.clear();
        // Rematerialize the main view with the current selection.
        self.dirty = true;
    }

    /// Returns whether a node directly matches the active search query.
    pub fn is_search_match(&self, node_id: NodeId) -> bool {
        self.search_matches.get(node_id.0).copied().unwrap_or(false)
    }

    /// Applies externally computed search state to the visible tree.
    pub fn apply_search_state(&mut self, tree: &DependencyTree, search_state: SearchState) {
        self.ensure_node_capacity(tree);
        self.search_visible_nodes = search_state.visible_nodes;
        self.search_matches = search_state.matches;
        self.search_visible_ids = search_state.visible_ids;
        self.search_match_ids = search_state.match_ids;
        self.rebuild_search_view(tree);
    }

    /// Updates search-filtered nodes by matching crate names case-sensitively.
    pub fn set_search_query(&mut self, tree: &DependencyTree, query: &str) {
        if query.is_empty() {
            self.clear_search();
            return;
        }

        self.apply_search_state(tree, Self::search(tree, query));
    }

    /// Computes search-filtered nodes without mutating widget state.
    pub fn search(tree: &DependencyTree, query: &str) -> SearchState {
        if query.is_empty() {
            return SearchState::new(tree.nodes.len());
        }

        let mut search_state = SearchState::new(tree.nodes.len());

        for node_id in tree.crate_nodes() {
            let Some(DependencyNode::Crate(dependency)) = tree.node(node_id) else {
                continue;
            };

            if dependency.name.contains(query) {
                search_state.matches[node_id.0] = true;
                search_state.match_ids.push(node_id);
                Self::include_ancestors(
                    tree,
                    node_id,
                    &mut search_state.visible_nodes,
                    &mut search_state.visible_ids,
                );
            }
        }

        search_state
    }

    /// Moves the selection to the next visible dependency.
    pub fn select_next(&mut self, tree: &DependencyTree) {
        if !self.ensure_selection(tree) {
            return;
        }

        let Some(vpos) = self.selected_virtual_pos else {
            return;
        };
        let total = self.active_total_virtual_lines();
        if vpos.0 + 1 < total {
            self.selected_virtual_pos = Some(VirtualPos(vpos.0 + 1));
            self.dirty = true;
        }
    }

    /// Moves the selection to the previous visible dependency.
    pub fn select_previous(&mut self, tree: &DependencyTree) {
        if !self.ensure_selection(tree) {
            return;
        }

        let Some(vpos) = self.selected_virtual_pos else {
            return;
        };
        if vpos.0 > 0 {
            self.selected_virtual_pos = Some(VirtualPos(vpos.0 - 1));
            self.dirty = true;
        }
    }

    /// Expands or collapses (toggles) the selected node.
    pub fn toggle(&mut self, tree: &DependencyTree) {
        if !self.ensure_selection(tree) {
            return;
        }
        self.ensure_visible_nodes(tree);
        let Some(node_id) = self.selected_node_id() else {
            return;
        };
        let Some(node) = tree.node(node_id) else {
            return;
        };

        if node.children().is_empty() {
            return;
        }

        if self.open[node_id.0] {
            self.collapse(tree);
        } else {
            self.expand(tree);
        }
    }

    /// Expands the selected node or moves into its first child when already expanded.
    pub fn expand(&mut self, tree: &DependencyTree) {
        if !self.ensure_selection(tree) {
            return;
        }
        let Some(vpos) = self.selected_virtual_pos else {
            return;
        };
        self.ensure_visible_nodes(tree);
        let Some(node_id) = self.selected_node_id() else {
            return;
        };
        let Some(node) = tree.node(node_id) else {
            return;
        };

        if node.children().is_empty() {
            return;
        }

        if !self.open[node_id.0] {
            self.open[node_id.0] = true;
            self.subtree_dirty = true;
            self.dirty = true;
            return;
        }

        // Already open — move into first child (next virtual position).
        let total = self.active_total_virtual_lines();
        if vpos.0 + 1 < total {
            self.selected_virtual_pos = Some(VirtualPos(vpos.0 + 1));
            self.dirty = true;
        }
    }

    /// Collapses the selected node or moves focus to its parent when already closed.
    pub fn collapse(&mut self, tree: &DependencyTree) {
        if !self.ensure_selection(tree) {
            return;
        }
        self.ensure_visible_nodes(tree);
        let Some(node_id) = self.selected_node_id() else {
            return;
        };
        let Some(node) = tree.node(node_id) else {
            return;
        };

        // If the node has children and is open, close it.
        if !node.children().is_empty() && self.open[node_id.0] {
            self.open[node_id.0] = false;
            self.subtree_dirty = true;
            self.dirty = true;
            return;
        }

        // Otherwise move focus to parent.
        self.select_parent(tree);
    }

    /// Moves the selection to the parent node, if any.
    pub fn select_parent(&mut self, tree: &DependencyTree) {
        if !self.ensure_selection(tree) {
            return;
        }
        // Find the selected node in the cache and read its parent's virtual_pos.
        let Some(vpos) = self.selected_virtual_pos else {
            return;
        };
        if let Some((_, vnode)) = self.find_by_vpos(vpos)
            && let Some(parent_vis) = vnode.parent_vis_idx
            && let Some(parent_node) = self.active_visible_nodes().get(parent_vis.0)
        {
            self.selected_virtual_pos = Some(parent_node.virtual_pos);
            self.dirty = true;
        }
    }

    /// Moves the selection to the next sibling, if any.
    pub fn select_next_sibling(&mut self, tree: &DependencyTree) {
        self.select_sibling(tree, |n| n.next_sibling);
    }

    /// Moves the selection to the previous sibling, if any.
    pub fn select_previous_sibling(&mut self, tree: &DependencyTree) {
        self.select_sibling(tree, |n| n.prev_sibling);
    }

    fn select_sibling(
        &mut self,
        tree: &DependencyTree,
        pick: impl Fn(&VisibleNode) -> Option<VisIdx>,
    ) {
        if !self.ensure_selection(tree) {
            return;
        }
        let Some(vpos) = self.selected_virtual_pos else {
            return;
        };

        if let Some((_, vnode)) = self.find_by_vpos(vpos)
            && let Some(sibling) = pick(vnode)
            && let Some(sibling_node) = self.active_visible_nodes().get(sibling.0)
        {
            let target = sibling_node.virtual_pos;
            let alignment = if sibling_node.parent_vis_idx.is_none() {
                ScrollAlignment::Top
            } else {
                ScrollAlignment::Center
            };
            self.selected_virtual_pos = Some(target);
            self.sibling_scroll = Some((target, alignment));
            self.dirty = true;
        }
    }

    /// Returns the pending sibling-navigation alignment for the selected node.
    pub(crate) fn take_sibling_scroll_alignment(&mut self) -> Option<ScrollAlignment> {
        let selected = self.selected_virtual_pos?;
        let (target, alignment) = self.sibling_scroll.take()?;
        (target == selected).then_some(alignment)
    }

    /// Moves the selection up by approximately one page.
    pub fn page_up(&mut self, tree: &DependencyTree) {
        let step = self.viewport.height.saturating_sub(1).max(1) as isize;
        self.move_by(tree, -step);
    }

    /// Moves the selection down by approximately one page.
    pub fn page_down(&mut self, tree: &DependencyTree) {
        let step = self.viewport.height.saturating_sub(1).max(1) as isize;
        self.move_by(tree, step);
    }

    /// Moves the selection by a specified delta.
    fn move_by(&mut self, tree: &DependencyTree, delta: isize) {
        if !self.ensure_selection(tree) {
            return;
        }
        let Some(vpos) = self.selected_virtual_pos else {
            return;
        };
        let total = self.active_total_virtual_lines() as isize;
        if total == 0 {
            return;
        }

        let next = (vpos.0 as isize + delta).clamp(0, total - 1) as usize;
        if next != vpos.0 {
            self.selected_virtual_pos = Some(VirtualPos(next));
            self.dirty = true;
        }
    }

    /// Opens all nodes up to the specified depth.
    pub fn open_to_depth(&mut self, tree: &DependencyTree, max_depth: usize) {
        if max_depth == 0 {
            return;
        }
        self.ensure_node_capacity(tree);
        self.open.fill(false);
        let mut ancestors = FxHashSet::default();
        for &root in tree.roots() {
            self.open_node(tree, root, 1, max_depth, &mut ancestors);
        }
        self.subtree_dirty = true;
        self.dirty = true;
        self.ensure_selection(tree);
    }

    /// Opens nodes so the initial view includes dependencies through the given
    /// crate depth. Synthetic dependency-group nodes do not count toward depth.
    pub fn open_to_dependency_depth(&mut self, tree: &DependencyTree, max_depth: usize) {
        self.ensure_node_capacity(tree);
        self.open.fill(false);
        let mut ancestors = FxHashSet::default();
        for &root in tree.roots() {
            self.open_dependency_node(tree, root, 0, max_depth, &mut ancestors);
        }
        self.subtree_dirty = true;
        self.dirty = true;
        self.ensure_selection(tree);
    }

    fn open_dependency_node(
        &mut self,
        tree: &DependencyTree,
        id: NodeId,
        crate_depth: usize,
        max_depth: usize,
        ancestors: &mut FxHashSet<NodeId>,
    ) {
        if crate_depth >= max_depth {
            return;
        }

        if let Some(node) = tree.node(id) {
            if node.children().is_empty() {
                return;
            }

            self.open[id.0] = true;
            ancestors.insert(id);
            for &child in node.children() {
                if ancestors.contains(&child) {
                    continue;
                }
                let child_depth = if tree.node(child).is_some_and(DependencyNode::is_group) {
                    crate_depth
                } else {
                    crate_depth + 1
                };
                self.open_dependency_node(tree, child, child_depth, max_depth, ancestors);
            }
            ancestors.remove(&id);
        }
    }

    fn open_node(
        &mut self,
        tree: &DependencyTree,
        id: NodeId,
        depth: usize,
        max_depth: usize,
        ancestors: &mut FxHashSet<NodeId>,
    ) {
        if depth >= max_depth {
            return;
        }

        if let Some(node) = tree.node(id) {
            if node.children().is_empty() {
                return;
            }

            self.open[id.0] = true;
            ancestors.insert(id);
            for &child in node.children() {
                if !ancestors.contains(&child) {
                    self.open_node(tree, child, depth + 1, max_depth, ancestors);
                }
            }
            ancestors.remove(&id);
        }
    }

    /// Returns cached visible nodes along with their depth in the hierarchy.
    pub fn visible_nodes(&mut self, tree: &DependencyTree) -> &[VisibleNode] {
        self.ensure_visible_nodes(tree);
        self.active_visible_nodes()
    }

    /// Recomputes subtree sizes if dirty.
    fn ensure_subtree_sizes(&mut self, tree: &DependencyTree) {
        if !self.subtree_dirty {
            return;
        }

        self.ensure_node_capacity(tree);

        self.normal.refresh_sizes(tree, &self.open, None);

        if self.is_searching() {
            self.search
                .refresh_sizes(tree, &self.open, Some(&self.search_visible_nodes));
        }

        self.subtree_dirty = false;
    }

    /// Rebuilds the visible caches lazily when tree openness has changed.
    pub fn ensure_visible_nodes(&mut self, tree: &DependencyTree) {
        if !self.dirty && !self.subtree_dirty {
            return;
        }

        self.ensure_subtree_sizes(tree);
        self.rebuild_visible(tree);
        self.dirty = false;
    }

    /// Returns the currently active visible slice.
    pub fn active_visible_nodes(&self) -> &[VisibleNode] {
        &self.active_cache().nodes
    }

    /// Returns the total virtual line count for the active view.
    fn active_total_virtual_lines(&self) -> usize {
        self.active_cache().total_virtual_lines
    }

    /// Returns the active subtree sizes slice.
    fn active_subtree_sizes(&self) -> &[usize] {
        if self.is_searching() && !self.search.subtree_sizes.is_empty() {
            &self.search.subtree_sizes
        } else {
            &self.normal.subtree_sizes
        }
    }

    /// Returns whether a search filter is currently active.
    fn is_searching(&self) -> bool {
        !self.search_match_ids.is_empty()
    }

    /// Returns the active ViewCache (search if searching, normal otherwise).
    fn active_cache(&self) -> &ViewCache {
        if self.is_searching() {
            &self.search
        } else {
            &self.normal
        }
    }

    /// Returns the active filter, if searching.
    fn active_filter(&self) -> Option<&[bool]> {
        if self.is_searching() {
            Some(&self.search_visible_nodes)
        } else {
            None
        }
    }

    fn rebuild_visible(&mut self, tree: &DependencyTree) {
        let vpos = self.selected_virtual_pos.unwrap_or(VirtualPos(0));

        // Use viewport height if known, otherwise a generous default.
        let viewport_height = if self.viewport.height > 0 {
            self.viewport.height
        } else {
            50
        };

        // Estimate window_start from the selected position, with generous
        // buffer to ensure the materialized window covers what render needs.
        // Start well before the selection so ancestor prefix nodes are included.
        let window_start = vpos.0.saturating_sub(viewport_height);

        // Materialize enough for viewport + buffer for scrolling.
        let window_count = viewport_height * 2;

        let searching = self.is_searching();
        let (cache, filter): (&mut ViewCache, Option<&[bool]>) = if searching {
            (&mut self.search, Some(&self.search_visible_nodes))
        } else {
            (&mut self.normal, None)
        };

        cache.rematerialize(
            tree,
            &self.open,
            filter,
            tree.roots(),
            window_start..window_start + window_count,
        );
    }

    /// Rebuilds the search view after applying new search state.
    fn rebuild_search_view(&mut self, tree: &DependencyTree) {
        if self.search_match_ids.is_empty() {
            self.search.clear();
            return;
        }

        self.search
            .refresh_sizes(tree, &self.open, Some(&self.search_visible_nodes));

        // Clamp selection to search view bounds.
        if let Some(vpos) = self.selected_virtual_pos
            && vpos.0 >= self.search.total_virtual_lines
            && self.search.total_virtual_lines > 0
        {
            self.selected_virtual_pos = Some(VirtualPos(self.search.total_virtual_lines - 1));
        }

        self.subtree_dirty = false;
        self.dirty = true;
        self.ensure_visible_nodes(tree);
    }

    /// Marks a matching node and all of its visible ancestors in the search bitset.
    ///
    /// A deduplicated node can have multiple parents, so this walks the full
    /// parent DAG rather than a single ancestor chain.
    fn include_ancestors(
        tree: &DependencyTree,
        id: NodeId,
        search_visible_nodes: &mut [bool],
        search_visible_ids: &mut Vec<NodeId>,
    ) {
        let mut stack = vec![id];
        while let Some(node_id) = stack.pop() {
            if search_visible_nodes[node_id.0] {
                continue;
            }

            search_visible_nodes[node_id.0] = true;
            search_visible_ids.push(node_id);

            stack.extend_from_slice(&tree.parents[node_id.0]);
        }
    }

    /// Ensures the selection points to a valid visible node, defaulting to position 0.
    ///
    /// Returns `true` if a valid selection exists after the operation.
    fn ensure_selection(&mut self, tree: &DependencyTree) -> bool {
        self.ensure_subtree_sizes(tree);
        let total = self.active_total_virtual_lines();

        if total == 0 {
            self.selected_virtual_pos = None;
            return false;
        }

        match self.selected_virtual_pos {
            Some(vpos) if vpos.0 < total => true,
            Some(_) => {
                self.selected_virtual_pos = Some(VirtualPos(total - 1));
                self.dirty = true;
                true
            }
            None => {
                self.selected_virtual_pos = Some(VirtualPos(0));
                self.dirty = true;
                true
            }
        }
    }

    /// Returns the VisIdx of the selected node within the materialized cache.
    pub fn selected_position(&mut self, tree: &DependencyTree) -> Option<VisIdx> {
        if !self.ensure_selection(tree) {
            return None;
        }
        self.ensure_visible_nodes(tree);
        self.selected_vis_idx()
    }

    /// Returns the cached selection VisIdx without triggering a rebuild.
    pub fn selected_position_cached(&self) -> Option<VisIdx> {
        self.selected_vis_idx()
    }

    /// Finds the VisIdx of the selected virtual position in the active cache.
    fn selected_vis_idx(&self) -> Option<VisIdx> {
        let vpos = self.selected_virtual_pos?;
        self.find_by_vpos(vpos).map(|(idx, _)| idx)
    }

    /// Returns the total virtual line count for scrollbar/viewport calculations.
    pub fn total_lines(&mut self, tree: &DependencyTree) -> usize {
        self.ensure_subtree_sizes(tree);
        self.active_total_virtual_lines()
    }

    /// Returns the selected virtual position.
    pub fn selected_virtual_pos(&self) -> Option<VirtualPos> {
        self.selected_virtual_pos
    }

    /// Updates the available viewport.
    ///
    /// If the new viewport height exceeds the previous one, the materialized
    /// window (sized at `height * 2`) no longer covers the visible area, so
    /// rematerialization is forced.
    pub(crate) fn update_viewport(&mut self, viewport: Viewport) {
        if viewport.height > self.viewport.height {
            self.dirty = true;
        }
        self.viewport = viewport;
    }

    /// Expands all nodes in the tree.
    pub fn expand_all(&mut self, tree: &DependencyTree) {
        self.ensure_node_capacity(tree);
        self.open.fill(false);
        for i in 0..tree.nodes.len() {
            let id = NodeId(i);
            if let Some(node) = tree.node(id) {
                // Only mark non-leaf nodes as open, leaves stay implicit.
                if !node.children().is_empty() {
                    self.open[id.0] = true;
                }
            }
        }
        self.subtree_dirty = true;
        self.dirty = true;
        self.ensure_selection(tree);
    }
}

/// Finds the virtual position of the first occurrence of a `NodeId` in the virtual tree.
fn find_virtual_pos(
    tree: &DependencyTree,
    open: &[bool],
    sizes: &[usize],
    filter: Option<&[bool]>,
    roots: &[NodeId],
    target: NodeId,
) -> Option<VirtualPos> {
    let mut vpos = 0usize;
    for &root in roots {
        if filter.is_some_and(|f| !f[root.0]) {
            continue;
        }
        if let Some(found) = find_vpos_recursive(tree, open, sizes, filter, root, target, &mut vpos)
        {
            return Some(VirtualPos(found));
        }
    }
    None
}

fn find_vpos_recursive(
    tree: &DependencyTree,
    open: &[bool],
    sizes: &[usize],
    filter: Option<&[bool]>,
    id: NodeId,
    target: NodeId,
    vpos: &mut usize,
) -> Option<usize> {
    if id == target {
        return Some(*vpos);
    }

    *vpos += 1;

    if open[id.0]
        && let Some(node) = tree.node(id)
    {
        for &child in node.children() {
            if filter.is_some_and(|f| !f[child.0]) {
                continue;
            }
            if child != target && sizes[child.0] == 0 {
                continue;
            }
            if let Some(found) = find_vpos_recursive(tree, open, sizes, filter, child, target, vpos)
            {
                return Some(found);
            }
        }
    }

    None
}
