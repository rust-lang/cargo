use crate::util::style::{DEP_BUILD, DEP_FEATURE};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Block,
};

use crate::ops::cargo_tree::tui::TuiTree;
use crate::ops::cargo_tree::tui::dependency::{Dependency, DependencyNode};

use super::{
    format::{FormatStyles, TreeFormat},
    lineage::Lineage,
    state::{TreeWidgetState, VisIdx, VisibleNode},
    style::{TreePrefix, TreeWidgetStyle},
    viewport::Viewport,
};

#[derive(Default)]
pub struct RenderOutput<'a> {
    pub lines: Vec<Line<'a>>,
    pub context_lines: Vec<Line<'a>>,
    pub total_lines: usize,
    pub viewport: Viewport,
}

/// Context for rendering the dependency tree.
///
/// # Note for lifetimes
///
/// - `'a` is the lifetime of the dependency tree and style references.
/// - `'s` is the lifetime of the mutable state reference.
///
/// The reason why we keep them separate is to let the mutable borrow of the
/// state end before we later borrow the state immutably (e.g. for breadcrumb
/// rendering) while still holding references to the tree and style.
pub struct RenderContext<'a, 's> {
    pub tree: &'a TuiTree,
    pub state: &'s mut TreeWidgetState,
    pub style: &'a TreeWidgetStyle,
    pub block: Option<&'a Block<'a>>,
    pub prefix: TreePrefix,
    pub format: Option<&'a TreeFormat>,
}

impl<'a, 's> RenderContext<'a, 's> {
    pub fn new(
        tree: &'a TuiTree,
        state: &'s mut TreeWidgetState,
        style: &'a TreeWidgetStyle,
        block: Option<&'a Block<'a>>,
        prefix: TreePrefix,
        format: Option<&'a TreeFormat>,
    ) -> Self {
        Self {
            tree,
            state,
            style,
            block,
            prefix,
            format,
        }
    }

    pub fn render(&mut self, area: Rect) -> RenderOutput<'a> {
        if self.state.selected_position(self.tree).is_none() {
            return RenderOutput::default();
        };

        self.state.ensure_visible_nodes(self.tree);

        let total_lines = self.state.total_lines(self.tree);
        let selected_vpos = self.state.selected_virtual_pos();
        let prev_offset = self.state.viewport.offset;
        let selected_vline = selected_vpos.map(|vp| vp.0).unwrap_or(0);
        let sibling_alignment = self.state.take_sibling_scroll_alignment();
        let mut viewport = Viewport::new(area, self.block).scroll_into_view(
            selected_vline,
            total_lines,
            1,
            prev_offset,
            sibling_alignment,
        );
        self.state.update_viewport(viewport);

