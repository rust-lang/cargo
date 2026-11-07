//! Render incoming dependency usage and its declaration context.

use super::declaration::{format_usage_declaration, push_compact_field, toml_wrapped_line_count};
use super::graph::GraphIndex;
use super::{MetadataPopup, MetadataPopupState};
use crate::ops::cargo_tree::tui::dependency::{
    Dependency, DependencyNode, DependencyTree, DependencyUsage, NodeId,
};
use crate::ops::cargo_tree::tui::{
    TuiTree,
    widget::{TreeWidget, TreeWidgetState},
};
use crate::util::style::{HEADER, NOP};
use crate::workspace::PackageId;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget,
        Widget, Wrap,
    },
};
use rustc_hash::FxHashMap;

#[derive(Debug)]
pub(super) struct UsageTree {
    pub(super) package_id: Option<PackageId>,
    pub(super) tree: TuiTree,
    pub(super) state: TreeWidgetState,
    pub(super) usage_indices: Vec<Vec<usize>>,
    pub(super) parent_nodes: FxHashMap<PackageId, NodeId>,
}
impl MetadataPopup<'_> {
    pub(super) fn render_usage(
        &self,
        area: Rect,
        buf: &mut Buffer,
        state: &mut MetadataPopupState,
    ) {
        let Some(dependency) = self.dependency else {
            Paragraph::new(" No dependency selected ").render(area, buf);
            return;
        };
        let Some(usages) = self.usages.filter(|usages| !usages.is_empty()) else {
            Paragraph::new(" No incoming dependency declarations in this tree ").render(area, buf);
            return;
        };

        let package_id = dependency.package_id;
        if state.graph_index.is_none() {
            state.graph_index = Some(GraphIndex::new(self.source_tree));
        }
        if state
            .usage_tree
            .as_ref()
            .is_none_or(|cached| cached.package_id != package_id)
        {
            let usage_tree = build_usage_tree(
                dependency,
                usages,
                self.source_tree,
                &state
                    .graph_index
                    .as_ref()
                    .expect("graph index was initialized")
                    .package_nodes,
            );
            state.usage_tree = Some(usage_tree);
        }
        let usage_tree = state
            .usage_tree
            .as_mut()
            .expect("usage tree was initialized");
        usage_tree.state.ensure_visible_nodes(&usage_tree.tree);
        let selected = usage_tree.state.selected_node_id();
        let usage_index = selected
            .and_then(|id| usage_tree.usage_indices.get(id.0))
            .and_then(|indices| indices.first())
            .copied();
        let selected_usage = usage_index.and_then(|index| usages.get(index));
        let declaration =
            selected_usage.map(|usage| format_usage_declaration(usage, &dependency.version));
        let [tree_area, details_column] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                .areas(area);
        let declaration_height = declaration
            .as_ref()
            .map(|(section, text)| {
                let content_width = details_column.width.saturating_sub(3).max(1) as usize;
                2 + toml_wrapped_line_count(&Line::from(section.as_str()), content_width)
                    + toml_wrapped_line_count(&Line::from(text.as_str()), content_width)
            })
            .unwrap_or(0)
            .min(details_column.height as usize) as u16;
        let details_height = details_column.height.saturating_sub(declaration_height);
        let [details_area, declaration_area] = Layout::vertical([
            Constraint::Length(details_height),
            Constraint::Length(declaration_height),
        ])
        .areas(details_column);
        let tree_widget_area = Rect {
            width: tree_area.width.saturating_sub(1),
            ..tree_area
        };
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .track_symbol(Some("┆"))
            .thumb_symbol("▐")
            .begin_symbol(Some("▴"))
            .end_symbol(Some("▾"));
        let widget = TreeWidget::new(&usage_tree.tree)
            .prefix(self.prefix)
            .charset(self.charset)
            .format(self.format)
            .scrollbar(scrollbar);
        widget.render(tree_widget_area, buf, &mut usage_tree.state);

        if let Some(usage) = selected_usage {
            let mut lines = Vec::with_capacity(9);
            lines.push(Line::from(Span::styled(
                "Dependency declaration",
                Style::from(HEADER).add_modifier(Modifier::BOLD),
            )));
            if let Some(parent) = usage_tree
                .parent_nodes
                .get(&usage.parent)
                .and_then(|id| usage_tree.tree.node(*id))
                .and_then(DependencyNode::as_dependency)
            {
                push_compact_field(
                    &mut lines,
                    "From",
                    format!("{} v{}", parent.name, parent.version),
                );
            }
            push_compact_field(
                &mut lines,
                "To",
                format!("{} v{}", dependency.name, dependency.version),
            );
            push_compact_field(
                &mut lines,
                "Manifest section",
                format!("[{}]", usage.kind.kind_table()),
            );
            push_compact_field(
                &mut lines,
                "Declared as",
                usage.dependency.name_in_toml().as_str(),
            );
            push_compact_field(
                &mut lines,
                "Optional",
                if usage.dependency.is_optional() {
                    "yes"
                } else {
                    "no"
                },
            );
            push_compact_field(
                &mut lines,
                "Default features",
                if usage.dependency.uses_default_features() {
                    "enabled"
                } else {
                    "disabled"
                },
            );
            push_compact_field(
                &mut lines,
                "Requested features",
                if usage.dependency.features().is_empty() {
                    "none".to_owned()
                } else {
                    usage
                        .dependency
                        .features()
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                },
            );
            if let Some(target) = usage.dependency.platform() {
                push_compact_field(&mut lines, "Target condition", target.to_string());
            }
            Paragraph::new(Text::from(lines))
                .style(Style::from(NOP))
                .wrap(Wrap { trim: true })
                .render(details_area, buf);
        }

        if let Some(declaration) = declaration {
            let block = Block::default()
                .title(Line::from(Span::styled(
                    " Cargo.toml (simplified) ",
                    Style::from(HEADER)
                        .add_modifier(Modifier::BOLD)
                        .add_modifier(Modifier::REVERSED),
                )))
                .borders(Borders::ALL)
                .border_style(Style::from(HEADER));
            let content_area = block.inner(declaration_area);
            block.render(declaration_area, buf);
            let lines = Text::from(vec![Line::from(declaration.0), Line::from(declaration.1)]);
            let paragraph_area = Rect {
                width: content_area.width.saturating_sub(1),
                ..content_area
            };
            let content_height = lines
                .lines
                .iter()
                .map(|line| toml_wrapped_line_count(line, paragraph_area.width.max(1) as usize))
                .sum::<usize>();
            let max_scroll = content_height.saturating_sub(paragraph_area.height as usize);
            state.scroll = state.scroll.min(max_scroll);
            state.page_size = paragraph_area.height as usize;
            Paragraph::new(lines)
                .style(Style::from(NOP))
                .wrap(Wrap { trim: false })
                .scroll((state.scroll.min(u16::MAX as usize) as u16, 0))
                .render(paragraph_area, buf);
            if max_scroll > 0 {
                let mut scrollbar_state = ScrollbarState::new(max_scroll)
                    .position(state.scroll)
                    .viewport_content_length(paragraph_area.height as usize);
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .track_symbol(Some("┆"))
                    .thumb_symbol("▐")
                    .render(content_area, buf, &mut scrollbar_state);
            }
        }
    }
}

