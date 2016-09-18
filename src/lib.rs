#![feature(custom_derive, plugin)]
#![plugin(serde_macros)]

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
    pub text: String,
    pub replacement: String,
}

fn collect_span(message: &str, span: &DiagnosticSpan) -> Option<Suggestion> {
    if let Some(replacement) = span.suggested_replacement.clone() {
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
            text: span.text.iter().map(|x| x.text.clone()).collect::<Vec<_>>().join("\n"),
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
