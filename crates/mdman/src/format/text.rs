//! Text formatter.

use crate::util::{header_text, unwrap};
use crate::EventIter;
use anyhow::{bail, Error};
use pulldown_cmark::{Alignment, Event, HeadingLevel, LinkType, Tag};
use std::fmt::Write;
use std::mem;
use url::Url;

pub struct TextFormatter {
    url: Option<Url>,
}

impl TextFormatter {
    pub fn new(url: Option<Url>) -> TextFormatter {
        TextFormatter { url }
    }
}

impl super::Formatter for TextFormatter {
    fn render(&self, input: &str) -> Result<String, Error> {
        TextRenderer::render(input, self.url.clone(), 0)
    }

    fn render_options_start(&self) -> &'static str {
        // Tell pulldown_cmark to ignore this.
        // This will be stripped out later.
        "<![CDATA["
    }

    fn render_options_end(&self) -> &'static str {
        "]]>"
    }

    fn render_option(
        &self,
        params: &[&str],
        block: &str,
        _man_name: &str,
    ) -> Result<String, Error> {
        let rendered_options = params
            .iter()
            .map(|param| TextRenderer::render(param, self.url.clone(), 0))
            .collect::<Result<Vec<_>, Error>>()?;
        let trimmed: Vec<_> = rendered_options.iter().map(|o| o.trim()).collect();
        // Wrap in HTML tags, they will be stripped out during rendering.
        Ok(format!(
            "<dt>{}</dt>\n<dd>{}</dd>\n<br>\n",
            trimmed.join(", "),
            block
        ))
    }

    fn linkify_man_to_md(&self, name: &str, section: u8) -> Result<String, Error> {
        Ok(format!("`{}`({})", name, section))
    }
}

struct TextRenderer<'e> {
    output: String,
    indent: usize,
    /// The current line being written. Once a line break is encountered (such
    /// as starting a new paragraph), this will be written to `output` via
    /// `flush`.
    line: String,
    /// The current word being written. Once a break is encountered (such as a
    /// space) this will be written to `line` via `flush_word`.
    word: String,
    parser: EventIter<'e>,
    /// The base URL used for relative URLs.
    url: Option<Url>,
    table: Table,
}

impl<'e> TextRenderer<'e> {
    fn render(input: &str, url: Option<Url>, indent: usize) -> Result<String, Error> {
        let parser = crate::md_parser(input, url.clone());
        let output = String::with_capacity(input.len() * 3 / 2);
        let mut mr = TextRenderer {
            output,
            indent,
            line: String::new(),
            word: String::new(),
            parser,
            url,
            table: Table::new(),
        };
        mr.push_md()?;
        Ok(mr.output)
    }

