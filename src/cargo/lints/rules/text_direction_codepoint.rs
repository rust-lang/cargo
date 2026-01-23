use std::path::Path;

use annotate_snippets::AnnotationKind;
use annotate_snippets::Group;
use annotate_snippets::Level;
use annotate_snippets::Patch;
use annotate_snippets::Snippet;
use cargo_util_schemas::manifest::TomlToolLints;
use toml_parser::Source;
use toml_parser::Span;
use toml_parser::decoder::Encoding;
use toml_parser::parser::EventReceiver;
use toml_writer::{ToTomlKey, ToTomlValue};

use crate::CargoResult;
use crate::GlobalContext;
use crate::core::MaybePackage;
use crate::lints::CORRECTNESS;
use crate::lints::Lint;
use crate::lints::LintLevel;
use crate::lints::LintLevelReason;
use crate::lints::ManifestFor;
use crate::lints::rel_cwd_manifest_path;

const UNICODE_BIDI_CODEPOINTS: &[(char, &str)] = &[
    ('\u{202A}', "LEFT-TO-RIGHT EMBEDDING"),
    ('\u{202B}', "RIGHT-TO-LEFT EMBEDDING"),
    ('\u{202C}', "POP DIRECTIONAL FORMATTING"),
    ('\u{202D}', "LEFT-TO-RIGHT OVERRIDE"),
    ('\u{202E}', "RIGHT-TO-LEFT OVERRIDE"),
    ('\u{2066}', "LEFT-TO-RIGHT ISOLATE"),
    ('\u{2067}', "RIGHT-TO-LEFT ISOLATE"),
    ('\u{2068}', "FIRST STRONG ISOLATE"),
    ('\u{2069}', "POP DIRECTIONAL ISOLATE"),
];

pub const LINT: Lint = Lint {
    name: "text_direction_codepoint",
    desc: "unicode codepoint changing visible direction of text present in manifest",
    primary_group: &CORRECTNESS,
    edition_lint_opts: None,
    feature_gate: None,
    docs: None,
};

/// Paths where BiDi codepoints are allowed (legitimate RTL content).
const ALLOWED_BIDI_PATHS: &[&[&str]] = &[
    &["package", "description"],
    &["package", "metadata"],
    &["workspace", "metadata"],
];

fn is_allowed_bidi_path(path: &[String]) -> bool {
    ALLOWED_BIDI_PATHS.iter().any(|allowed| {
        if path.len() < allowed.len() {
            return false;
        }
        path.iter()
            .zip(allowed.iter())
            .all(|(a, b)| a.as_str() == *b)
    })
}

/// Generate a suggestion for replacing a literal string or bare key with a basic string
/// that has escaped BiDi codepoints.
fn generate_suggestion(
    contents: &str,
    span: Span,
    finding_type: &FindingType,
    encoding: Option<&Encoding>,
) -> Option<String> {
    let needs_suggestion = match (finding_type, encoding) {
        (FindingType::Key, Some(Encoding::LiteralString)) => true,
        (FindingType::Key, None) => true, // bare key
        (FindingType::Scalar, Some(Encoding::LiteralString)) => true,
        _ => false,
    };

    if !needs_suggestion {
        return None;
    }

    let text = &contents[span.start()..span.end()];

    let decoded = if text.starts_with('"') && text.ends_with('"') {
        return None;
    } else if text.starts_with('\'') && text.ends_with('\'') {
        text[1..text.len() - 1].to_string()
    } else {
        text.to_string()
    };

    match finding_type {
        FindingType::Key => {
            let builder = toml_writer::TomlKeyBuilder::new(&decoded);
            if let Some(key) = builder.as_unquoted() {
                Some(key.to_toml_key())
            } else {
                Some(builder.as_basic().to_toml_key())
            }
        }
        FindingType::Scalar => {
            let builder = toml_writer::TomlStringBuilder::new(&decoded);
            Some(builder.as_basic().to_toml_value())
        }
        _ => None,
    }
}

#[derive(Clone)]
enum FindingType {
    Key,
    Scalar,
    Comment,
}

