//! Dependency metadata popup and its package and usage views.

use crate::ops::cargo_tree::tui::{
    TuiTree,
    dependency::{Dependency, DependencyUsage},
    widget::{TreeCharset, TreeFormat, TreePrefix},
};
use crate::util::style::{HEADER, NOP};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, StatefulWidget, Tabs, Widget},
};

mod declaration;
mod graph;
mod package;
mod usage;

use graph::GraphIndex;
use usage::UsageTree;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MetadataTab {
    #[default]
    Package,
    Usage,
}

impl MetadataTab {
    fn index(self) -> usize {
        match self {
            Self::Package => 0,
            Self::Usage => 1,
        }
    }

    pub fn toggle(&mut self) {
        *self = match self {
            Self::Package => Self::Usage,
            Self::Usage => Self::Package,
        };
    }
}

#[derive(Debug, Default)]
pub struct MetadataPopupState {
    pub tab: MetadataTab,
    pub scroll: usize,
    pub(crate) page_size: usize,
    graph_index: Option<GraphIndex>,
    usage_tree: Option<UsageTree>,
}

impl MetadataPopupState {
    pub fn invalidate_usage_tree(&mut self) {
        self.usage_tree = None;
        self.graph_index = None;
    }

    pub fn move_usage_selection(&mut self, down: bool) {
        let Some(usage_tree) = &mut self.usage_tree else {
            return;
        };
        if down {
            usage_tree.state.select_next(&usage_tree.tree);
        } else {
            usage_tree.state.select_previous(&usage_tree.tree);
        }
    }
}

pub struct MetadataPopup<'a> {
    dependency: Option<&'a Dependency>,
    usages: Option<&'a [DependencyUsage]>,
    source_tree: &'a TuiTree,
    prefix: TreePrefix,
    charset: TreeCharset,
    format: &'a TreeFormat,
}

impl<'a> MetadataPopup<'a> {
    pub fn new(
        dependency: Option<&'a Dependency>,
        usages: Option<&'a [DependencyUsage]>,
        source_tree: &'a TuiTree,
        prefix: TreePrefix,
        charset: TreeCharset,
        format: &'a TreeFormat,
    ) -> Self {
        Self {
            dependency,
            usages,
            source_tree,
            prefix,
            charset,
            format,
        }
    }
}

impl StatefulWidget for MetadataPopup<'_> {
    type State = MetadataPopupState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let width = (area.width.saturating_mul(4) / 5).min(100);
        let height = area.height.saturating_sub(2).min(30);
        if width < 4 || height < 3 {
            return;
        }
        let popup_area = Rect {
            x: area.x + area.width.saturating_sub(width) / 2,
            y: area.y + area.height.saturating_sub(height).saturating_sub(2),
            width,
            height,
        };
        Clear.render(popup_area, buf);
        let title_style = Style::from(HEADER).add_modifier(Modifier::BOLD | Modifier::REVERSED);
        let block = Block::default()
            .title(Line::from(Span::styled(
                " DEPENDENCY METADATA ",
                title_style,
            )))
            .borders(Borders::ALL)
            .border_style(Style::from(HEADER));
        let inner = block.inner(popup_area);
        let body_area = Rect {
            x: inner.x.saturating_add(1),
            width: inner.width.saturating_sub(2),
            ..inner
        };
        let content_area = Rect {
            width: body_area.width.saturating_sub(1),
            ..body_area
        };
        let scrollbar_area = Rect {
            x: body_area.right().saturating_sub(1),
            width: 1,
            ..body_area
        };
        block.render(popup_area, buf);
        let tab_titles = [" Package ", " Usage "];
        let tabs_width = (tab_titles
            .iter()
            .map(|title| Line::from(*title).width())
            .sum::<usize>()
            + 1) as u16;
        let tabs_area = Rect {
            x: popup_area
                .right()
                .saturating_sub(2)
                .saturating_sub(tabs_width),
            y: popup_area.y,
            width: tabs_width,
            height: 1,
        };
        Tabs::new(tab_titles)
            .select(state.tab.index())
            .highlight_style(Style::from(HEADER).add_modifier(Modifier::BOLD | Modifier::REVERSED))
            .padding("", "")
            .divider(Span::styled("│", Style::from(NOP)))
            .render(tabs_area, buf);
        match state.tab {
            MetadataTab::Package => {
                package::render(&self, content_area, scrollbar_area, buf, state)
            }
            MetadataTab::Usage => self.render_usage(content_area, buf, state),
        }
    }
}
