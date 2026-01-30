use std::path::Path;

use annotate_snippets::AnnotationKind;
use annotate_snippets::Group;
use annotate_snippets::Level;
use annotate_snippets::Origin;
use annotate_snippets::Patch;
use annotate_snippets::Snippet;
use cargo_util_schemas::manifest::InheritableDependency;
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

pub const LINT: Lint = Lint {
    name: "unused_workspace_dependencies",
    desc: "unused workspace dependency",
    primary_group: &SUSPICIOUS,
    edition_lint_opts: None,
    feature_gate: None,
    docs: Some(
        r#"
### What it does
Checks for any entry in `[workspace.dependencies]` that has not been inherited

### Why it is bad
They can give the false impression that these dependencies are used

### Example
```toml
[workspace.dependencies]
regex = "1"

[dependencies]
```
"#,
    ),
};

pub fn unused_workspace_dependencies(
    ws: &Workspace<'_>,
    maybe_pkg: &MaybePackage,
    manifest_path: &Path,
    cargo_lints: &TomlToolLints,
    error_count: &mut usize,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let (lint_level, reason) = LINT.level(
        cargo_lints,
        maybe_pkg.edition(),
        maybe_pkg.unstable_features(),
    );
    if lint_level == LintLevel::Allow {
        return Ok(());
    }

    let workspace_deps: IndexSet<_> = maybe_pkg
        .original_toml()
        .and_then(|t| t.workspace.as_ref())
        .and_then(|w| w.dependencies.as_ref())
        .iter()
        .flat_map(|d| d.keys())
        .collect();

    let mut inherited_deps = IndexSet::new();
    for member in ws.members() {
        let Some(original_toml) = member.manifest().original_toml() else {
            return Ok(());
        };
        inherited_deps.extend(
            original_toml
                .build_dependencies()
                .into_iter()
                .flatten()
                .filter(|(_, d)| is_inherited(d))
                .map(|(name, _)| name),
        );
        inherited_deps.extend(
            original_toml
                .dependencies
                .iter()
                .flatten()
                .filter(|(_, d)| is_inherited(d))
                .map(|(name, _)| name),
        );
        inherited_deps.extend(
            original_toml
                .dev_dependencies()
                .into_iter()
                .flatten()
                .filter(|(_, d)| is_inherited(d))
                .map(|(name, _)| name),
        );
        for target in original_toml.target.iter().flat_map(|t| t.values()) {
            inherited_deps.extend(
                target
                    .build_dependencies()
                    .into_iter()
                    .flatten()
                    .filter(|(_, d)| is_inherited(d))
                    .map(|(name, _)| name),
            );
            inherited_deps.extend(
                target
                    .dependencies
                    .iter()
                    .flatten()
                    .filter(|(_, d)| is_inherited(d))
                    .map(|(name, _)| name),
            );
            inherited_deps.extend(
                target
                    .dev_dependencies()
                    .into_iter()
                    .flatten()
                    .filter(|(_, d)| is_inherited(d))
                    .map(|(name, _)| name),
            );
        }
    }

    for (i, unused) in workspace_deps.difference(&inherited_deps).enumerate() {
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
                get_key_value_span(document, &["workspace", "dependencies", unused.as_str()])
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
                Level::HELP.secondary_title("consider removing the unused dependency"),
            );
            let mut snippet = Snippet::source(contents).path(&manifest_path);
            if let Some(span) =
                get_key_value_span(document, &["workspace", "dependencies", unused.as_str()])
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

fn is_inherited(dep: &InheritableDependency) -> bool {
    matches!(dep, InheritableDependency::Inherit(_))
}
