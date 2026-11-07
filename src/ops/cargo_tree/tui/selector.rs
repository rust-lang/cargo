use crate::util::style::{HEADER, NOP, VALID};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Position, Rect, Size},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Clear, List, ListItem, ListState, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState, StatefulWidget, Tabs, Widget,
    },
};

#[derive(Debug, Clone)]
pub struct SelectorOption {
    pub value: String,
    pub label: String,
    pub checked: bool,
    pub exclusive: bool,
    tab: Option<usize>,
}

impl SelectorOption {
    pub fn new(value: impl Into<String>, label: impl Into<String>, checked: bool) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            checked,
            exclusive: false,
            tab: None,
        }
    }

    pub fn exclusive(mut self) -> Self {
        self.exclusive = true;
        self
    }

    pub fn in_tab(mut self, tab: usize) -> Self {
        self.tab = Some(tab);
        self
    }
}

/// Reusable state for a popup that selects multiple named options.
#[derive(Debug, Clone)]
pub struct SelectorState {
    pub title: String,
    pub options: Vec<SelectorOption>,
    pub selected: usize,
    pub applying: bool,
    pub message: Option<String>,
    pub filter: Option<String>,
    pub editor: Option<String>,
    editor_cursor: usize,
    tabs: Option<Vec<String>>,
    selected_tab: usize,
    max_height: usize,
    visible_options: Vec<usize>,
}

impl SelectorState {
    pub fn new(title: impl Into<String>, options: Vec<SelectorOption>) -> Self {
        let visible_options = (0..options.len()).collect();
        Self {
            title: title.into(),
            options,
            selected: 0,
            applying: false,
            message: None,
            filter: None,
            editor: None,
            editor_cursor: 0,
            tabs: None,
            selected_tab: 0,
            max_height: 14,
            visible_options,
        }
    }

    pub fn filterable(mut self) -> Self {
        self.filter = Some(String::new());
        self
    }

    pub fn editor(mut self, value: impl Into<String>) -> Self {
        let value = value.into();
        self.editor_cursor = value.chars().count();
        self.editor = Some(value);
        self
    }

    pub fn editor_cursor(&self) -> usize {
        self.editor_cursor
    }

    pub fn set_editor(&mut self, value: String) {
        self.editor_cursor = value.chars().count();
        self.editor = Some(value);
    }