pub(super) fn build_usage_tree(
    dependency: &Dependency,
    usages: &[DependencyUsage],
    source_tree: &TuiTree,
    source_package_nodes: &FxHashMap<PackageId, NodeId>,
) -> UsageTree {
    let mut root = dependency.clone();
    root.children.clear();
    let mut nodes = vec![DependencyNode::Crate(root)];
    let mut usage_indices = vec![Vec::new()];
    let mut parent_nodes = FxHashMap::<PackageId, NodeId>::default();
    let mut selected_node = None;

    for (index, usage) in usages.iter().enumerate() {
        let Some(parent_node_id) = source_package_nodes.get(&usage.parent) else {
            continue;
        };
        let Some(parent_dependency) = source_tree
            .node(*parent_node_id)
            .and_then(DependencyNode::as_dependency)
        else {
            continue;
        };
        let parent_id = *parent_nodes.entry(usage.parent).or_insert_with(|| {
            let mut parent = parent_dependency.clone();
            parent.children.clear();
            let id = NodeId(nodes.len());
            nodes.push(DependencyNode::Crate(parent));
            usage_indices.push(Vec::new());
            if let DependencyNode::Crate(root) = &mut nodes[0] {
                root.children.push(id);
            }
            id
        });
        usage_indices[parent_id.0].push(index);

        selected_node.get_or_insert(parent_id);
    }

    let mut parents = vec![Vec::new(); nodes.len()];
    for (parent, node) in nodes.iter().enumerate() {
        for &child in node.children() {
            parents[child.0].push(NodeId(parent));
        }
    }
    let tree = DependencyTree {
        workspace_name: dependency.name.clone(),
        nodes,
        parents,
        dependency_usages: FxHashMap::default(),
        roots: vec![NodeId(0)],
    };
    let mut state = TreeWidgetState::default();
    state.expand_all(&tree);
    if let Some(selected_node) = selected_node {
        state.set_selected_node_id(&tree, selected_node);
    }
    UsageTree {
        package_id: dependency.package_id,
        tree: TuiTree::new(tree, true),
        state,
        usage_indices,
        parent_nodes,
    }
}