        // Context lines: walk parent_vis_idx from the node at viewport.offset.min(max_offset),
        // matching the original context bar behavior.
        let context_vpos = viewport.offset.min(viewport.max_offset);
        let context_lines = if context_vpos > 0 {
            let visible_nodes = self.state.active_visible_nodes();
            let selected_vis = self.state.selected_position_cached();
            if let Some(context_idx) = visible_nodes
                .iter()
                .position(|n| n.virtual_pos.0 == context_vpos)
            {
                self.render_context_lines(visible_nodes, context_idx, selected_vis)
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        let content_height = viewport.height.saturating_sub(context_lines.len());
        viewport.clamp_offset(total_lines, context_lines.len());
        self.state.update_viewport(viewport);

        // Render viewport rows: find nodes with virtual_pos in [viewport.offset, offset + content_height).
        let render_start_vpos = viewport.offset;
        let render_end_vpos = viewport.offset + content_height;
        let mut lines = Vec::with_capacity(content_height);
        {
            let visible_nodes = self.state.active_visible_nodes();
            let selected_vis = self.state.selected_position_cached();
            for (i, vnode) in visible_nodes.iter().enumerate() {
                if vnode.virtual_pos.0 < render_start_vpos {
                    continue;
                }
                if vnode.virtual_pos.0 >= render_end_vpos {
                    break;
                }
                let vis = VisIdx(i);
                if let Some(line) =
                    self.render_visible_node(visible_nodes, vis, selected_vis, false)
                {
                    lines.push(line);
                }
            }
        }

        RenderOutput {
            lines,
            context_lines,
            total_lines,
            viewport,
        }
    }

    pub fn render_visible_node(
        &self,
        visible_nodes: &[VisibleNode],
        vis_idx: VisIdx,
        selected_vis: Option<VisIdx>,
        context_lines: bool,
    ) -> Option<Line<'a>> {
        let vnode = visible_nodes.get(vis_idx.0)?;
        let node_id = vnode.id;
        let node_data = self.tree.node(node_id)?;
        let lineage = match self.prefix {
            TreePrefix::Depth => {
                Lineage::build::<true>(self.tree, visible_nodes, vis_idx, selected_vis)?
            }
            TreePrefix::Indent | TreePrefix::None => {
                Lineage::build::<false>(self.tree, visible_nodes, vis_idx, selected_vis)?
            }
        };
        let has_children = !node_data.children().is_empty();
        let is_open = self.state.open.get(node_id.0).copied().unwrap_or(false);
        let is_group = node_data.is_group();

        let is_root = vnode.parent_vis_idx.is_none();
        let show_connector = !is_root;

        let mut spans = Vec::new();

        let toggle = if has_children {
            if is_open {
                format!("{} ", self.style.node_open_symbol)
            } else {
                format!("{} ", self.style.node_closed_symbol)
            }
        } else {
            format!("{} ", self.style.node_symbol)
        };

        match self.prefix {
            TreePrefix::Indent if show_connector => {
                for segment in &lineage.segments {
                    if segment.is_group {
                        continue;
                    }
                    let base_style = if context_lines {
                        self.style.context_style
                    } else {
                        segment.edge_style.unwrap_or(self.style.style)
                    };
                    let symbol = if segment.has_more_siblings {
                        self.style.continuation_symbol
                    } else {
                        self.style.empty_symbol
                    };

                    spans.push(Span::styled(symbol, base_style));
                }

                if !is_group {
                    let connector = if lineage.is_last {
                        self.style.last_branch_symbol
                    } else {
                        self.style.branch_symbol
                    };
                    let parent_group_style = vnode
                        .parent_vis_idx
                        .and_then(|pvis| visible_nodes.get(pvis.0))
                        .and_then(|pvnode| self.tree.node(pvnode.id))
                        .and_then(|parent| parent.as_group().map(|group| group.style()));
                    let connector_style = parent_group_style.unwrap_or(self.style.style);
                    spans.push(Span::styled(connector, connector_style));
                    spans.push(Span::styled(toggle, self.style.style));
                }
            }
            TreePrefix::Depth => {
                spans.push(Span::styled(
                    format!(
                        "{} ",
                        lineage
                            .package_depth
                            .expect("depth prefix computes package depth")
                    ),
                    self.style.context_style,
                ));
                if show_connector && !is_group {
                    spans.push(Span::styled(toggle, self.style.style));
                }
            }
            TreePrefix::None if show_connector && !is_group => {
                spans.push(Span::styled(toggle, self.style.style));
            }
            TreePrefix::Indent | TreePrefix::None => {}
        }

        let is_search_match = self.state.is_search_match(node_id);
        let name_style = if lineage.is_selected {
            self.style.highlight_style
        } else if is_search_match {
            self.style.filtered_style
        } else {
            self.style.name_style
        };
        let license_style = if lineage.is_selected || is_search_match {
            name_style
        } else {
            DEP_BUILD.into()
        }
        .remove_modifier(Modifier::BOLD);
        let feature_style = if lineage.is_selected || is_search_match {
            name_style
        } else {
            DEP_FEATURE.into()
        };

        match node_data {
            DependencyNode::Crate(dependency) => {
                if let Some(format) = self.format {
                    format.append_spans(
                        dependency,
                        &mut spans,
                        FormatStyles {
                            name: name_style,
                            license: license_style,
                            feature: feature_style,
                            version: self.style.version_style,
                            suffix: self.style.suffix_style,
                        },
                    );
                    if self.tree.is_duplicate(node_id) {
                        spans.push(Span::styled(" (*)", self.style.suffix_style));
                    }
                } else {
                    spans.push(Span::styled(dependency.name.clone(), name_style));
                    if self.tree.is_duplicate(node_id) {
                        spans.push(Span::styled(" (*)", self.style.suffix_style));
                    }
                    if !dependency.version.is_empty() {
                        spans.push(Span::styled(
                            format!(" v{}", dependency.version),
                            self.style.version_style,
                        ));
                    }

                    if let Some(extra) = format_suffixes(dependency, self.style) {
                        spans.extend(extra);
                    }
                }
            }
            DependencyNode::Group(group) => {
                let group_style = if lineage.is_selected {
                    self.style.highlight_style
                } else if self.state.is_search_match(node_id) {
                    self.style.filtered_style
                } else {
                    group.style()
                };
                spans.push(Span::styled(group.label().to_string(), group_style));
            }
            DependencyNode::Feature(feature) => {
                let is_search_match = self.state.is_search_match(node_id);
                let feature_style = if lineage.is_selected {
                    self.style.highlight_style
                } else if is_search_match {
                    self.style.filtered_style
                } else {
                    crate::util::style::DEP_FEATURE.into()
                };
                let prefix_style = if lineage.is_selected || is_search_match {
                    feature_style
                } else {
                    Style::default().fg(Color::White)
                };
                spans.push(Span::styled(
                    format!("{} feature ", feature.package_name),
                    prefix_style,
                ));
                spans.push(Span::styled(format!("\"{}\"", feature.name), feature_style));
                if feature.command_line {
                    spans.push(Span::styled(" (command-line)", self.style.suffix_style));
                }
                if self.tree.is_duplicate(node_id) {
                    spans.push(Span::styled(" (*)", self.style.suffix_style));
                }
            }
        }

        Some(Line::from(spans))
    }

