use std::path::Path;

use cargo_util_schemas::manifest::TomlToolLints;
use cargo_util_terminal::report::AnnotationKind;
use cargo_util_terminal::report::Group;
use cargo_util_terminal::report::Level;
use cargo_util_terminal::report::Patch;
use cargo_util_terminal::report::Snippet;
use toml_parser::Source;
use toml_parser::Span;
use toml_parser::decoder::Encoding;
use toml_parser::parser::Event;
use toml_parser::parser::EventKind;
use toml_parser::parser::EventReceiver;
use tracing::instrument;

use super::CORRECTNESS;
use crate::CargoResult;
use crate::GlobalContext;
use crate::core::MaybePackage;
use crate::diagnostics::DiagnosticStats;
use crate::diagnostics::Lint;
use crate::diagnostics::LintLevel;
use crate::diagnostics::ManifestFor;
use crate::diagnostics::rel_cwd_manifest_path;

pub static LINT: &Lint = &Lint {
    name: "text_direction_codepoint_in_literal",
    desc: "unicode codepoint changing visible direction of text present in literal",
    primary_group: &CORRECTNESS,
    msrv: Some(super::CARGO_LINTS_MSRV),
    feature_gate: None,
    docs: Some(
        r#"
### What it does
Detects Unicode codepoints in literals in manifests that change the visual representation of text on screen
in a way that does not correspond to their on memory representation.

### Why it is bad
Unicode allows changing the visual flow of text on screen
in order to support scripts that are written right-to-left,
but a specially crafted literal can make code that will be compiled appear to be part of a literal,
depending on the software used to read the code.
To avoid potential problems or confusion,
such as in CVE-2021-42574,
by default we deny their use.
"#,
    ),
};

#[instrument(skip_all)]
pub fn text_direction_codepoint_in_literal(
    manifest: ManifestFor<'_>,
    manifest_path: &Path,
    cargo_lints: &TomlToolLints,
    stats: &mut DiagnosticStats,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let (lint_level, source) = manifest.lint_level(cargo_lints, LINT);
    if lint_level == LintLevel::Allow {
        return Ok(());
    }

    if matches!(
        &manifest,
        ManifestFor::Workspace {
            maybe_pkg: MaybePackage::Package { .. },
            ..
        }
    ) {
        // For real manifests, lint as a package, rather than a workspace
        return Ok(());
    }

    let Some(contents) = manifest.contents() else {
        return Ok(());
    };

    let bidi_spans = contents
        .char_indices()
        .filter(|(_i, c)| {
            UNICODE_BIDI_CODEPOINTS
                .iter()
                .any(|(bidi, _, _name)| c == bidi)
        })
        .map(|(i, c)| (i, i + c.len_utf8()))
        .collect::<Vec<_>>();
    if bidi_spans.is_empty() {
        return Ok(());
    }

    let toml_source = Source::new(contents);
    let events = bidi_events(&toml_source, &bidi_spans);
    let manifest_path = rel_cwd_manifest_path(manifest_path, gctx);
    let mut emitted_source = None;
    for event in events {
        let token_span = event.token.span();
        let token_span = token_span.start()..token_span.end();
        let mut snippet = Snippet::source(contents).path(&manifest_path).annotation(
            AnnotationKind::Context
                .span(token_span.clone())
                .label("this literal contains an invisible unicode text flow control codepoint"),
        );
        for bidi_span in event.bidi_spans {
            let bidi_span = bidi_span.0..bidi_span.1;
            let escaped = format!("{:?}", &contents[bidi_span.clone()]);
            snippet = snippet.annotation(AnnotationKind::Primary.span(bidi_span).label(escaped));
        }
        let mut help_snippet = Snippet::source(contents).path(&manifest_path);
        if let Some(original_raw) = toml_source.get(&event.token) {
            let mut decoded = String::new();
            let replacement = match event.token.kind() {
                toml_parser::parser::EventKind::SimpleKey => {
                    use toml_writer::ToTomlKey as _;
                    original_raw.decode_key(&mut decoded, &mut ());
                    let builder = toml_writer::TomlKeyBuilder::new(&decoded);
                    let replacement = builder.as_basic();
                    Some(replacement.to_toml_key())
                }
                toml_parser::parser::EventKind::Scalar => {
                    use toml_writer::ToTomlValue as _;
                    let kind = original_raw.decode_scalar(&mut decoded, &mut ());
                    if matches!(kind, toml_parser::decoder::ScalarKind::String) {
                        let builder = toml_writer::TomlStringBuilder::new(&decoded);
                        let replacement = match event.token.encoding() {
                            Some(toml_parser::decoder::Encoding::BasicString)
                            | Some(toml_parser::decoder::Encoding::LiteralString)
                            | None => builder.as_basic(),
                            Some(toml_parser::decoder::Encoding::MlBasicString)
                            | Some(toml_parser::decoder::Encoding::MlLiteralString) => {
                                builder.as_ml_basic()
                            }
                        };
                        Some(replacement.to_toml_value())
                    } else {
                        None
                    }
                }
                _ => None,
            };
            if let Some(mut replacement) = replacement {
                for (bidi, escaped, _) in UNICODE_BIDI_CODEPOINTS {
                    replacement = replacement.replace(*bidi, escaped);
                }
                help_snippet = help_snippet.patch(Patch::new(token_span.clone(), replacement));
            }
        }

        let level = lint_level.to_diagnostic_level();
        let mut primary = Group::with_title(level.primary_title(LINT.desc)).element(snippet);
        if emitted_source.is_none() {
            emitted_source = Some(LINT.emitted_source(lint_level, source));
            primary = primary.element(Level::NOTE.message(emitted_source.as_ref().unwrap()));
        }

        let help = Group::with_title(Level::HELP.secondary_title("if you want to keep them but make them visible in your source code, you can escape them")).element(help_snippet);

        let report = [primary, help];

        stats.record_lint(lint_level);
        gctx.shell().print_report(&report, lint_level.force())?;
    }

    Ok(())
}

