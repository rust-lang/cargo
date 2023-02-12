//! Man-page formatter.

use crate::util::{header_text, parse_name_and_section};
use crate::EventIter;
use anyhow::{bail, Error};
use pulldown_cmark::{Alignment, Event, HeadingLevel, LinkType, Tag};
use std::fmt::Write;
use url::Url;

pub struct ManFormatter {
    url: Option<Url>,
}

impl ManFormatter {
    pub fn new(url: Option<Url>) -> ManFormatter {
        ManFormatter { url }
    }
}

impl super::Formatter for ManFormatter {
    fn render(&self, input: &str) -> Result<String, Error> {
        ManRenderer::render(input, self.url.clone())
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
            .map(|param| {
                let r = self.render(param)?;
                Ok(r.trim().trim_start_matches(".sp").to_string())
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let rendered_block = self.render(block)?;
        let rendered_block = rendered_block.trim().trim_start_matches(".sp").trim();
        // .RS = move left margin to right 4.
        // .RE = move margin back one level.
        Ok(format!(
            "\n.sp\n{}\n.RS 4\n{}\n.RE\n",
            rendered_options.join(", "),
            rendered_block
        ))
    }

    fn linkify_man_to_md(&self, name: &str, section: u8) -> Result<String, Error> {
        Ok(format!("`{}`({})", name, section))
    }
}

#[derive(Copy, Clone)]
enum Font {
    Bold,
    Italic,
}

impl Font {
    fn str_from_stack(font_stack: &[Font]) -> &'static str {
        let has_bold = font_stack.iter().any(|font| matches!(font, Font::Bold));
        let has_italic = font_stack.iter().any(|font| matches!(font, Font::Italic));
        match (has_bold, has_italic) {
            (false, false) => "\\fR", // roman (normal)
            (false, true) => "\\fI",  // italic
            (true, false) => "\\fB",  // bold
            (true, true) => "\\f(BI", // bold italic
        }
    }
}

struct ManRenderer<'e> {
    output: String,
    parser: EventIter<'e>,
    font_stack: Vec<Font>,
}

impl<'e> ManRenderer<'e> {
    fn render(input: &str, url: Option<Url>) -> Result<String, Error> {
        let parser = crate::md_parser(input, url);
        let output = String::with_capacity(input.len() * 3 / 2);
        let mut mr = ManRenderer {
            parser,
            output,
            font_stack: Vec::new(),
        };
        mr.push_man()?;
        Ok(mr.output)
    }