    /// Renders context lines by walking the parent chain from the first window-zone node.
    fn render_context_lines(
        &self,
        visible_nodes: &[VisibleNode],
        first_window_idx: usize,
        selected_vis: Option<VisIdx>,
    ) -> Vec<Line<'a>> {
        let Some(first_visible) = visible_nodes.get(first_window_idx) else {
            return Vec::new();
        };

        // Collect ancestor visible indices bottom → top.
        let mut ancestor_vis_indices: Vec<VisIdx> = Vec::new();
        let mut current_vis = first_visible.parent_vis_idx;

        while let Some(vis_idx) = current_vis {
            if let Some(vnode) = visible_nodes.get(vis_idx.0) {
                ancestor_vis_indices.push(vis_idx);
                current_vis = vnode.parent_vis_idx;
            } else {
                break;
            }
        }

        // Render top → bottom.
        ancestor_vis_indices
            .into_iter()
            .rev()
            .filter_map(|vis_idx| {
                self.render_visible_node(visible_nodes, vis_idx, selected_vis, true)
            })
            .collect()
    }
}

/// Formats suffixes for a dependency node.
fn format_suffixes<'a>(node: &Dependency, style: &TreeWidgetStyle) -> Option<Vec<Span<'a>>> {
    let mut suffixes = Vec::new();

    if let Some(path) = &node.manifest_dir {
        suffixes.push(path.to_string());
    }

    if node.is_proc_macro {
        suffixes.push("proc-macro".to_string());
    }

    if suffixes.is_empty() {
        return None;
    }

    let mut spans = Vec::new();
    spans.push(Span::styled(" (", style.style));

    for (idx, suffix) in suffixes.iter().enumerate() {
        if idx > 0 {
            spans.push(Span::styled(", ", style.style));
        }
        spans.push(Span::styled(suffix.clone(), style.suffix_style));
    }

    spans.push(Span::styled(")", style.style));

    Some(spans)
}
