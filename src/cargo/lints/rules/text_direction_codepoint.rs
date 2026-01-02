use std::path::Path;

use annotate_snippets::AnnotationKind;
use annotate_snippets::Group;
use annotate_snippets::Level;
use annotate_snippets::Snippet;
use cargo_util_schemas::manifest::TomlToolLints;

use crate::CargoResult;
use crate::GlobalContext;
use crate::core::MaybePackage;
use crate::lints::Lint;
use crate::lints::LintLevel;
use crate::lints::ManifestFor;
use crate::lints::rel_cwd_manifest_path;

/// Unicode BiDi (bidirectional) control codepoints that can be used in
/// "Trojan Source" attacks (CVE-2021-42574).
///
/// These codepoints change the visual representation of text on screen
/// in a way that does not correspond to their in-memory representation.
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
    groups: &[],
    default_level: LintLevel::Deny,
    edition_lint_opts: None,
    feature_gate: None,
    docs: None,
};

pub fn text_direction_codepoint(
    manifest: ManifestFor<'_>,
    manifest_path: &Path,
    cargo_lints: &TomlToolLints,
    error_count: &mut usize,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let (lint_level, reason) = manifest.lint_level(cargo_lints, LINT);

    if lint_level == LintLevel::Allow {
        return Ok(());
    }

    // Skip non-virtual workspace manifests - they are already checked as Package via emit_pkg_lints
    if matches!(&manifest, ManifestFor::Workspace(MaybePackage::Package(_))) {
        return Ok(());
    }

    let contents = manifest.contents();
    let manifest_path_str = rel_cwd_manifest_path(manifest_path, gctx);

    let findings: Vec<_> = contents
        .char_indices()
        .filter_map(|(idx, ch)| {
            UNICODE_BIDI_CODEPOINTS
                .iter()
                .find(|(c, _)| *c == ch)
                .map(|(_, name)| (idx, ch, *name))
        })
        .collect();

    if findings.is_empty() {
        return Ok(());
    }

    // Build line number map
    let mut line_map = Vec::new();
    line_map.push(0);
    for (idx, ch) in contents.char_indices() {
        if ch == '\n' {
            line_map.push(idx + 1);
        }
    }

    let mut findings_by_line: std::collections::BTreeMap<usize, Vec<_>> =
        std::collections::BTreeMap::new();
    for (byte_idx, ch, name) in findings {
        let line_num = line_map
            .iter()
            .rposition(|&line_start| line_start <= byte_idx)
            .map(|i| i + 1)
            .unwrap_or(1);
        findings_by_line
            .entry(line_num)
            .or_insert_with(Vec::new)
            .push((byte_idx, ch, name));
    }

    let level = lint_level.to_diagnostic_level();
    let emitted_source = LINT.emitted_source(lint_level, reason);

    for (line_idx, line_findings) in findings_by_line.values().enumerate() {
        if lint_level.is_error() {
            *error_count += 1;
        }

        let title = LINT.desc.to_string();

        // Build snippet with multiple annotations
        let labels: Vec<String> = line_findings
            .iter()
            .map(|(_, ch, name)| format!(r#"`\u{{{:04X}}}` ({})"#, *ch as u32, name))
            .collect();

        let mut snippet = Snippet::source(contents).path(&manifest_path_str);
        for ((byte_idx, ch, _), label) in line_findings.iter().zip(labels.iter()) {
            let span = *byte_idx..(*byte_idx + ch.len_utf8());
            snippet = snippet.annotation(AnnotationKind::Primary.span(span).label(label));
        }

        let mut group = Group::with_title(level.clone().primary_title(&title)).element(snippet);

        if line_idx == 0 {
            group = group.element(Level::NOTE.message(&emitted_source));
            group = group.element(Level::NOTE.message(
                "these kinds of unicode codepoints change the way text flows in applications/editors that support them, but can cause confusion because they change the order of characters on the screen",
            ));
        }
        group = group.element(
            Level::HELP.message("if their presence wasn't intentional, you can remove them"),
        );

        gctx.shell().print_report(&[group], lint_level.force())?;
    }

    Ok(())
}
