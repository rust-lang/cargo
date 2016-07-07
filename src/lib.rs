#![feature(custom_derive, plugin)]
#![plugin(serde_macros)]

extern crate serde_json;

pub mod diagnostics;
use diagnostics::{Diagnostic, DiagnosticSpan};

#[derive(Debug)]
pub struct LinePosition(pub usize, pub usize);

impl std::fmt::Display for LinePosition {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}:{}", self.0, self.1)
    }
}

#[derive(Debug)]
pub struct LineRange(pub LinePosition, pub LinePosition);

impl std::fmt::Display for LineRange {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}-{}", self.0, self.1)
    }
}

#[derive(Debug)]
pub struct Suggestion {
    pub message: String,
    pub file_name: String,
    pub line_range: LineRange,
    pub byte_range: (usize, usize),
    pub text: String,
    pub replacement: String,
}

// fn normalize_indent<'a, T: Iterator<Item = &'a DiagnosticSpanLine>>(lines: &T)
//     -> Option<String>
// {
//     if let Some(first_line) = lines.clone().next() {
//         let leading_whitespace =
//             first_line.text.chars()
//             .take_while(|&c| char::is_whitespace(c))
//             .count();
        
//         Some(lines.clone()
//             .map(|line| String::from(&line.text[leading_whitespace..]))
//             .collect::<Vec<_>>()
//             .join("\n"))
//     } else {
//         None
//     }
// }

fn collect_span(message: &str, span: &DiagnosticSpan) -> Option<Suggestion> {
    if let Some(replacement) = span.suggested_replacement.clone() {
        Some(Suggestion {
            message: message.into(),
            file_name: span.file_name.clone(),
            line_range: LineRange(LinePosition(span.line_start, span.column_start),
                LinePosition(span.line_end, span.column_end)),
            byte_range: (span.byte_start, span.byte_end),
            text: span.text.iter().map(|ref x| x.text.clone()).collect::<Vec<_>>().join("\n"),
            replacement: replacement,
        })
    } else {
        None
    }
}

pub fn collect_suggestions(diagnostic: &Diagnostic, parent_message: Option<String>)
    -> Vec<Suggestion>
{
    let message = parent_message.unwrap_or(diagnostic.message.clone());
    let mut suggestions = vec![];

    suggestions.extend(diagnostic.spans.iter()
        .flat_map(|span| collect_span(&message, span)));
    
    suggestions.extend(diagnostic.children.iter()
        .flat_map(|children| collect_suggestions(children, Some(message.clone()))));
    
    suggestions
}

#[cfg(test)]
mod tests;