    fn push_md(&mut self) -> Result<(), Error> {
        // If this is true, this is inside a cdata block used for hiding
        // content from pulldown_cmark.
        let mut in_cdata = false;
        // The current list stack. None if unordered, Some if ordered with the
        // given number as the current index.
        let mut list: Vec<Option<u64>> = Vec::new();
        // Used in some cases where spacing isn't desired.
        let mut suppress_paragraph = false;
        // Whether or not word-wrapping is enabled.
        let mut wrap_text = true;

        while let Some((event, range)) = self.parser.next() {
            let this_suppress_paragraph = suppress_paragraph;
            // Always reset suppression, even if the next event isn't a
            // paragraph. This is in essence, a 1-token lookahead where the
            // suppression is only enabled if the next event is a paragraph.
            suppress_paragraph = false;
            match event {
                Event::Start(tag) => {
                    match tag {
                        Tag::Paragraph => {
                            if !this_suppress_paragraph {
                                self.flush();
                            }
                        }
                        Tag::Heading(level, ..) => {
                            self.flush();
                            if level == HeadingLevel::H1 {
                                let text = header_text(&mut self.parser)?;
                                self.push_to_line(&text.to_uppercase());
                                self.hard_break();
                                self.hard_break();
                            } else if level == HeadingLevel::H2 {
                                let text = header_text(&mut self.parser)?;
                                self.push_to_line(&text.to_uppercase());
                                self.flush();
                                self.indent = 7;
                            } else {
                                let text = header_text(&mut self.parser)?;
                                self.push_indent((level as usize - 2) * 3);
                                self.push_to_line(&text);
                                self.flush();
                                self.indent = (level as usize - 1) * 3 + 1;
                            }
                        }
                        Tag::BlockQuote => {
                            self.indent += 3;
                        }
                        Tag::CodeBlock(_kind) => {
                            self.flush();
                            wrap_text = false;
                            self.indent += 4;
                        }
                        Tag::List(start) => list.push(start),
                        Tag::Item => {
                            self.flush();
                            match list.last_mut().expect("item must have list start") {
                                // Ordered list.
                                Some(n) => {
                                    self.push_indent(self.indent);
                                    write!(self.line, "{}.", n)?;
                                    *n += 1;
                                }
                                // Unordered list.
                                None => {
                                    self.push_indent(self.indent);
                                    self.push_to_line("o ")
                                }
                            }
                            self.indent += 3;
                            suppress_paragraph = true;
                        }
                        Tag::FootnoteDefinition(_label) => unimplemented!(),
                        Tag::Table(alignment) => {
                            assert!(self.table.alignment.is_empty());
                            self.flush();
                            self.table.alignment.extend(alignment);
                            let table = self.table.process(&mut self.parser, self.indent)?;
                            self.output.push_str(&table);
                            self.hard_break();
                            self.table = Table::new();
                        }
                        Tag::TableHead | Tag::TableRow | Tag::TableCell => {
                            bail!("unexpected table element")
                        }
                        Tag::Emphasis => {}
                        Tag::Strong => {}
                        // Strikethrough isn't usually supported for TTY.
                        Tag::Strikethrough => self.word.push_str("~~"),
                        Tag::Link(link_type, dest_url, _title) => {
                            if dest_url.starts_with('#') {
                                // In a man page, page-relative anchors don't
                                // have much meaning.
                                continue;
                            }
                            match link_type {
                                LinkType::Autolink | LinkType::Email => {
                                    // The text is a copy of the URL, which is not needed.
                                    match self.parser.next() {
                                        Some((Event::Text(_), _range)) => {}
                                        _ => bail!("expected text after autolink"),
                                    }
                                }
                                LinkType::Inline
                                | LinkType::Reference
                                | LinkType::Collapsed
                                | LinkType::Shortcut => {}
                                // This is currently unused. This is only
                                // emitted with a broken link callback, but I
                                // felt it is too annoying to escape `[` in
                                // option descriptions.
                                LinkType::ReferenceUnknown
                                | LinkType::CollapsedUnknown
                                | LinkType::ShortcutUnknown => {
                                    bail!(
                                        "link with missing reference `{}` located at offset {}",
                                        dest_url,
                                        range.start
                                    );
                                }
                            }
                        }
                        Tag::Image(_link_type, _dest_url, _title) => {
                            bail!("images are not currently supported")
                        }
                    }
                }
                Event::End(tag) => match &tag {
                    Tag::Paragraph => {
                        self.flush();
                        self.hard_break();
                    }
                    Tag::Heading(..) => {}
                    Tag::BlockQuote => {
                        self.indent -= 3;
                    }
                    Tag::CodeBlock(_kind) => {
                        self.hard_break();
                        wrap_text = true;
                        self.indent -= 4;
                    }
                    Tag::List(_) => {
                        list.pop();
                    }
                    Tag::Item => {
                        self.flush();
                        self.indent -= 3;
                        self.hard_break();
                    }
                    Tag::FootnoteDefinition(_label) => {}
                    Tag::Table(_) => {}
                    Tag::TableHead => {}
                    Tag::TableRow => {}
                    Tag::TableCell => {}
                    Tag::Emphasis => {}
                    Tag::Strong => {}
                    Tag::Strikethrough => self.word.push_str("~~"),
                    Tag::Link(link_type, dest_url, _title) => {
                        if dest_url.starts_with('#') {
                            continue;
                        }
                        match link_type {
                            LinkType::Autolink | LinkType::Email => {}
                            LinkType::Inline
                            | LinkType::Reference
                            | LinkType::Collapsed
                            | LinkType::Shortcut => self.flush_word(),
                            _ => {
                                panic!("unexpected tag {:?}", tag);
                            }
                        }
                        self.flush_word();
                        write!(self.word, "<{}>", dest_url)?;
                    }
                    Tag::Image(_link_type, _dest_url, _title) => {}
                },
                Event::Text(t) | Event::Code(t) => {
                    if wrap_text {
                        let chunks = split_chunks(&t);
                        for chunk in chunks {
                            if chunk == " " {
                                self.flush_word();
                            } else {
                                self.word.push_str(chunk);
                            }
                        }
                    } else {
                        for line in t.lines() {
                            self.push_indent(self.indent);
                            self.push_to_line(line);
                            self.flush();
                        }
                    }
                }
                Event::Html(t) => {
                    if t.starts_with("<![CDATA[") {
                        // CDATA is a special marker used for handling options.
                        in_cdata = true;
                        self.flush();
                    } else if in_cdata {
                        if t.trim().ends_with("]]>") {
                            in_cdata = false;
                        } else {
                            let trimmed = t.trim();
                            if trimmed.is_empty() {
                                continue;
                            }
                            if trimmed == "<br>" {
                                self.hard_break();
                            } else if trimmed.starts_with("<dt>") {
                                let opts = unwrap(trimmed, "<dt>", "</dt>");
                                self.push_indent(self.indent);
                                self.push_to_line(opts);
                                self.flush();
                            } else if trimmed.starts_with("<dd>") {
                                let mut def = String::new();
                                while let Some((Event::Html(t), _range)) = self.parser.next() {
                                    if t.starts_with("</dd>") {
                                        break;
                                    }
                                    def.push_str(&t);
                                }
                                let rendered =
                                    TextRenderer::render(&def, self.url.clone(), self.indent + 4)?;
                                self.push_to_line(rendered.trim_end());
                                self.flush();
                            } else {
                                self.push_to_line(&t);
                                self.flush();
                            }
                        }
                    } else {
                        self.push_to_line(&t);
                        self.flush();
                    }
                }
                Event::FootnoteReference(_t) => {}
                Event::SoftBreak => self.flush_word(),
                Event::HardBreak => self.flush(),
                Event::Rule => {
                    self.flush();
                    self.push_indent(self.indent);
                    self.push_to_line(&"_".repeat(79 - self.indent * 2));
                    self.flush();
                }
                Event::TaskListMarker(_b) => unimplemented!(),
            }
        }
        Ok(())
    }

