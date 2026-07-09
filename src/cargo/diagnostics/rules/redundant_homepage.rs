use std::path::Path;

use cargo_util_schemas::manifest::InheritableField;
use cargo_util_terminal::report::AnnotationKind;
use cargo_util_terminal::report::Group;
use cargo_util_terminal::report::Level;
use cargo_util_terminal::report::Origin;
use cargo_util_terminal::report::Snippet;
use tracing::instrument;

use super::STYLE;
use crate::CargoResult;
use crate::GlobalContext;
use crate::core::Package;
use crate::core::Workspace;
use crate::diagnostics::Lint;
use crate::diagnostics::LintLevel;
use crate::diagnostics::LintLevelProduct;
use crate::diagnostics::LintLevelSource;
use crate::diagnostics::ScopedDiagnosticStats;
use crate::diagnostics::get_key_value_span;
use crate::diagnostics::workspace_rel_path;

pub static LINT: &Lint = &Lint {
    name: "redundant_homepage",
    desc: "`package.homepage` is redundant with another manifest field",
    primary_group: &STYLE,
    msrv: Some(super::CARGO_LINTS_MSRV),
    feature_gate: None,
    docs: Some(
        r#"
### What it does

Checks if the value of `package.homepage` is already covered by another field.

See also [`package.homepage` reference documentation](manifest.md#the-homepage-field).

### Why is this bad?

When package browsers render each link, a redundant link adds visual noise.

### Drawbacks

### Example

```toml
[package]
name = "foo"
homepage = "https://github.com/rust-lang/cargo/"
repository = "https://github.com/rust-lang/cargo/"
```

Should be written as:

```toml
[package]
name = "foo"
repository = "https://github.com/rust-lang/cargo/"
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
    let LintLevelProduct {
        level: lint_level,
        source,
    } = level;

    let manifest_path = workspace_rel_path(ws, manifest_path);

    lint_package_inner(pkg, &manifest_path, lint_level, source, pkg_stats, gctx)
}

fn lint_package_inner(
    pkg: &Package,
    manifest_path: &str,
    lint_level: LintLevel,
    source: LintLevelSource,
    pkg_stats: &mut ScopedDiagnosticStats<'_>,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let manifest = pkg.manifest();

    let Some(normalized_pkg) = &manifest.normalized_toml().package else {
        return Ok(());
    };
    let Some(InheritableField::Value(homepage)) = &normalized_pkg.homepage else {
        return Ok(());
    };

    let other_field = if let Some(InheritableField::Value(repository)) = &normalized_pkg.repository
        && repository == homepage
    {
        "repository"
    } else if let Some(InheritableField::Value(documentation)) = &normalized_pkg.documentation
        && documentation == homepage
    {
        "documentation"
    } else {
        return Ok(());
    };

    let document = manifest.document();
    let contents = manifest.contents();
    let level = lint_level.to_diagnostic_level();
    let emitted_source = LINT.emitted_source(lint_level, source);

    let mut primary = Group::with_title(level.primary_title(LINT.desc));
    if let Some(document) = document
        && let Some(contents) = contents
        && let Some(span) = get_key_value_span(document, &["package", "homepage"])
    {
        let mut snippet = Snippet::source(contents)
            .path(manifest_path)
            .annotation(AnnotationKind::Primary.span(span.value));
        if let Some(span) = get_key_value_span(document, &["package", other_field]) {
            snippet = snippet.annotation(AnnotationKind::Context.span(span.value));
        }
        primary = primary.element(snippet);
    } else {
        primary = primary.element(Origin::path(manifest_path));
    }
    primary = primary.element(Level::NOTE.message(emitted_source));
    let mut report = vec![primary];
    let help =
        Group::with_title(Level::HELP.secondary_title("consider removing `package.homepage`"));
    report.push(help);

    pkg_stats.record_lint(lint_level);
    gctx.shell().print_report(&report, lint_level.force())?;

    Ok(())
}
