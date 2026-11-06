use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Widget,
};

use crate::ops::cargo_tree::tui::dependency::DependencyTree;

use super::{state::TreeWidgetState, style::TreeWidgetStyle};

const CONNECTOR_SYMBOL: char = '→';
const CONTINUATION_SYMBOL: char = '…';
const FOOTER_RESERVED_WIDTH: u16 = 32;

#[derive(Clone)]
struct Crumb {
    name: String,
    group_style: Option<Style>,
    is_group: bool,
}

pub struct Breadcrumb<'a> {
    tree: &'a DependencyTree,
    state: &'a TreeWidgetState,
    style: &'a TreeWidgetStyle,
}

impl<'a> Breadcrumb<'a> {
    /// Create a breadcrumb widget for the current tree selection.
    pub fn new(
        tree: &'a DependencyTree,
        state: &'a TreeWidgetState,
        style: &'a TreeWidgetStyle,
    ) -> Self {
        Self { tree, state, style }
    }

    /// Collect the breadcrumb trail from root to the selected node.
    fn collect_crumbs(&self) -> Vec<Crumb> {
        let mut crumbs = Vec::new();
        // Walk the visible cache via parent_vis_idx for correct position-aware breadcrumbs.
        let visible = self.state.active_visible_nodes();
        let mut current_vis = self.state.selected_position_cached();

        while let Some(vis_idx) = current_vis {
            let Some(vnode) = visible.get(vis_idx.0) else {
                break;
            };
            let Some(node) = self.tree.node(vnode.id) else {
                break;
            };

            let group_style = node.as_group().map(|group| group.style());
            crumbs.push(Crumb {
                name: node.display_label(),
                group_style,
                is_group: node.is_group(),
            });
            current_vis = vnode.parent_vis_idx;
        }

        crumbs.reverse();
        crumbs
    }

    /// Elide middle items with a continuation marker when the breadcrumb is too wide.
    ///
    /// The output always keeps the root and current node, then adds as many
    /// prefix items as will fit between them.
    fn elide_crumbs(mut crumbs: Vec<Crumb>, max_width: usize) -> Vec<Crumb> {
        if crumbs.len() <= 2 {
            return crumbs;
        }

        let sep_len = format!(" {CONNECTOR_SYMBOL} ").chars().count();
        let full_len: usize = crumbs
            .iter()
            .map(|crumb| crumb.name.chars().count())
            .sum::<usize>()
            .saturating_add(sep_len.saturating_mul(crumbs.len().saturating_sub(1)));

        if full_len <= max_width {
            return crumbs;
        }

        let ellipsis = Crumb {
            name: CONTINUATION_SYMBOL.to_string(),
            group_style: None,
            is_group: false,
        };
        let last_idx = crumbs.len() - 1;
        let mut prefix_len = 1usize;

        let total_len = |prefix_count: usize, crumbs: &[Crumb]| -> usize {
            let prefix_len_sum: usize = crumbs
                .iter()
                .take(prefix_count)
                .map(|crumb| crumb.name.chars().count())
                .sum();
            let last_len = crumbs[last_idx].name.chars().count();
            let item_count = prefix_count + 2;
            prefix_len_sum
                .saturating_add(ellipsis.name.chars().count())
                .saturating_add(last_len)
                .saturating_add(sep_len.saturating_mul(item_count.saturating_sub(1)))
        };

        while prefix_len + 1 < last_idx && total_len(prefix_len + 1, &crumbs) <= max_width {
            prefix_len += 1;
        }

        let mut minimized = Vec::with_capacity(prefix_len + 2);
        minimized.extend_from_slice(&crumbs[..prefix_len]);
        minimized.push(ellipsis);
        minimized.push(crumbs.remove(last_idx));
        minimized
    }
}

impl Widget for Breadcrumb<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let crumbs = self.collect_crumbs();

        let max_width = area.width.saturating_sub(FOOTER_RESERVED_WIDTH) as usize;
        let display_crumbs = Self::elide_crumbs(crumbs, max_width);
        let mut spans = Vec::new();

        for (i, crumb) in display_crumbs.iter().enumerate() {
            let is_last = i + 1 == display_crumbs.len();
            let is_next_group = display_crumbs.get(i + 1).is_some_and(|next| next.is_group);
            let is_group = crumb.is_group;

            let style = if is_next_group || is_group {
                crumb.group_style.unwrap_or(self.style.style)
            } else {
                self.style.style
            };

            if !is_group {
                spans.push(Span::styled(
                    crumb.name.clone(),
                    if is_last {
                        self.style.highlight_style
                    } else {
                        self.style.style
                    },
                ));
            }

            if !is_last && !is_next_group {
                spans.push(Span::styled(format!(" {CONNECTOR_SYMBOL} "), style));
            }
        }
        Line::from(spans).render(area, buf);
    }
}
