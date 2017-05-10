#[macro_use]
extern crate serde_derive;
extern crate serde_json;

pub mod diagnostics;
use diagnostics::{Diagnostic, DiagnosticSpan};

#[derive(Debug, Copy, Clone, Hash, PartialEq)]
pub struct LinePosition {
    pub line: usize,
    pub column: usize,
}

impl std::fmt::Display for LinePosition {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

#[derive(Debug, Copy, Clone, Hash, PartialEq)]
pub struct LineRange {
    pub start: LinePosition,
    pub end: LinePosition,
}

impl std::fmt::Display for LineRange {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}-{}", self.start, self.end)
    }
}

#[derive(Debug, Clone, Hash, PartialEq)]
pub struct Suggestion {
    pub message: String,
    pub file_name: String,
    pub line_range: LineRange,
    /// leading surrounding text, text to replace, trailing surrounding text
    ///
    /// This split is useful for higlighting the part that gets replaced
    pub text: (String, String, String),
    pub replacement: String,
}

fn collect_span(message: &str, span: &DiagnosticSpan) -> Option<Suggestion> {
    if let Some(replacement) = span.suggested_replacement.clone() {
        // unindent the snippet
        let indent = span.text.iter().map(|line| {
            let indent = line.text
                .chars()
                .take_while(|&c| char::is_whitespace(c))
                .count();
            std::cmp::min(indent, line.highlight_start)
        }).min().expect("text to replace is empty");
        let start = span.text[0].highlight_start - 1;
        let end = span.text[0].highlight_end - 1;
        let lead = span.text[0].text[indent..start].to_string();
        let mut body = span.text[0].text[start..end].to_string();
        for line in span.text.iter().take(span.text.len() - 1).skip(1) {
            body.push('\n');
            body.push_str(&line.text[indent..]);
        }
        let mut tail = String::new();
        let last = &span.text[span.text.len() - 1];
        if span.text.len() > 1 {
            body.push('\n');
            body.push_str(&last.text[indent..last.highlight_end - 1]);
        }
        tail.push_str(&last.text[last.highlight_end - 1..]);
        Some(Suggestion {
            message: message.into(),
            file_name: span.file_name.clone(),
            line_range: LineRange {
                start: LinePosition {
                    line: span.line_start,
                    column: span.column_start,
                },
                end: LinePosition {
                    line: span.line_end,
                    column: span.column_end,
                },
            },
            text: (lead, body, tail),
            replacement: replacement,
        })
    } else {
        None
    }
}

pub fn collect_suggestions(diagnostic: &Diagnostic,
                           parent_message: Option<String>)
                           -> Vec<Suggestion> {
    let message = parent_message.unwrap_or(diagnostic.message.clone());
    let mut suggestions = vec![];

    suggestions.extend(diagnostic.spans
        .iter()
        .flat_map(|span| collect_span(&message, span)));

    suggestions.extend(diagnostic.children
        .iter()
        .flat_map(|children| collect_suggestions(children, Some(message.clone()))));

    suggestions
}
