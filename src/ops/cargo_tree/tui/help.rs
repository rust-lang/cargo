use crate::util::style::{HEADER, NOP, VALID};
use ratatui::{
    buffer::Buffer,
    layout::{Rect, Size},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

const KEY_BINDINGS: &[(&str, &str)] = &[
    ("?", "Show this popup"),
    ("/", "Search"),
    ("i", "Invert selected"),
    ("e", "Select dependency edge kinds"),
    ("t", "Select target triples"),
    ("f", "Select features"),
    ("p", "Select packages"),
    ("m", "Toggle dependency metadata"),
    ("o", "Open crates.io page"),
    ("d", "Open docs.rs page"),
    ("M", "Edit package format"),
    ("c", "Toggle tree charset"),
    ("←", "Collapse selected"),
    ("→", "Expand selected"),
    ("space", "Toggle expand/collapse"),
    ("[", "Go to previous sibling"),
    ("]", "Go to next sibling"),
    ("P", "Go to parent"),
    ("q", "Quit"),
];

fn key_bindings() -> Text<'static> {
    let key_style = Style::from(VALID);
    let max_key_len = KEY_BINDINGS
        .iter()
        .map(|(key, _)| key.chars().count())
        .max()
        .unwrap_or(0);

    let lines = KEY_BINDINGS
        .iter()
        .map(|(key, desc)| {
            let padding = " ".repeat(max_key_len.saturating_sub(key.chars().count()) + 3);
            Line::from(vec![
                Span::raw(" "),
                Span::styled((*key).to_string(), key_style),
                Span::raw(padding),
                Span::raw((*desc).to_string()),
                Span::raw(" "),
            ])
        })
        .collect::<Vec<_>>();

    Text::from(lines)
}

#[derive(Debug)]
pub struct HelpPopupStyle {
    border: Style,
    title: Style,
    default: Style,
}

impl Default for HelpPopupStyle {
    fn default() -> Self {
        HelpPopupStyle {
            border: HEADER.into(),
            title: Style::from(HEADER)
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::REVERSED),
            default: NOP.into(),
        }
    }
}

#[derive(Debug)]
pub struct HelpPopup<'a> {
    title: Line<'a>,
    content: Text<'a>,
    style: HelpPopupStyle,
}

impl Default for HelpPopup<'_> {
    fn default() -> Self {
        HelpPopup {
            title: Line::from(" COMMANDS "),
            content: key_bindings(),
            style: HelpPopupStyle::default(),
        }
    }
}

impl<'a> HelpPopup<'a> {
    pub fn size(&self) -> Size {
        Size {
            width: (self.content.width() + 2) as u16,
            height: (self.content.height() + 2) as u16,
        }
    }
}

impl Widget for HelpPopup<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);

        let block = Block::new()
            .title(self.title)
            .title_style(self.style.title)
            .borders(Borders::ALL)
            .border_style(self.style.border);

        Paragraph::new(self.content)
            .style(self.style.default)
            .block(block)
            .render(area, buf);
    }
}
