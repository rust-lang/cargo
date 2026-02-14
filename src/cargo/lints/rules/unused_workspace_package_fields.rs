use std::path::Path;

use annotate_snippets::AnnotationKind;
use annotate_snippets::Group;
use annotate_snippets::Level;
use annotate_snippets::Origin;
use annotate_snippets::Patch;
use annotate_snippets::Snippet;
use cargo_util_schemas::manifest::TomlToolLints;
use indexmap::IndexSet;

use crate::CargoResult;
use crate::GlobalContext;
use crate::core::MaybePackage;
use crate::core::Workspace;
use crate::lints::Lint;
use crate::lints::LintLevel;
use crate::lints::SUSPICIOUS;
use crate::lints::get_key_value_span;
use crate::lints::rel_cwd_manifest_path;

pub static LINT: &Lint = &Lint {
    name: "unused_workspace_package_fields",
    desc: "unused field in `workspace.package`",
    primary_group: &SUSPICIOUS,
    msrv: Some(super::CARGO_LINTS_MSRV),
    edition_lint_opts: None,
    feature_gate: None,
    docs: Some(
        r#"
### What it does
Checks for any fields in `[workspace.package]` that has not been inherited

### Why it is bad
They can give the false impression that these fields are used

### Example
```toml
[workspace.package]
edition = "2024"

[package]
name = "foo"
```
"#,
    ),
};

pub fn unused_workspace_package_fields(
    ws: &Workspace<'_>,
    maybe_pkg: &MaybePackage,
    manifest_path: &Path,
    cargo_lints: &TomlToolLints,
    error_count: &mut usize,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let (lint_level, reason) = LINT.level(
        cargo_lints,
        ws.lowest_rust_version(),
        maybe_pkg.edition(),
        maybe_pkg.unstable_features(),
    );
    if lint_level == LintLevel::Allow {
        return Ok(());
    }

    let workspace_package_fields: IndexSet<_> = maybe_pkg
        .document()
        .and_then(|d| d.get_ref().get("workspace"))
        .and_then(|w| w.get_ref().get("package"))
        .and_then(|p| p.get_ref().as_table())
        .iter()
        .flat_map(|d| d.keys())
        .collect();

    let mut inherited_fields = IndexSet::new();
    for member in ws.members() {
        inherited_fields.extend(
            member
                .manifest()
                .document()
                .and_then(|w| w.get_ref().get("package"))
                .and_then(|p| p.get_ref().as_table())
                .iter()
                .flat_map(|d| {
                    d.iter()
                        .filter(|(_, v)| {
                            v.get_ref()
                                .get("workspace")
                                .and_then(|w| w.get_ref().as_bool())
                                == Some(true)
                        })
                        .map(|(k, _)| k)
                }),
        );
    }

    for (i, unused) in workspace_package_fields
        .difference(&inherited_fields)
        .enumerate()
    {
        let document = maybe_pkg.document();
        let contents = maybe_pkg.contents();
        let level = lint_level.to_diagnostic_level();
        let manifest_path = rel_cwd_manifest_path(manifest_path, gctx);
        let emitted_source = LINT.emitted_source(lint_level, reason);

        let mut primary = Group::with_title(level.primary_title(LINT.desc));
        if let Some(document) = document
            && let Some(contents) = contents
        {
            let mut snippet = Snippet::source(contents).path(&manifest_path);
            if let Some(span) =
                get_key_value_span(document, &["workspace", "package", unused.as_ref()])
            {
                snippet = snippet.annotation(AnnotationKind::Primary.span(span.key));
            }
            primary = primary.element(snippet);
        } else {
            primary = primary.element(Origin::path(&manifest_path));
        }
        if i == 0 {
            primary = primary.element(Level::NOTE.message(emitted_source));
        }
        let mut report = vec![primary];
        if let Some(document) = document
            && let Some(contents) = contents
        {
            let mut help = Group::with_title(
                Level::HELP.secondary_title("consider removing the unused field"),
            );
            let mut snippet = Snippet::source(contents).path(&manifest_path);
            if let Some(span) =
                get_key_value_span(document, &["workspace", "package", unused.as_ref()])
            {
                snippet = snippet.patch(Patch::new(span.key.start..span.value.end, ""));
            }
            help = help.element(snippet);
            report.push(help);
        }

        if lint_level.is_error() {
            *error_count += 1;
        }
        gctx.shell().print_report(&report, lint_level.force())?;
    }

    Ok(())
}
