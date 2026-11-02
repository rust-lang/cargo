//! Render package metadata and resolved graph statistics.

use std::borrow::Cow;

use super::graph::GraphIndex;
use super::{MetadataPopup, MetadataPopupState};
use crate::util::style::{HEADER, NOP, VALID, WARN};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget, Widget, Wrap,
    },
};

pub(super) fn render(
    popup: &MetadataPopup<'_>,
    content_area: Rect,
    scrollbar_area: Rect,
    buf: &mut Buffer,
    state: &mut MetadataPopupState,
) {
    if state.graph_index.is_none() {
        state.graph_index = Some(GraphIndex::new(popup.source_tree));
    }
    let lines = popup.package_lines(
        state
            .graph_index
            .as_mut()
            .expect("graph index was initialized"),
    );
    let content_height = lines
        .lines
        .iter()
        .map(|line| {
            line.width()
                .max(1)
                .div_ceil((content_area.width as usize).max(1))
        })
        .sum::<usize>();
    state.page_size = content_area.height as usize;
    let paragraph = Paragraph::new(lines)
        .style(Style::from(NOP))
        .wrap(Wrap { trim: true });
    let max_scroll = content_height.saturating_sub(content_area.height as usize);
    state.scroll = state.scroll.min(max_scroll);
    let mut scrollbar_state = ScrollbarState::new(max_scroll)
        .position(state.scroll)
        .viewport_content_length(content_area.height as usize);
    paragraph
        .scroll((state.scroll.min(u16::MAX as usize) as u16, 0))
        .render(content_area, buf);
    if max_scroll > 0 {
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .track_symbol(Some("┆"))
            .thumb_symbol("▐")
            .render(scrollbar_area, buf, &mut scrollbar_state);
    }
}

impl<'a> MetadataPopup<'a> {
    pub(super) fn package_lines(&self, graph_index: &mut GraphIndex) -> Text<'_> {
        let Some(dependency) = self.dependency else {
            return Text::from(" No dependency selected ");
        };

        let metadata = dependency.format_metadata.as_deref();
        let mut lines = Vec::with_capacity(24);
        push_field_styled(&mut lines, "Name", &dependency.name, Style::from(WARN));
        push_field_styled(&mut lines, "Version", &dependency.version, Style::default());
        push_field_styled(
            &mut lines,
            "Edition",
            metadata
                .and_then(|metadata| metadata.edition.as_deref())
                .unwrap_or("Not specified"),
            Style::default(),
        );
        push_field_styled(
            &mut lines,
            "Rust version",
            metadata
                .and_then(|metadata| metadata.rust_version.as_deref())
                .unwrap_or("Not specified"),
            Style::default(),
        );
        push_field_styled(
            &mut lines,
            "Targets",
            metadata
                .and_then(|metadata| metadata.targets.as_deref())
                .unwrap_or("Not specified"),
            Style::default(),
        );

        if let Some(package_id) = &dependency.package_id {
            let source = package_id.source_id();
            let source = if source.is_crates_io() {
                "crates.io"
            } else {
                metadata
                    .and_then(|metadata| metadata.source.as_deref())
                    .unwrap_or("Not specified")
            };
            push_field_styled(&mut lines, "Source", source, Style::default());
        }

        push_field_styled(
            &mut lines,
            "Description",
            metadata
                .and_then(|metadata| metadata.description.as_deref())
                .unwrap_or("Not specified"),
            Style::default(),
        );
        push_field_styled(
            &mut lines,
            "License",
            metadata
                .and_then(|metadata| metadata.license.as_deref())
                .unwrap_or("Not specified"),
            Style::default(),
        );
        push_link_field(
            &mut lines,
            "Repository",
            metadata.and_then(|metadata| metadata.repository.as_deref()),
        );
        push_link_field(
            &mut lines,
            "Homepage",
            metadata.and_then(|metadata| metadata.homepage.as_deref()),
        );
        push_link_field(
            &mut lines,
            "Documentation",
            metadata.and_then(|metadata| metadata.documentation.as_deref()),
        );
        push_field_styled(
            &mut lines,
            "Authors",
            metadata
                .and_then(|metadata| metadata.authors.as_deref())
                .unwrap_or("Not specified"),
            Style::default(),
        );
        push_field_styled(
            &mut lines,
            "Keywords",
            metadata
                .and_then(|metadata| metadata.keywords.as_deref())
                .unwrap_or("Not specified"),
            Style::default(),
        );
        push_field_styled(
            &mut lines,
            "Categories",
            metadata
                .and_then(|metadata| metadata.categories.as_deref())
                .unwrap_or("Not specified"),
            Style::default(),
        );
        if let Some(library) = metadata.and_then(|metadata| metadata.library.as_deref()) {
            push_field_styled(&mut lines, "Library", library, Style::default());
        }
        if dependency.is_proc_macro {
            push_field_styled(&mut lines, "Target", "proc-macro", Style::default());
        }
        if let Some(path) = &dependency.manifest_dir {
            push_field_styled(&mut lines, "Manifest", path.as_str(), Style::default());
        }

        if let Some(package_id) = dependency.package_id {
            lines.push(Line::from("").style(Style::from(NOP)));
            lines.push(Line::from(Span::styled(
                "Resolved in this graph",
                Style::from(HEADER).add_modifier(Modifier::BOLD),
            )));
            let mut versions = graph_index
                .versions_by_name
                .get(&dependency.name)
                .cloned()
                .unwrap_or_else(|| vec![dependency.version.clone()]);
            if let Some(current) = versions
                .iter()
                .position(|version| version == &dependency.version)
            {
                versions.swap(0, current);
            }
            let version_count = versions.len();
            let version_count_style = if version_count > 1 {
                Style::from(WARN)
            } else {
                Style::default()
            };
            lines.push(Line::from(vec![
                Span::styled("Versions in graph", Style::from(VALID)),
                Span::raw(" ".repeat(18 - "Versions in graph".len() + 1)),
                Span::styled(version_count.to_string(), version_count_style),
                Span::raw(format!(" ({})", versions.join(", "))),
            ]));
            let dependents = graph_index.dependents(package_id, self.source_tree);
            push_field_styled(
                &mut lines,
                "Dependents",
                format!("{} direct / {} total", dependents.direct, dependents.total),
                Style::default(),
            );
            push_field_styled(
                &mut lines,
                "Enabled features",
                metadata
                    .and_then(|metadata| metadata.features.as_deref())
                    .map(|features| features.split(',').collect::<Vec<_>>().join(", "))
                    .unwrap_or_else(|| "none".to_owned()),
                Style::default(),
            );
        }

        Text::from(lines)
    }
}

fn push_field_styled<'a>(
    lines: &mut Vec<Line<'a>>,
    label: &'static str,
    value: impl Into<Cow<'a, str>>,
    value_style: Style,
) {
    const LABEL_COLUMN_WIDTH: usize = 18;
    let padding = LABEL_COLUMN_WIDTH.saturating_sub(label.len()) + 1;
    lines.push(Line::from(vec![
        Span::styled(label, Style::from(VALID)),
        Span::raw(" ".repeat(padding)),
        Span::raw(value).style(value_style),
    ]));
}

fn push_link_field<'a>(lines: &mut Vec<Line<'a>>, label: &'static str, value: Option<&'a str>) {
    push_field_styled(
        lines,
        label,
        value.unwrap_or("Not specified"),
        Style::default(),
    );
    if let Some(value) = lines.last_mut().and_then(|line| line.spans.last_mut()) {
        value.style = value.style.add_modifier(Modifier::ITALIC);
    }
}