struct Finding {
    byte_idx: usize,
    ch: char,
    name: &'static str,
    in_allowed_value: bool,
    event_span: (usize, usize),
    finding_type: FindingType,
    encoding: Option<Encoding>,
    span: Span,
}

struct BiDiCollector<'a> {
    contents: &'a str,
    findings: &'a mut Vec<Finding>,
    key_path: Vec<String>,
    table_stack: Vec<Vec<String>>,
}

impl<'a> BiDiCollector<'a> {
    fn new(contents: &'a str, findings: &'a mut Vec<Finding>) -> Self {
        Self {
            contents,
            findings,
            key_path: Vec::new(),
            table_stack: Vec::new(),
        }
    }

    fn check_span_for_bidi(
        &mut self,
        span: Span,
        in_value: bool,
        finding_type: FindingType,
        encoding: Option<Encoding>,
    ) {
        let text = &self.contents[span.start()..span.end()];
        let event_span = (span.start(), span.end());
        for (offset, ch) in text.char_indices() {
            if let Some((_, name)) = UNICODE_BIDI_CODEPOINTS.iter().find(|(c, _)| *c == ch) {
                let in_allowed_value = in_value && is_allowed_bidi_path(&self.key_path);
                self.findings.push(Finding {
                    byte_idx: span.start() + offset,
                    ch,
                    name,
                    in_allowed_value,
                    event_span,
                    finding_type: finding_type.clone(),
                    encoding: encoding.clone(),
                    span,
                });
            }
        }
    }

    fn current_path(&self) -> Vec<String> {
        let mut path = self.table_stack.last().cloned().unwrap_or_default();
        path.extend(self.key_path.clone());
        path
    }
}

impl EventReceiver for BiDiCollector<'_> {
    fn std_table_open(&mut self, _span: Span, _error: &mut dyn toml_parser::ErrorSink) {
        self.table_stack.push(self.key_path.clone());
        self.key_path.clear();
    }

    fn std_table_close(&mut self, _span: Span, _error: &mut dyn toml_parser::ErrorSink) {
        if let Some(last) = self.table_stack.last_mut() {
            *last = self.key_path.clone();
        }
        self.key_path.clear();
    }

    fn array_table_open(&mut self, _span: Span, _error: &mut dyn toml_parser::ErrorSink) {
        self.table_stack.push(self.key_path.clone());
        self.key_path.clear();
    }

    fn array_table_close(&mut self, _span: Span, _error: &mut dyn toml_parser::ErrorSink) {
        if let Some(last) = self.table_stack.last_mut() {
            *last = self.key_path.clone();
        }
        self.key_path.clear();
    }

    fn simple_key(
        &mut self,
        span: Span,
        kind: Option<toml_parser::decoder::Encoding>,
        _error: &mut dyn toml_parser::ErrorSink,
    ) {
        self.check_span_for_bidi(span, false, FindingType::Key, kind);

        let key_text = &self.contents[span.start()..span.end()];
        let key = if (key_text.starts_with('"') && key_text.ends_with('"'))
            || (key_text.starts_with('\'') && key_text.ends_with('\''))
        {
            key_text[1..key_text.len() - 1].to_string()
        } else {
            key_text.to_string()
        };
        self.key_path.push(key);
    }

    fn key_sep(&mut self, _span: Span, _error: &mut dyn toml_parser::ErrorSink) {}

    fn key_val_sep(&mut self, _span: Span, _error: &mut dyn toml_parser::ErrorSink) {}

    fn scalar(
        &mut self,
        span: Span,
        kind: Option<toml_parser::decoder::Encoding>,
        _error: &mut dyn toml_parser::ErrorSink,
    ) {
        let full_path = self.current_path();
        let saved_key_path = std::mem::replace(&mut self.key_path, full_path);
        self.check_span_for_bidi(span, true, FindingType::Scalar, kind);
        self.key_path = saved_key_path;
        self.key_path.clear();
    }

    fn comment(&mut self, span: Span, _error: &mut dyn toml_parser::ErrorSink) {
        self.check_span_for_bidi(span, false, FindingType::Comment, None);
    }
}