    pub fn insert_editor_char(&mut self, ch: char) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        let byte_index = editor
            .char_indices()
            .nth(self.editor_cursor)
            .map_or(editor.len(), |(index, _)| index);
        editor.insert(byte_index, ch);
        self.editor_cursor += 1;
    }

    pub fn backspace_editor(&mut self) {
        if self.editor_cursor == 0 {
            return;
        }
        let Some(editor) = &mut self.editor else {
            return;
        };
        let byte_start = editor
            .char_indices()
            .nth(self.editor_cursor - 1)
            .map(|(index, _)| index);
        let byte_end = editor
            .char_indices()
            .nth(self.editor_cursor)
            .map_or(editor.len(), |(index, _)| index);
        if let Some(byte_start) = byte_start {
            editor.replace_range(byte_start..byte_end, "");
            self.editor_cursor -= 1;
        }
    }

    pub fn delete_editor_char(&mut self) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        let Some((byte_start, _)) = editor.char_indices().nth(self.editor_cursor) else {
            return;
        };
        let byte_end = editor
            .char_indices()
            .nth(self.editor_cursor + 1)
            .map_or(editor.len(), |(index, _)| index);
        editor.replace_range(byte_start..byte_end, "");
    }

    pub fn move_editor_cursor_left(&mut self) {
        self.editor_cursor = self.editor_cursor.saturating_sub(1);
    }

    pub fn move_editor_cursor_right(&mut self) {
        let char_count = self
            .editor
            .as_ref()
            .map_or(0, |value| value.chars().count());
        self.editor_cursor = self.editor_cursor.saturating_add(1).min(char_count);
    }

    pub fn move_editor_cursor_start(&mut self) {
        self.editor_cursor = 0;
    }

    pub fn move_editor_cursor_end(&mut self) {
        self.editor_cursor = self
            .editor
            .as_ref()
            .map_or(0, |value| value.chars().count());
    }

    pub fn max_height(mut self, max_height: usize) -> Self {
        self.max_height = max_height;
        self
    }

    pub fn tabs(mut self, tabs: Vec<String>) -> Self {
        self.tabs = Some(tabs);
        self.refresh_visible_options();
        self
    }

    pub fn has_tabs(&self) -> bool {
        self.tabs.is_some()
    }

    pub fn selected_tab(&self) -> usize {
        self.selected_tab
    }

    pub fn tab_titles(&self) -> Option<&[String]> {
        self.tabs.as_deref()
    }

    pub fn next_tab(&mut self) {
        if let Some(tabs) = &self.tabs
            && !tabs.is_empty()
        {
            self.selected_tab = (self.selected_tab + 1) % tabs.len();
            self.selected = 0;
            self.refresh_visible_options();
        }
    }

    pub fn checked_values(&self) -> Vec<String> {
        self.options
            .iter()
            .filter(|option| option.checked)
            .map(|option| option.value.clone())
            .collect()
    }

    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn move_down(&mut self) {
        self.selected = self
            .selected
            .saturating_add(1)
            .min(self.visible_options.len().saturating_sub(1));
    }

    pub fn push_filter_char(&mut self, ch: char) {
        if let Some(filter) = &mut self.filter {
            filter.push(ch);
            self.selected = 0;
            self.refresh_visible_options();
        }
    }

    pub fn pop_filter_char(&mut self) {
        if let Some(filter) = &mut self.filter
            && filter.pop().is_some()
        {
            self.selected = 0;
            self.refresh_visible_options();
        }
    }

    pub fn visible_options(&self) -> &[usize] {
        &self.visible_options
    }

    fn refresh_visible_options(&mut self) {
        let filter = self.filter.as_deref().unwrap_or_default().to_lowercase();
        self.visible_options = self
            .options
            .iter()
            .enumerate()
            .filter_map(|(index, option)| {
                (option.tab.is_none_or(|tab| tab == self.selected_tab)
                    && option.label.to_lowercase().contains(&filter))
                .then_some(index)
            })
            .collect();
        self.selected = self
            .selected
            .min(self.visible_options.len().saturating_sub(1));
    }

    pub fn toggle_selected(&mut self) {
        let Some(&option_index) = self.visible_options.get(self.selected) else {
            return;
        };
        let option = &self.options[option_index];
        let was_checked = option.checked;
        let is_exclusive = option.exclusive;

        if is_exclusive && !was_checked {
            for option in &mut self.options {
                option.checked = false;
            }
            self.options[option_index].checked = true;
        } else {
            self.options[option_index].checked = !was_checked;
            if !is_exclusive && !was_checked {
                for option in &mut self.options {
                    if option.exclusive {
                        option.checked = false;
                    }
                }
            }
        }
    }
}

pub struct SelectorPopup<'a> {
    state: &'a SelectorState,
}

impl<'a> SelectorPopup<'a> {
    pub fn new(state: &'a SelectorState) -> Self {
        Self { state }
    }

