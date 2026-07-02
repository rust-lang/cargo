use std::path::Path;

use cargo_util_terminal::report::{AnnotationKind, Group, Level, Origin, Snippet};
use tracing::instrument;

use super::SUSPICIOUS;
use crate::diagnostics::{
    Lint, LintLevelProduct, ScopedDiagnosticStats, get_key_value_span, workspace_rel_path,
};
use crate::{
    CargoResult, GlobalContext,
    core::{MaybePackage, Workspace},
};

pub static LINT: &Lint = &Lint {
    name: "unused_workspace_exclude",
    desc: "unused workspace exclude",
    primary_group: &SUSPICIOUS,
    msrv: Some(super::CARGO_LINTS_MSRV),
    feature_gate: None,
    docs: Some(
        r#"
### What it does
Checks for any entry in `[workspace.exclude]` that does not match any workspace member

### Why it is bad
They can give the false impression that a package is excluded when it is actually not present

### Example
```toml
[workspace]
exclude = ["does-not-exist"]
```
"#,
    ),
};

#[instrument(skip_all)]
pub(crate) fn lint_workspace(
    ws: &Workspace<'_>,
    maybe_pkg: &MaybePackage,
    manifest_path: &Path,
    level: LintLevelProduct,
    pkg_stats: &mut ScopedDiagnosticStats<'_>,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let LintLevelProduct {
        level: lint_level,
        source,
    } = level;

    let Some(original_toml) = maybe_pkg.original_toml() else {
        return Ok(());
    };

    let Some(workspace) = original_toml.workspace.as_ref() else {
        return Ok(());
    };

    let Some(exclude) = workspace.exclude.as_ref() else {
        return Ok(());
    };

    let used_exclude_patterns = ws.used_exclude_patterns();

    for (i, unused) in exclude
        .iter()
        .filter(|pattern| !used_exclude_patterns.contains(*pattern))
        .enumerate()
    {
        let document = maybe_pkg.document();
        let contents = maybe_pkg.contents();
        let level = lint_level.to_diagnostic_level();
        let manifest_path = workspace_rel_path(ws, manifest_path);
        let emitted_source = LINT.emitted_source(lint_level, source);

        let mut primary =
            Group::with_title(level.primary_title(format!("unused exclude pattern '{}'", unused)));
        if let Some(document) = document
            && let Some(contents) = contents
        {
            let mut snippet = Snippet::source(contents).path(&manifest_path);
            if let Some(span) = get_key_value_span(document, &["workspace", "exclude"]) {
                snippet = snippet.annotation(AnnotationKind::Primary.span(span.key));
            }
            primary = primary.element(snippet);
        } else {
            primary = primary.element(Origin::path(&manifest_path));
        }
        if i == 0 {
            primary = primary.element(Level::NOTE.message(emitted_source));
        }
        let report = vec![primary];

        pkg_stats.record_lint(lint_level);
        gctx.shell().print_report(&report, lint_level.force())?;
    }

    Ok(())
}