const UNICODE_BIDI_CODEPOINTS: &[(char, &str, &str)] = &[
    ('\u{202A}', r"\u{202A}", "LEFT-TO-RIGHT EMBEDDING"),
    ('\u{202B}', r"\u{202B}", "RIGHT-TO-LEFT EMBEDDING"),
    ('\u{202C}', r"\u{202C}", "POP DIRECTIONAL FORMATTING"),
    ('\u{202D}', r"\u{202D}", "LEFT-TO-RIGHT OVERRIDE"),
    ('\u{202E}', r"\u{202E}", "RIGHT-TO-LEFT OVERRIDE"),
    ('\u{2066}', r"\u{2066}", "LEFT-TO-RIGHT ISOLATE"),
    ('\u{2067}', r"\u{2067}", "RIGHT-TO-LEFT ISOLATE"),
    ('\u{2068}', r"\u{2068}", "FIRST STRONG ISOLATE"),
    ('\u{2069}', r"\u{2069}", "POP DIRECTIONAL ISOLATE"),
];

struct BiDiEvent {
    token: Event,
    bidi_spans: Vec<(usize, usize)>,
}

fn bidi_events(source: &Source<'_>, bidi_spans: &[(usize, usize)]) -> Vec<BiDiEvent> {
    let mut bidi_spans = bidi_spans.iter();
    let bidi_span = bidi_spans.next().copied();

    let tokens = source.lex().into_vec();
    let mut collector = BiDiCollector {
        bidi_span,
        bidi_spans,
        events: Vec::new(),
    };
    let mut errors = ();
    toml_parser::parser::parse_document(&tokens, &mut collector, &mut errors);

    collector.events
}

struct BiDiCollector<'b> {
    bidi_span: Option<(usize, usize)>,
    bidi_spans: std::slice::Iter<'b, (usize, usize)>,
    events: Vec<BiDiEvent>,
}

impl BiDiCollector<'_> {
    fn process(&mut self, kind: EventKind, encoding: Option<Encoding>, span: Span) {
        let mut event_bidi_spans = Vec::new();
        while let Some(bidi_span) = self.bidi_span {
            if bidi_span.0 < span.start() {
                self.bidi_span = self.bidi_spans.next().copied();
                continue;
            } else if span.end() <= bidi_span.0 {
                break;
            }

            event_bidi_spans.push(bidi_span);
            self.bidi_span = self.bidi_spans.next().copied();
        }

        if !event_bidi_spans.is_empty() {
            let token = Event::new_unchecked(kind, encoding, span);
            self.events.push(BiDiEvent {
                token,
                bidi_spans: event_bidi_spans,
            });
        }
    }
}

impl EventReceiver for BiDiCollector<'_> {
    fn simple_key(
        &mut self,
        span: Span,
        encoding: Option<Encoding>,
        _error: &mut dyn toml_parser::ErrorSink,
    ) {
        self.process(EventKind::SimpleKey, encoding, span)
    }
    fn scalar(
        &mut self,
        span: Span,
        encoding: Option<Encoding>,
        _error: &mut dyn toml_parser::ErrorSink,
    ) {
        self.process(EventKind::Scalar, encoding, span)
    }
}
