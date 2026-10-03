use std::path::Path;

use cargo_util_schemas::manifest::InheritableField;
use cargo_util_terminal::report::{AnnotationKind, Group, Level, Origin, Snippet};
use tracing::instrument;

use super::PEDANTIC;
use crate::diagnostics::{
    Lint, LintLevelProduct, ScopedDiagnosticStats, get_key_value_span, workspace_rel_path,
};
use crate::workspace::{Package, Workspace};
use crate::{CargoResult, GlobalContext};

pub static LINT: &Lint = &Lint {
    name: "repository_not_inherited",
    primary_group: &PEDANTIC,
    msrv: Some(super::CARGO_LINTS_MSRV),
    feature_gate: None,
    docs: Some(
        r#"
### What it does

Checks for a directly set `package.repository` in an explicit workspace.

### Why is this bad?

Package-specific repository links can point to a file browser rather than a cloneable repository.
Inheriting a workspace repository can help keep these links consistent.

### Drawbacks

Workspace members can intentionally use different repositories.

### Example

```toml
[workspace]

[workspace.package]
repository = "https://github.com/rust-lang/cargo"

[package]
name = "example"
repository = "https://github.com/rust-lang/cargo/tree/master/crates/example"
```

Consider writing:

```toml
[package]
name = "example"
repository.workspace = true
```
"#,
    ),
};

#[instrument(skip_all)]
pub(crate) fn lint_package(
    ws: &Workspace<'_>,
    pkg: &Package,
    manifest_path: &Path,
    level: LintLevelProduct,
    pkg_stats: &mut ScopedDiagnosticStats<'_>,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    if !ws.is_member(pkg) {
        return Ok(());
    }
    let Some(workspace) = ws
        .root_maybe()
        .original_toml()
        .and_then(|root| root.workspace.as_ref())
    else {
        return Ok(());
    };
    let manifest = pkg.manifest();
    let Some(InheritableField::Value(_)) = manifest
        .original_toml()
        .and_then(|toml| toml.package.as_ref())
        .and_then(|package| package.repository.as_ref())
    else {
        return Ok(());
    };

    let manifest_path = workspace_rel_path(ws, manifest_path);
    let mut primary = Group::with_title(
        level
            .level
            .to_diagnostic_level()
            .primary_title("`package.repository` is not inherited from the workspace"),
    );
    if let Some(document) = manifest.document()
        && let Some(contents) = manifest.contents()
        && let Some(span) = get_key_value_span(document, &["package", "repository"])
    {
        primary = primary.element(
            Snippet::source(contents)
                .path(&manifest_path)
                .annotation(AnnotationKind::Primary.span(span.value)),
        );
    } else {
        primary = primary.element(Origin::path(&manifest_path));
    }
    primary = primary.element(Level::NOTE.message(LINT.emitted_source(level.level, level.source)));

    let help = if workspace
        .package
        .as_ref()
        .and_then(|package| package.repository.as_ref())
        .is_some()
    {
        "consider using `repository.workspace = true` if the workspace repository applies"
    } else {
        "consider defining `workspace.package.repository` and inheriting it where appropriate"
    };
    let report = [
        primary,
        Group::with_title(Level::HELP.secondary_title(help)),
    ];
    pkg_stats.record_lint(level.level);
    gctx.shell().print_report(&report, level.level.force())?;
    Ok(())
}
