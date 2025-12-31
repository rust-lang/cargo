use std::path::Path;

use annotate_snippets::AnnotationKind;
use annotate_snippets::Group;
use annotate_snippets::Level;
use annotate_snippets::Snippet;
use cargo_util_schemas::manifest::TomlToolLints;

use crate::CargoResult;
use crate::GlobalContext;
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
    docs: Some(
        r#"
### What it does
Checks for Unicode codepoints in `Cargo.toml` that change the visual
representation of text on screen in a way that does not correspond to
their on memory representation.

### Why it is bad
The Unicode characters `\u{202A}`, `\u{202B}`, `\u{202C}`, `\u{202D}`,
`\u{202E}`, `\u{2066}`, `\u{2067}`, `\u{2068}`, and `\u{2069}` make the
flow of text on screen change its direction. This makes the text "abc"
display as "cba" on screen. By leveraging these, people can write specially
crafted text that makes the surrounding manifest content seem like it's
specifying one thing, when in reality it is specifying another.

This is known as a "Trojan Source" attack (CVE-2021-42574).

### Example
A malicious `Cargo.toml` could contain invisible Unicode control characters
that reorder how text is displayed, making a malicious dependency appear
as a comment or vice versa.

See [CVE-2021-42574](https://cve.mitre.org/cgi-bin/cvename.cgi?name=CVE-2021-42574) for more details.
"#,
    ),
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

    let contents = manifest.contents();
    let manifest_path_str = rel_cwd_manifest_path(manifest_path, gctx);

    // Find all occurrences of BiDi codepoints
    let mut findings: Vec<(usize, char, &str)> = Vec::new();

    for (byte_idx, ch) in contents.char_indices() {
        if let Some((_, name)) = UNICODE_BIDI_CODEPOINTS.iter().find(|(c, _)| *c == ch) {
            findings.push((byte_idx, ch, name));
        }
    }

    if findings.is_empty() {
        return Ok(());
    }

    let level = lint_level.to_diagnostic_level();
    let emitted_source = LINT.emitted_source(lint_level, reason);

    for (i, (byte_idx, ch, name)) in findings.iter().enumerate() {
        if lint_level.is_error() {
            *error_count += 1;
        }

        let title = format!(
            "{}: `\\u{{{:04X}}}` ({})",
            LINT.desc,
            *ch as u32,
            name
        );

        // The span covers just the single character
        let span = *byte_idx..(*byte_idx + ch.len_utf8());

        let label = format!(
            "this invisible unicode codepoint changes text flow direction"
        );

        let help = "if their presence wasn't intentional, you can remove them";

        let mut group = Group::with_title(level.clone().primary_title(&title)).element(
            Snippet::source(contents)
                .path(&manifest_path_str)
                .annotation(AnnotationKind::Primary.span(span).label(&label)),
        );

        // Only emit the source note on the first finding
        if i == 0 {
            group = group.element(Level::NOTE.message(&emitted_source));
        }

        group = group.element(Level::HELP.message(help));

        gctx.shell().print_report(&[group], lint_level.force())?;
    }

    Ok(())
}