    fn size(&self, area: Rect) -> Size {
        let options_width = self
            .state
            .options
            .iter()
            .map(|option| option.label.chars().count().saturating_add(4))
            .max()
            .unwrap_or(0);
        let message_width = self
            .state
            .message
            .as_ref()
            .map_or(0, |message| message.chars().count().saturating_add(2));
        let filter_width = self.state.filter.as_ref().map_or(0, |filter| {
            "Filter: ".chars().count() + filter.chars().count() + 2
        });
        let editor_width = self.state.editor.as_ref().map_or(0, |editor| {
            Line::from(" Format: ").width() + Line::from(editor.as_str()).width() + 2
        });
        let tabs_width = self.state.tab_titles().map_or(0, |tabs| {
            tabs.iter()
                .map(|tab| Line::from(tab.as_str()).width())
                .sum::<usize>()
                .saturating_add(tabs.len().saturating_mul(2))
                + tabs.len().saturating_sub(1)
        });
        let title_width = self.state.tab_titles().map_or(0, |_| {
            Line::from(format!(" {} ", self.state.title.to_uppercase())).width()
        });
        let header_width = title_width.saturating_add(tabs_width).saturating_add(2);
        let footer_width = if self.state.applying {
            "Resolving dependency graph...".len()
        } else if self.state.editor.is_some() {
            "Type edit format   Backspace erase   Space toggle   Enter apply   Esc cancel".len()
        } else if self.state.filter.is_some() {
            if self.state.has_tabs() {
                "Tab switch   Type filter   Backspace erase   Space toggle   Enter apply   Esc cancel"
                    .len()
            } else {
                "Type filter   Backspace erase   Space toggle   Enter apply   Esc cancel".len()
            }
        } else {
            if self.state.has_tabs() {
                "Tab switch   Space toggle   Enter apply   Esc cancel".len()
            } else {
                "Space toggle   Enter apply   Esc cancel".len()
            }
        }
        .saturating_add(2);
        let width = options_width
            .max(filter_width)
            .max(editor_width)
            .max(tabs_width)
            .max(header_width)
            .max(message_width)
            .max(footer_width)
            .saturating_add(2)
            .min(usize::from(area.width));
        let height = self
            .state
            .visible_options()
            .len()
            .saturating_add(5)
            .saturating_add(usize::from(self.state.filter.is_some()))
            .saturating_add(usize::from(self.state.editor.is_some()))
            .saturating_add(usize::from(self.state.message.is_some()))
            .min(self.state.max_height)
            .min(usize::from(area.height));
        Size::new(width as u16, height as u16)
    }

    fn editor_cursor_column(&self) -> u16 {
        let Some(editor) = &self.state.editor else {
            return 0;
        };
        let before_cursor = editor
            .chars()
            .take(self.state.editor_cursor())
            .collect::<String>();
        Line::from(before_cursor).width().min(usize::from(u16::MAX)) as u16
    }

    fn editor_scroll_offset(&self, viewport_width: u16) -> u16 {
        self.editor_cursor_column()
            .saturating_sub(viewport_width.saturating_sub(1))
    }

    pub fn cursor_position(&self, area: Rect) -> Option<Position> {
        if self.state.applying {
            return None;
        }

        let size = self.size(area);
        let popup_area = Rect {
            x: area.x + area.width.saturating_sub(size.width) / 2,
            y: area.y + area.height.saturating_sub(size.height) / 2,
            width: size.width,
            height: size.height,
        };
        if self.state.editor.is_some() {
            let editor_area_width = popup_area.width.saturating_sub(11);
            let cursor_column = self
                .editor_cursor_column()
                .saturating_sub(self.editor_scroll_offset(editor_area_width));
            let x = popup_area
                .x
                .saturating_add(10)
                .saturating_add(cursor_column)
                .min(popup_area.right().saturating_sub(2));
            let y = popup_area
                .y
                .saturating_add(1 + u16::from(self.state.filter.is_some()));
            return Some(Position::new(x, y));
        }

        let filter = self.state.filter.as_ref()?;
        let x = popup_area
            .x
            .saturating_add(2)
            .saturating_add(Line::from("Filter: ").width() as u16)
            .saturating_add(Line::from(filter.as_str()).width() as u16)
            .min(popup_area.right().saturating_sub(2));
        let y = popup_area.y.saturating_add(1);
        Some(Position::new(x, y))
    }
}

