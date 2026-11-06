pub mod app;
pub mod dependency;
pub mod help;
pub mod metadata;
pub mod selector;
pub mod state;
pub(crate) mod tree;
pub mod widget;

pub use tree::TuiTree;

use crate::util::style::{HEADER, USAGE};
use ratatui::{
    Frame,
    layout::{Position, Rect},
    style::{Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Paragraph, Scrollbar, ScrollbarOrientation},
};

use help::HelpPopup;
use metadata::MetadataPopup;
use selector::{SelectorPopup, SelectorState};
use state::{InputMode, TuiState};
use widget::TreeWidget;

pub fn draw_tui(frame: &mut Frame<'_>, state: &mut TuiState) {
    draw_tree(frame, frame.area(), state);
    if state.show_metadata {
        let dependency =
            TuiState::selected_dependency(&state.dependency_tree, &state.tree_widget_state);
        let usages = dependency
            .and_then(|dependency| dependency.package_id)
            .and_then(|package_id| state.dependency_tree.dependency_usages.get(&package_id))
            .map(Vec::as_slice);
        frame.render_stateful_widget(
            MetadataPopup::new(
                dependency,
                usages,
                &state.dependency_tree,
                state.prefix,
                state.charset,
                &state.format,
            ),
            frame.area(),
            &mut state.metadata_popup,
        );
    }
    draw_help_text(frame, frame.area());
    if state.show_help {
        draw_help_popup(frame);
    }
    if let Some(selector) = &state.selector {
        draw_selector_popup(frame, frame.area(), selector);
    }
}

pub fn draw_tree(frame: &mut Frame<'_>, area: Rect, state: &mut TuiState) {
    state.advance_spinner();
    let prompt = if matches!(
        state.input_mode,
        InputMode::Search | InputMode::SearchResults
    ) {
        Some(Line::from(vec![
            Span::raw(state.search_prompt_symbol().to_string()).bold(),
            Span::raw(state.search_query.as_str()),
        ]))
    } else {
        state.invert_prompt.as_deref().map(|prompt| {
            Line::from(vec![
                Span::styled("invert: ", Style::from(crate::util::style::VALID)),
                Span::raw(prompt),
            ])
        })
    };
    let prompt_width = prompt.as_ref().map_or(0, Line::width);

    let tree_widget = TreeWidget::new(&state.dependency_tree)
        .prefix(state.prefix)
        .charset(state.charset)
        .format(&state.format)
        .prompt(prompt)
        .scrollbar(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .track_symbol(Some("┆"))
                .thumb_symbol("▐")
                .begin_symbol(Some("▴"))
                .end_symbol(Some("▾")),
        );
    frame.render_stateful_widget(tree_widget, area, &mut state.tree_widget_state);

    if state.input_mode == InputMode::Search {
        frame.set_cursor_position(Position::new(
            area.x.saturating_add(prompt_width as u16),
            area.bottom().saturating_sub(2),
        ));
    }
}

pub fn draw_help_text(frame: &mut Frame<'_>, area: Rect) {
    let key_style = Style::from(HEADER)
        .add_modifier(Modifier::BOLD)
        .add_modifier(Modifier::REVERSED);

    let text = Line::from(vec![
        " q ".bold(),
        Span::styled(" QUIT ", key_style),
        " ? ".bold(),
        Span::styled(" HELP ", key_style),
    ]);

    let area = Rect {
        x: area.right().saturating_sub(text.width() as u16 + 2),
        y: area.bottom().saturating_sub(1),
        width: text.width() as u16,
        height: 1,
    };

    let paragraph = Paragraph::new(text).style(Style::from(USAGE));
    frame.render_widget(paragraph, area);
}

pub fn draw_help_popup(frame: &mut Frame<'_>) {
    let help_popup = HelpPopup::default();
    let size = help_popup.size();
    let area = Rect {
        x: frame.area().right().saturating_sub(size.width + 1),
        y: frame.area().bottom().saturating_sub(size.height + 1),
        width: size.width,
        height: size.height,
    };
    let area = frame.area().clamp(area);
    frame.render_widget(help_popup, area);
}

pub fn draw_selector_popup(frame: &mut Frame<'_>, area: Rect, selector: &SelectorState) {
    let popup = SelectorPopup::new(selector);
    let cursor_position = popup.cursor_position(area);
    frame.render_widget(popup, area);
    if let Some(position) = cursor_position {
        frame.set_cursor_position(position);
    }
}