    fn push_man(&mut self) -> Result<(), Error> {
        // If this is true, this is inside a cdata block used for hiding
        // content from pulldown_cmark.
        let mut in_cdata = false;
        // The current list stack. None if unordered, Some if ordered with the
        // given number as the current index.
        let mut list: Vec<Option<u64>> = Vec::new();
        // Used in some cases where spacing isn't desired.
        let mut suppress_paragraph = false;
        let mut table_cell_index = 0;

        while let Some((event, range)) = self.parser.next() {
            let this_suppress_paragraph = suppress_paragraph;
            suppress_paragraph = false;
            match event {
                Event::Start(tag) => {
                    match tag {
                        Tag::Paragraph => {
                            if !this_suppress_paragraph {
                                self.flush();
                                self.output.push_str(".sp\n");
                            }
                        }
                        Tag::Heading(level, ..) => {
                            if level == HeadingLevel::H1 {
                                self.push_top_header()?;
                            } else if level == HeadingLevel::H2 {
                                // Section header
                                let text = header_text(&mut self.parser)?;
                                self.flush();
                                write!(self.output, ".SH \"{}\"\n", text)?;
                                suppress_paragraph = true;
                            } else {
                                // Subsection header
                                let text = header_text(&mut self.parser)?;
                                self.flush();
                                write!(self.output, ".SS \"{}\"\n", text)?;
                                suppress_paragraph = true;
                            }
                        }
                        Tag::BlockQuote => {
                            self.flush();
                            // .RS = move left margin over 3
                            // .ll = shrink line length
                            self.output.push_str(".RS 3\n.ll -5\n.sp\n");
                            suppress_paragraph = true;
                        }
                        Tag::CodeBlock(_kind) => {
                            // space down, indent 4, no-fill mode
                            self.flush();
                            self.output.push_str(".sp\n.RS 4\n.nf\n");
                        }
                        Tag::List(start) => list.push(start),
                        Tag::Item => {
                            // Note: This uses explicit movement instead of .IP
                            // because the spacing on .IP looks weird to me.
                            // space down, indent 4
                            self.flush();
                            self.output.push_str(".sp\n.RS 4\n");
                            match list.last_mut().expect("item must have list start") {
                                // Ordered list.
                                Some(n) => {
                                    // move left 4, output the list index number, move right 1.
                                    write!(self.output, "\\h'-04' {}.\\h'+01'", n)?;
                                    *n += 1;
                                }
                                // Unordered list.
                                None => self.output.push_str("\\h'-04'\\(bu\\h'+02'"),
                            }
                            suppress_paragraph = true;
                        }
                        Tag::FootnoteDefinition(_label) => unimplemented!(),
                        Tag::Table(alignment) => {
                            // Table start
                            // allbox = draw a box around all the cells
                            // tab(:) = Use `:` to separate cell data (instead of tab)
                            // ; = end of options
                            self.output.push_str(
                                "\n.TS\n\
                                allbox tab(:);\n",
                            );
                            let alignments: Vec<_> = alignment
                                .iter()
                                .map(|a| match a {
                                    Alignment::Left | Alignment::None => "lt",
                                    Alignment::Center => "ct",
                                    Alignment::Right => "rt",
                                })
                                .collect();
                            self.output.push_str(&alignments.join(" "));
                            self.output.push_str(".\n");
                            table_cell_index = 0;
                        }
                        Tag::TableHead => {
                            table_cell_index = 0;
                        }
                        Tag::TableRow => {
                            table_cell_index = 0;
                            self.output.push('\n');
                        }
                        Tag::TableCell => {
                            if table_cell_index != 0 {
                                // Separator between columns.
                                self.output.push(':');
                            }
                            // Start a text block.
                            self.output.push_str("T{\n");
                            table_cell_index += 1
                        }
                        Tag::Emphasis => self.push_font(Font::Italic),
                        Tag::Strong => self.push_font(Font::Bold),
                        // Strikethrough isn't usually supported for TTY.
                        Tag::Strikethrough => self.output.push_str("~~"),
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
                                | LinkType::Shortcut => {
                                    self.push_font(Font::Italic);
                                }
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
                Event::End(tag) => {
                    match &tag {
                        Tag::Paragraph => self.flush(),
                        Tag::Heading(..) => {}
                        Tag::BlockQuote => {
                            self.flush();
                            // restore left margin, restore line length
                            self.output.push_str(".br\n.RE\n.ll\n");
                        }
                        Tag::CodeBlock(_kind) => {
                            self.flush();
                            // Restore fill mode, move margin back one level.
                            self.output.push_str(".fi\n.RE\n");
                        }
                        Tag::List(_) => {
                            list.pop();
                        }
                        Tag::Item => {
                            self.flush();
                            // Move margin back one level.
                            self.output.push_str(".RE\n");
                        }
                        Tag::FootnoteDefinition(_label) => {}
                        Tag::Table(_) => {
                            // Table end
                            // I don't know why, but the .sp is needed to provide
                            // space with the following content.
                            self.output.push_str("\n.TE\n.sp\n");
                        }
                        Tag::TableHead => {}
                        Tag::TableRow => {}
                        Tag::TableCell => {
                            // End text block.
                            self.output.push_str("\nT}");
                        }
                        Tag::Emphasis | Tag::Strong => self.pop_font(),
                        Tag::Strikethrough => self.output.push_str("~~"),
                        Tag::Link(link_type, dest_url, _title) => {
                            if dest_url.starts_with('#') {
                                continue;
                            }
                            match link_type {
                                LinkType::Autolink | LinkType::Email => {}
                                LinkType::Inline
                                | LinkType::Reference
                                | LinkType::Collapsed
                                | LinkType::Shortcut => {
                                    self.pop_font();
                                    self.output.push(' ');
                                }
                                _ => {
                                    panic!("unexpected tag {:?}", tag);
                                }
                            }
                            write!(self.output, "<{}>", escape(&dest_url)?)?;
                        }
                        Tag::Image(_link_type, _dest_url, _title) => {}
                    }
                }
                Event::Text(t) => {
                    self.output.push_str(&escape(&t)?);
                }
                Event::Code(t) => {
                    self.push_font(Font::Bold);
                    self.output.push_str(&escape(&t)?);
                    self.pop_font();
                }
                Event::Html(t) => {
                    if t.starts_with("<![CDATA[") {
                        // CDATA is a special marker used for handling options.
                        in_cdata = true;
                    } else if in_cdata {
                        if t.trim().ends_with("]]>") {
                            in_cdata = false;
                        } else if !t.trim().is_empty() {
                            self.output.push_str(&t);
                        }
                    } else {
                        self.output.push_str(&escape(&t)?);
                    }
                }
                Event::FootnoteReference(_t) => {}
                Event::SoftBreak => self.output.push('\n'),
                Event::HardBreak => {
                    self.flush();
                    self.output.push_str(".br\n");
                }
                Event::Rule => {
                    self.flush();
                    // \l' **length** '   Draw horizontal line (default underscore).
                    // \n(.lu  Gets value from register "lu" (current line length)
                    self.output.push_str("\\l'\\n(.lu'\n");
                }
                Event::TaskListMarker(_b) => unimplemented!(),
            }
        }
        Ok(())
    }

    fn flush(&mut self) {
        if !self.output.ends_with('\n') {
            self.output.push('\n');
        }
    }

    /// Switch to the given font.
    ///
    /// Because the troff sequence `\fP` for switching to the "previous" font
    /// doesn't support nesting, this needs to emulate it here. This is needed
    /// for situations like **hi _there_**.
    fn push_font(&mut self, font: Font) {
        self.font_stack.push(font);
        self.output.push_str(Font::str_from_stack(&self.font_stack));
    }

    fn pop_font(&mut self) {
        self.font_stack.pop();
        self.output.push_str(Font::str_from_stack(&self.font_stack));
    }

    /// Parse and render the first top-level header of the document.
    fn push_top_header(&mut self) -> Result<(), Error> {
        // This enables the tbl preprocessor for tables.
        // This seems to be enabled by default on every modern system I could
        // find, but it doesn't seem to hurt to enable this.
        self.output.push_str("'\\\" t\n");
        // Extract the name of the man page.
        let text = header_text(&mut self.parser)?;
        let (name, section) = parse_name_and_section(&text)?;
        // .TH = Table header
        // .nh = disable hyphenation
        // .ad l = Left-adjust mode (disable justified).
        // .ss sets sentence_space_size to 0 (prevents double spaces after .
        //     if . is last on the line)
        write!(
            self.output,
            ".TH \"{}\" \"{}\"\n\
            .nh\n\
            .ad l\n\
            .ss \\n[.ss] 0\n",
            escape(&name.to_uppercase())?,
            section
        )?;
        Ok(())
    }
}

fn escape(s: &str) -> Result<String, Error> {
    // Note: Possible source on output escape sequences: https://man7.org/linux/man-pages/man7/groff_char.7.html.
    //       Otherwise, use generic escaping in the form `\[u1EE7]` or `\[u1F994]`.

    let mut replaced = s
        .replace('\\', "\\(rs")
        .replace('-', "\\-")
        .replace('\u{00A0}', "\\ ") // non-breaking space (non-stretchable)
        .replace('–', "\\[en]") // \u{2013} en-dash
        .replace('—', "\\[em]") // \u{2014} em-dash
        .replace('‘', "\\[oq]") // \u{2018} left single quote
        .replace('’', "\\[cq]") // \u{2019} right single quote or apostrophe
        .replace('“', "\\[lq]") // \u{201C} left double quote
        .replace('”', "\\[rq]") // \u{201D} right double quote
        .replace('…', "\\[u2026]") // \u{2026} ellipsis
        .replace('│', "|") // \u{2502} box drawing light vertical (could use \[br])
        .replace('├', "|") // \u{251C} box drawings light vertical and right
        .replace('└', "`") // \u{2514} box drawings light up and right
        .replace('─', "\\-") // \u{2500} box drawing light horizontal
    ;
    if replaced.starts_with('.') {
        replaced = format!("\\&.{}", &replaced[1..]);
    }

    if let Some(ch) = replaced.chars().find(|ch| {
        !matches!(ch, '\n' | ' ' | '!'..='/' | '0'..='9'
            | ':'..='@' | 'A'..='Z' | '['..='`' | 'a'..='z' | '{'..='~')
    }) {
        bail!(
            "character {:?} is not allowed (update the translation table if needed)",
            ch
        );
    }
    Ok(replaced)
}