pub fn text_direction_codepoint(
    manifest: ManifestFor<'_>,
    manifest_path: &Path,
    cargo_lints: &TomlToolLints,
    error_count: &mut usize,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let (lint_level, reason) = if let Some(spec) = cargo_lints.get(LINT.name) {
        (spec.level().into(), LintLevelReason::Package)
    } else {
        manifest.lint_level(cargo_lints, LINT)
    };

    if lint_level == LintLevel::Allow {
        return Ok(());
    }

    if matches!(&manifest, ManifestFor::Workspace(MaybePackage::Package(_))) {
        return Ok(());
    }

    let Some(contents) = manifest.contents() else {
        return Ok(());
    };

    let has_bidi = contents.chars().any(|ch| {
        UNICODE_BIDI_CODEPOINTS
            .iter()
            .any(|(bidi_ch, _)| *bidi_ch == ch)
    });

    if !has_bidi {
        return Ok(());
    }

    let mut findings = Vec::new();
    {
        let source = Source::new(contents);
        let tokens = source.lex().into_vec();
        let mut collector = BiDiCollector::new(contents, &mut findings);
        let mut errors = Vec::new();
        toml_parser::parser::parse_document(&tokens, &mut collector, &mut errors);
    }

    let disallowed_findings: Vec<_> = findings
        .into_iter()
        .filter(|f| !f.in_allowed_value)
        .collect();

    if disallowed_findings.is_empty() {
        return Ok(());
    }

    let manifest_path_str = rel_cwd_manifest_path(manifest_path, gctx);

    let mut findings_by_event: std::collections::BTreeMap<(usize, usize), Vec<_>> =
        std::collections::BTreeMap::new();
    for finding in disallowed_findings {
        findings_by_event
            .entry(finding.event_span)
            .or_insert_with(Vec::new)
            .push(finding);
    }

    let level = lint_level.to_diagnostic_level();
    let emitted_source = LINT.emitted_source(lint_level, reason);

    for (event_idx, event_findings) in findings_by_event.values().enumerate() {
        if lint_level.is_error() {
            *error_count += 1;
        }

        let title = LINT.desc.to_string();

        let labels: Vec<String> = event_findings
            .iter()
            .map(|f| format!(r#"`\u{{{:04X}}}` ({})"#, f.ch as u32, f.name))
            .collect();

        let first_finding = &event_findings[0];
        let suggestion = generate_suggestion(
            contents,
            first_finding.span,
            &first_finding.finding_type,
            first_finding.encoding.as_ref(),
        );

        let mut snippet = Snippet::source(contents).path(&manifest_path_str);
        for (finding, label) in event_findings.iter().zip(labels.iter()) {
            let span = finding.byte_idx..(finding.byte_idx + finding.ch.len_utf8());
            snippet = snippet.annotation(AnnotationKind::Primary.span(span).label(label));
        }

        let mut primary_group =
            Group::with_title(level.clone().primary_title(&title)).element(snippet);

        if event_idx == 0 {
            primary_group = primary_group.element(Level::NOTE.message(&emitted_source));
            primary_group = primary_group.element(Level::NOTE.message(
                "these kinds of unicode codepoints change the way text flows on screen, \
                 but can cause confusion because they change the order of characters",
            ));
        }

        let mut report = vec![primary_group];

        if let Some(sugg) = &suggestion {
            let event_span = first_finding.event_span;
            let sugg_str = sugg.as_str();
            report.push(
                Level::HELP.secondary_title("suggested fix").element(
                    Snippet::source(contents)
                        .path(&manifest_path_str)
                        .patch(Patch::new(event_span.0..event_span.1, sugg_str)),
                ),
            );
        } else {
            report[0] = report[0].clone().element(Level::HELP.message(
                "if their presence wasn't intentional, you can remove them, \
                 or use their escape sequence (e.g., \\u{202E}) in double-quoted strings",
            ));
        }

        gctx.shell().print_report(&report, lint_level.force())?;
    }

    Ok(())
}