    fn flush(&mut self) {
        self.flush_word();
        if !self.line.is_empty() {
            self.output.push_str(&self.line);
            self.output.push('\n');
            self.line.clear();
        }
    }

    fn hard_break(&mut self) {
        self.flush();
        if !self.output.ends_with("\n\n") {
            self.output.push('\n');
        }
    }

    fn flush_word(&mut self) {
        if self.word.is_empty() {
            return;
        }
        if self.line.len() + self.word.len() >= 79 {
            self.output.push_str(&self.line);
            self.output.push('\n');
            self.line.clear();
        }
        if self.line.is_empty() {
            self.push_indent(self.indent);
            self.line.push_str(&self.word);
        } else {
            self.line.push(' ');
            self.line.push_str(&self.word);
        }
        self.word.clear();
    }

    fn push_indent(&mut self, indent: usize) {
        for _ in 0..indent {
            self.line.push(' ');
        }
    }

    fn push_to_line(&mut self, text: &str) {
        self.flush_word();
        self.line.push_str(text);
    }
}

/// Splits the text on whitespace.
///
/// Consecutive whitespace is collapsed to a single ' ', and is included as a
/// separate element in the result.
fn split_chunks(text: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut start = 0;
    while start < text.len() {
        match text[start..].find(' ') {
            Some(i) => {
                if i != 0 {
                    result.push(&text[start..start + i]);
                }
                result.push(" ");
                // Skip past whitespace.
                match text[start + i..].find(|c| c != ' ') {
                    Some(n) => {
                        start = start + i + n;
                    }
                    None => {
                        break;
                    }
                }
            }
            None => {
                result.push(&text[start..]);
                break;
            }
        }
    }
    result
}

struct Table {
    alignment: Vec<Alignment>,
    rows: Vec<Vec<String>>,
    row: Vec<String>,
    cell: String,
}

impl Table {
    fn new() -> Table {
        Table {
            alignment: Vec::new(),
            rows: Vec::new(),
            row: Vec::new(),
            cell: String::new(),
        }
    }

    /// Processes table events and generates a text table.
    fn process(&mut self, parser: &mut EventIter<'_>, indent: usize) -> Result<String, Error> {
        while let Some((event, _range)) = parser.next() {
            match event {
                Event::Start(tag) => match tag {
                    Tag::TableHead
                    | Tag::TableRow
                    | Tag::TableCell
                    | Tag::Emphasis
                    | Tag::Strong => {}
                    Tag::Strikethrough => self.cell.push_str("~~"),
                    // Links not yet supported, they usually won't fit.
                    Tag::Link(_, _, _) => {}
                    _ => bail!("unexpected tag in table: {:?}", tag),
                },
                Event::End(tag) => match tag {
                    Tag::Table(_) => return self.render(indent),
                    Tag::TableCell => {
                        let cell = mem::replace(&mut self.cell, String::new());
                        self.row.push(cell);
                    }
                    Tag::TableHead | Tag::TableRow => {
                        let row = mem::replace(&mut self.row, Vec::new());
                        self.rows.push(row);
                    }
                    Tag::Strikethrough => self.cell.push_str("~~"),
                    _ => {}
                },
                Event::Text(t) | Event::Code(t) => {
                    self.cell.push_str(&t);
                }
                Event::Html(t) => bail!("html unsupported in tables: {:?}", t),
                _ => bail!("unexpected event in table: {:?}", event),
            }
        }
        bail!("table end not reached");
    }

