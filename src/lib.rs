#![warn(rust_2018_idioms)]

#[macro_use]
extern crate log;
#[cfg(test)]
#[macro_use]
extern crate proptest;

use std::collections::HashSet;
use std::ops::Range;

use anyhow::Error;

pub mod diagnostics;
use crate::diagnostics::{Diagnostic, DiagnosticSpan};
mod replace;

#[derive(Debug, Clone, Copy)]
pub enum Filter {
    MachineApplicableOnly,
    Everything,
}

pub fn get_suggestions_from_json<S: ::std::hash::BuildHasher>(
    input: &str,
    only: &HashSet<String, S>,
    filter: Filter,
) -> serde_json::error::Result<Vec<Suggestion>> {
    let mut result = Vec::new();
    for cargo_msg in serde_json::Deserializer::from_str(input).into_iter::<Diagnostic>() {
        // One diagnostic line might have multiple suggestions
        result.extend(collect_suggestions(&cargo_msg?, only, filter));
    }
    Ok(result)
}

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub struct LinePosition {
    pub line: usize,
    pub column: usize,
}

impl std::fmt::Display for LinePosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub struct LineRange {
    pub start: LinePosition,
    pub end: LinePosition,
}

impl std::fmt::Display for LineRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.start, self.end)
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
/// An error/warning and possible solutions for fixing it
pub struct Suggestion {
    pub message: String,
    pub snippets: Vec<Snippet>,
    pub solutions: Vec<Solution>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Solution {
    pub message: String,
    pub replacements: Vec<Replacement>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Snippet {
    pub file_name: String,
    pub line_range: LineRange,
    pub range: Range<usize>,
    /// leading surrounding text, text to replace, trailing surrounding text
    ///
    /// This split is useful for higlighting the part that gets replaced
    pub text: (String, String, String),
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Replacement {
    pub snippet: Snippet,
    pub replacement: String,
}

fn parse_snippet(span: &DiagnosticSpan) -> Option<Snippet> {
    // unindent the snippet
    let indent = span
        .text
        .iter()
        .map(|line| {
            let indent = line
                .text
                .chars()
                .take_while(|&c| char::is_whitespace(c))
                .count();
            std::cmp::min(indent, line.highlight_start - 1)
        })
        .min()?;

    let text_slice = span.text[0].text.chars().collect::<Vec<char>>();

    // We subtract `1` because these highlights are 1-based
    // Check the `min` so that it doesn't attempt to index out-of-bounds when
    // the span points to the "end" of the line. For example, a line of
    // "foo\n" with a highlight_start of 5 is intended to highlight *after*
    // the line. This needs to compensate since the newline has been removed
    // from the text slice.
    let start = (span.text[0].highlight_start - 1).min(text_slice.len());
    let end = (span.text[0].highlight_end - 1).min(text_slice.len());
    let lead = text_slice[indent..start].iter().collect();
    let mut body: String = text_slice[start..end].iter().collect();

    for line in span.text.iter().take(span.text.len() - 1).skip(1) {
        body.push('\n');
        body.push_str(&line.text[indent..]);
    }
    let mut tail = String::new();
    let last = &span.text[span.text.len() - 1];

    // If we get a DiagnosticSpanLine where highlight_end > text.len(), we prevent an 'out of
    // bounds' access by making sure the index is within the array bounds.
    // `saturating_sub` is used in case of an empty file
    let last_tail_index = last.highlight_end.min(last.text.len()).saturating_sub(1);
    let last_slice = last.text.chars().collect::<Vec<char>>();

    if span.text.len() > 1 {
        body.push('\n');
        body.push_str(
            &last_slice[indent..last_tail_index]
                .iter()
                .collect::<String>(),
        );
    }
    tail.push_str(&last_slice[last_tail_index..].iter().collect::<String>());
    Some(Snippet {
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
        range: (span.byte_start as usize)..(span.byte_end as usize),
        text: (lead, body, tail),
    })
}

fn collect_span(span: &DiagnosticSpan) -> Option<Replacement> {
    let snippet = parse_snippet(span)?;
    let replacement = span.suggested_replacement.clone()?;
    Some(Replacement {
        snippet,
        replacement,
    })
}

pub fn collect_suggestions<S: ::std::hash::BuildHasher>(
    diagnostic: &Diagnostic,
    only: &HashSet<String, S>,
    filter: Filter,
) -> Option<Suggestion> {
    if !only.is_empty() {
        if let Some(ref code) = diagnostic.code {
            if !only.contains(&code.code) {
                // This is not the code we are looking for
                return None;
            }
        } else {
            // No code, probably a weird builtin warning/error
            return None;
        }
    }

    let snippets = diagnostic.spans.iter().filter_map(parse_snippet).collect();

    let solutions: Vec<_> = diagnostic
        .children
        .iter()
        .filter_map(|child| {
            let replacements: Vec<_> = child
                .spans
                .iter()
                .filter(|span| {
                    use crate::diagnostics::Applicability::*;
                    use crate::Filter::*;

                    match (filter, &span.suggestion_applicability) {
                        (MachineApplicableOnly, Some(MachineApplicable)) => true,
                        (MachineApplicableOnly, _) => false,
                        (Everything, _) => true,
                    }
                })
                .filter_map(collect_span)
                .collect();
            if !replacements.is_empty() {
                Some(Solution {
                    message: child.message.clone(),
                    replacements,
                })
            } else {
                None
            }
        })
        .collect();

    if solutions.is_empty() {
        None
    } else {
        Some(Suggestion {
            message: diagnostic.message.clone(),
            snippets,
            solutions,
        })
    }
}

pub struct CodeFix {
    data: replace::Data,
}

impl CodeFix {
    pub fn new(s: &str) -> CodeFix {
        CodeFix {
            data: replace::Data::new(s.as_bytes()),
        }
    }

    pub fn apply(&mut self, suggestion: &Suggestion) -> Result<(), Error> {
        for sol in &suggestion.solutions {
            for r in &sol.replacements {
                self.data.replace_range(
                    r.snippet.range.start,
                    r.snippet.range.end.saturating_sub(1),
                    r.replacement.as_bytes(),
                )?;
            }
        }
        Ok(())
    }

    pub fn finish(&self) -> Result<String, Error> {
        Ok(String::from_utf8(self.data.to_vec())?)
    }
}

pub fn apply_suggestions(code: &str, suggestions: &[Suggestion]) -> Result<String, Error> {
    let mut fix = CodeFix::new(code);
    for suggestion in suggestions.iter().rev() {
        fix.apply(suggestion)?;
    }
    fix.finish()
}