impl Widget for SelectorPopup<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let size = self.size(area);
        let popup_area = Rect {
            x: area.x + area.width.saturating_sub(size.width) / 2,
            y: area.y + area.height.saturating_sub(size.height) / 2,
            width: size.width,
            height: size.height,
        };
        Clear.render(popup_area, buf);

        let title = format!(" {} ", self.state.title.to_uppercase());
        let block = Block::new()
            .title(title.as_str())
            .title_style(
                Style::from(HEADER)
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::REVERSED),
            )
            .borders(Borders::ALL)
            .border_style(Style::from(HEADER));
        let selected_style = Style::from(HEADER).add_modifier(Modifier::BOLD);
        let inner = block.inner(popup_area);
        block.render(popup_area, buf);
        if let Some(tabs) = self.state.tab_titles() {
            let tab_titles = tabs.iter().enumerate().map(|(index, title)| {
                let style = if index == self.state.selected_tab() {
                    Style::from(HEADER).add_modifier(Modifier::BOLD | Modifier::REVERSED)
                } else {
                    Style::from(NOP).add_modifier(Modifier::DIM)
                };
                Line::from(Span::styled(format!(" {title} "), style))
            });
            let tabs_width = tabs
                .iter()
                .map(|tab| Line::from(tab.as_str()).width())
                .sum::<usize>()
                .saturating_add(tabs.len().saturating_mul(2))
                .saturating_add(tabs.len().saturating_sub(1))
                .min(usize::from(u16::MAX)) as u16;
            let tabs_x = popup_area
                .right()
                .saturating_sub(2)
                .saturating_sub(tabs_width);
            let tabs_area = Rect {
                x: tabs_x,
                y: popup_area.y,
                width: tabs_width,
                height: 1,
            };
            Tabs::new(tab_titles)
                .select(self.state.selected_tab())
                .highlight_style(
                    Style::from(HEADER).add_modifier(Modifier::BOLD | Modifier::REVERSED),
                )
                .padding("", "")
                .divider(Span::styled("│", Style::from(NOP)))
                .render(tabs_area, buf);
        }

        let items = self
            .state
            .visible_options()
            .iter()
            .map(|&index| {
                let option = &self.state.options[index];
                let checkbox = if option.checked { "✔" } else { "□" };
                ListItem::new(Line::from(vec![
                    Span::raw(" "),
                    Span::raw(checkbox),
                    Span::raw(" "),
                    Span::raw(option.label.as_str()),
                    Span::raw(" "),
                ]))
            })
            .collect::<Vec<_>>();

        let list = List::new(items)
            .highlight_style(selected_style)
            .highlight_symbol("");
        let mut list_state = ListState::default().with_selected(
            (!self.state.visible_options().is_empty()).then_some(self.state.selected),
        );
        let footer_height = 1 + u16::from(self.state.message.is_some());
        let mut constraints = Vec::with_capacity(6);
        if self.state.filter.is_some() {
            constraints.push(Constraint::Length(1));
        }
        if self.state.editor.is_some() {
            constraints.push(Constraint::Length(1));
        }
        constraints.extend([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(footer_height),
        ]);
        let areas = Layout::vertical(constraints).split(inner);
        let mut row = 0;
        if let Some(filter) = &self.state.filter {
            let filter_line = Line::from(vec![
                Span::raw(" "),
                Span::styled("Filter: ", Style::from(VALID)),
                Span::raw(filter.as_str()),
            ]);
            Paragraph::new(filter_line)
                .style(Style::from(NOP))
                .render(areas[row], buf);
            row += 1;
        }
        if let Some(editor) = &self.state.editor {
            let editor_areas =
                Layout::horizontal([Constraint::Length(9), Constraint::Min(0)]).split(areas[row]);
            Paragraph::new(Line::from(vec![
                Span::raw(" "),
                Span::styled("Format: ", Style::from(VALID)),
            ]))
            .render(editor_areas[0], buf);
            Paragraph::new(editor.as_str())
                .scroll((0, self.editor_scroll_offset(editor_areas[1].width)))
                .style(selected_style)
                .render(editor_areas[1], buf);
            row += 1;
        }
        let list_area = areas[row + 1];
        let footer_area = areas[row + 3];
        StatefulWidget::render(list, list_area, buf, &mut list_state);
        if list_area.height > 0
            && self.state.visible_options().len() > usize::from(list_area.height)
        {
            let mut scrollbar_state = ScrollbarState::new(
                self.state
                    .visible_options()
                    .len()
                    .saturating_sub(usize::from(list_area.height)),
            )
            .position(list_state.offset())
            .viewport_content_length(usize::from(list_area.height));
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .track_symbol(Some("│"))
                .thumb_symbol("█")
                .begin_symbol(Some("▲"))
                .end_symbol(Some("▼"))
                .track_style(Style::from(NOP).add_modifier(Modifier::DIM))
                .thumb_style(Style::from(HEADER).add_modifier(Modifier::BOLD))
                .begin_style(Style::from(NOP))
                .end_style(Style::from(NOP))
                .render(list_area, buf, &mut scrollbar_state);
        }

        let mut footer_lines = Vec::with_capacity(footer_height as usize);
        if let Some(message) = &self.state.message {
            footer_lines.push(Line::from(format!(" {message} ")));
        }
        let footer = if self.state.applying {
            Line::from("Resolving dependency graph...")
        } else if self.state.editor.is_some() {
            let key_style = Style::from(VALID);
            Line::from(vec![
                Span::raw(" "),
                Span::styled("Type", key_style),
                Span::raw(" edit format   "),
                Span::styled("Backspace", key_style),
                Span::raw(" erase   "),
                Span::styled("Space", key_style),
                Span::raw(" toggle   "),
                Span::styled("Enter", key_style),
                Span::raw(" apply   "),
                Span::styled("Esc", key_style),
                Span::raw(" cancel "),
            ])
        } else if self.state.has_tabs() && self.state.filter.is_some() {
            let key_style = Style::from(VALID);
            Line::from(vec![
                Span::raw(" "),
                Span::styled("Tab", key_style),
                Span::raw(" switch   "),
                Span::styled("Type", key_style),
                Span::raw(" filter   "),
                Span::styled("Backspace", key_style),
                Span::raw(" erase   "),
                Span::styled("Space", key_style),
                Span::raw(" toggle   "),
                Span::styled("Enter", key_style),
                Span::raw(" apply   "),
                Span::styled("Esc", key_style),
                Span::raw(" cancel "),
            ])
        } else if self.state.has_tabs() {
            let key_style = Style::from(VALID);
            Line::from(vec![
                Span::raw(" "),
                Span::styled("Tab", key_style),
                Span::raw(" switch   "),
                Span::styled("Space", key_style),
                Span::raw(" toggle   "),
                Span::styled("Enter", key_style),
                Span::raw(" apply   "),
                Span::styled("Esc", key_style),
                Span::raw(" cancel "),
            ])
        } else if self.state.filter.is_some() {
            let key_style = Style::from(VALID);
            Line::from(vec![
                Span::raw(" "),
                Span::styled("Type", key_style),
                Span::raw(" filter   "),
                Span::styled("Backspace", key_style),
                Span::raw(" erase   "),
                Span::styled("Space", key_style),
                Span::raw(" toggle   "),
                Span::styled("Enter", key_style),
                Span::raw(" apply   "),
                Span::styled("Esc", key_style),
                Span::raw(" cancel "),
            ])
        } else {
            let key_style = Style::from(VALID);
            Line::from(vec![
                Span::raw(" "),
                Span::styled("Space", key_style),
                Span::raw(" toggle   "),
                Span::styled("Enter", key_style),
                Span::raw(" apply   "),
                Span::styled("Esc", key_style),
                Span::raw(" cancel "),
            ])
        }
        .style(Style::from(NOP));
        footer_lines.push(footer);

        Paragraph::new(footer_lines)
            .style(Style::from(NOP))
            .render(footer_area, buf);
    }
}