    fn render(&self, indent: usize) -> Result<String, Error> {
        // This is an extremely primitive layout routine.
        // First compute the potential maximum width of each cell.
        // 2 for 1 space margin on left and right.
        let width_acc = vec![2; self.alignment.len()];
        let mut col_widths = self
            .rows
            .iter()
            .map(|row| row.iter().map(|cell| cell.len()))
            .fold(width_acc, |mut acc, row| {
                acc.iter_mut()
                    .zip(row)
                    // +3 for left/right margin and | symbol
                    .for_each(|(a, b)| *a = (*a).max(b + 3));
                acc
            });
        // Shrink each column until it fits the total width, proportional to
        // the columns total percent width.
        let max_width = 78 - indent;
        // Include total len for | characters, and +1 for final |.
        let total_width = col_widths.iter().sum::<usize>() + col_widths.len() + 1;
        if total_width > max_width {
            let to_shrink = total_width - max_width;
            // Compute percentage widths, and shrink each column based on its
            // total percentage.
            for width in &mut col_widths {
                let percent = *width as f64 / total_width as f64;
                *width -= (to_shrink as f64 * percent).ceil() as usize;
            }
        }
        // Start rendering.
        let mut result = String::new();

        // Draw the horizontal line separating each row.
        let mut row_line = String::new();
        row_line.push_str(&" ".repeat(indent));
        row_line.push('+');
        let lines = col_widths
            .iter()
            .map(|width| "-".repeat(*width))
            .collect::<Vec<_>>();
        row_line.push_str(&lines.join("+"));
        row_line.push('+');
        row_line.push('\n');

        // Draw top of the table.
        result.push_str(&row_line);
        // Draw each row.
        for row in &self.rows {
            // Word-wrap and fill each column as needed.
            let filled = fill_row(row, &col_widths, &self.alignment);
            // Need to transpose the cells across rows for cells that span
            // multiple rows.
            let height = filled.iter().map(|c| c.len()).max().unwrap();
            for row_i in 0..height {
                result.push_str(&" ".repeat(indent));
                result.push('|');
                for filled_row in &filled {
                    let cell = &filled_row[row_i];
                    result.push_str(cell);
                    result.push('|');
                }
                result.push('\n');
            }
            result.push_str(&row_line);
        }
        Ok(result)
    }
}

/// Formats a row, filling cells with spaces and word-wrapping text.
///
/// Returns a vec of cells, where each cell is split into multiple lines.
fn fill_row(row: &[String], col_widths: &[usize], alignment: &[Alignment]) -> Vec<Vec<String>> {
    let mut cell_lines = row
        .iter()
        .zip(col_widths)
        .zip(alignment)
        .map(|((cell, width), alignment)| fill_cell(cell, *width - 2, *alignment))
        .collect::<Vec<_>>();
    // Fill each cell to match the maximum vertical height of the tallest cell.
    let max_lines = cell_lines.iter().map(|cell| cell.len()).max().unwrap();
    for (cell, width) in cell_lines.iter_mut().zip(col_widths) {
        if cell.len() < max_lines {
            cell.extend(std::iter::repeat(" ".repeat(*width)).take(max_lines - cell.len()));
        }
    }
    cell_lines
}

/// Formats a cell. Word-wraps based on width, and adjusts based on alignment.
///
/// Returns a vec of lines for the cell.
fn fill_cell(text: &str, width: usize, alignment: Alignment) -> Vec<String> {
    let fill_width = |text: &str| match alignment {
        Alignment::None | Alignment::Left => format!(" {:<width$} ", text, width = width),
        Alignment::Center => format!(" {:^width$} ", text, width = width),
        Alignment::Right => format!(" {:>width$} ", text, width = width),
    };
    if text.len() < width {
        // No wrapping necessary, just format.
        vec![fill_width(text)]
    } else {
        // Word-wrap the cell.
        let mut result = Vec::new();
        let mut line = String::new();
        for word in text.split_whitespace() {
            if line.len() + word.len() >= width {
                // todo: word.len() > width
                result.push(fill_width(&line));
                line.clear();
            }
            if line.is_empty() {
                line.push_str(word);
            } else {
                line.push(' ');
                line.push_str(&word);
            }
        }
        if !line.is_empty() {
            result.push(fill_width(&line));
        }

        result
    }
}
