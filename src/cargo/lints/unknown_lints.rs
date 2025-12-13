use annotate_snippets::AnnotationKind;
use annotate_snippets::Group;
use annotate_snippets::Level;
use annotate_snippets::Snippet;
use cargo_util_schemas::manifest::TomlToolLints;

use crate::CargoResult;
use crate::GlobalContext;
use crate::lints::LINT_GROUPS;
use crate::lints::LINTS;
use crate::lints::Lint;
use crate::lints::LintLevel;
use crate::lints::ManifestFor;
use crate::lints::get_key_value_span;

pub const LINT: Lint = Lint {
    name: "unknown_lints",
    desc: "unknown lint",
    groups: &[],
    default_level: LintLevel::Warn,
    edition_lint_opts: None,
    feature_gate: None,
    docs: Some(
        r#"
### What it does
Checks for unknown lints in the `[lints.cargo]` table

### Why it is bad
- The lint name could be misspelled, leading to confusion as to why it is
  not working as expected
- The unknown lint could end up causing an error if `cargo` decides to make
  a lint with the same name in the future

### Example
```toml
[lints.cargo]
this-lint-does-not-exist = "warn"
```
"#,
    ),
};

pub fn output_unknown_lints(
    unknown_lints: Vec<&String>,
    manifest: &ManifestFor<'_>,
    manifest_path: &str,
    cargo_lints: &TomlToolLints,
    error_count: &mut usize,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let (lint_level, reason) = manifest.lint_level(cargo_lints, LINT);
    if lint_level == LintLevel::Allow {
        return Ok(());
    }

    let document = manifest.document();
    let contents = manifest.contents();

    let level = lint_level.to_diagnostic_level();
    let mut emitted_source = None;
    for lint_name in unknown_lints {
        if lint_level.is_error() {
            *error_count += 1;
        }
        let title = format!("{}: `{lint_name}`", LINT.desc);
        let underscore_lint_name = lint_name.replace("-", "_");
        let matching = if let Some(lint) = LINTS.iter().find(|l| l.name == underscore_lint_name) {
            Some((lint.name, "lint"))
        } else if let Some(group) = LINT_GROUPS.iter().find(|g| g.name == underscore_lint_name) {
            Some((group.name, "group"))
        } else {
            None
        };
        let help =
            matching.map(|(name, kind)| format!("there is a {kind} with a similar name: `{name}`"));

        let key_path = match manifest {
            ManifestFor::Package(_) => &["lints", "cargo", lint_name][..],
            ManifestFor::Workspace(_) => &["workspace", "lints", "cargo", lint_name][..],
        };
        let Some(span) = get_key_value_span(document, key_path) else {
            // This lint is handled by either package or workspace lint.
            return Ok(());
        };

        let mut report = Vec::new();
        let mut group = Group::with_title(level.clone().primary_title(title)).element(
            Snippet::source(contents)
                .path(manifest_path)
                .annotation(AnnotationKind::Primary.span(span.key)),
        );
        if emitted_source.is_none() {
            emitted_source = Some(LINT.emitted_source(lint_level, reason));
            group = group.element(Level::NOTE.message(emitted_source.as_ref().unwrap()));
        }
        if let Some(help) = help.as_ref() {
            group = group.element(Level::HELP.message(help));
        }
        report.push(group);

        gctx.shell().print_report(&report, lint_level.force())?;
    }

    Ok(())
}
