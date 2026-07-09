use std::path::Path;

use cargo_util_terminal::report::AnnotationKind;
use cargo_util_terminal::report::Group;
use cargo_util_terminal::report::Level;
use cargo_util_terminal::report::Origin;
use cargo_util_terminal::report::Patch;
use cargo_util_terminal::report::Snippet;
use tracing::instrument;

use super::RESTRICTION;
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
    name: "non_kebab_case_features",
    desc: "features should have a kebab-case name",
    primary_group: &RESTRICTION,
    msrv: None,
    feature_gate: None,
    docs: Some(
        r#"
### What it does

Detect feature names that are not kebab-case.

### Why restrict this

Having multiple naming styles within a workspace can be confusing.

### Drawbacks

Users would expect that a feature tightly coupled to a dependency would match the dependency's name.

### Example

```toml
[features]
foo_bar = []
```

Should be written as:

```toml
[features]
foo-bar = []
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
    for original_name in pkg.summary().features().keys() {
        let original_name = &**original_name;
        let kebab_case = heck::ToKebabCase::to_kebab_case(original_name);
        if kebab_case == original_name {
            continue;
        }

        let manifest = pkg.manifest();
        let document = manifest.document();
        let contents = manifest.contents();
        let level = lint_level.to_diagnostic_level();
        let emitted_source = LINT.emitted_source(lint_level, source);

        let mut primary = Group::with_title(level.primary_title(LINT.desc));
        if let Some(document) = document
            && let Some(contents) = contents
            && let Some(span) = get_key_value_span(document, &["features", original_name])
        {
            primary = primary.element(
                Snippet::source(contents)
                    .path(manifest_path)
                    .annotation(AnnotationKind::Primary.span(span.key)),
            );
        } else if let Some(document) = document
            && let Some(contents) = contents
            && let Some(dep_span) = get_key_value_span(document, &["dependencies", original_name])
            && let Some(optional_span) =
                get_key_value_span(document, &["dependencies", original_name, "optional"])
        {
            primary = primary.element(
                Snippet::source(contents)
                    .path(manifest_path)
                    .annotation(AnnotationKind::Primary.span(dep_span.key).label("source of feature name"))
                    .annotation(
                        AnnotationKind::Context
                            .span(optional_span.key.start..optional_span.value.end)
                            .label("cause of feature"),
                    ),
            ).element(Level::NOTE.message("see also <https://doc.rust-lang.org/cargo/reference/features.html#optional-dependencies>"));
        } else {
            primary = primary.element(Origin::path(manifest_path));
        }
        primary = primary.element(Level::NOTE.message(emitted_source));
        let mut report = vec![primary];
        if let Some(document) = document
            && let Some(contents) = contents
            && let Some(span) = get_key_value_span(document, &["features", original_name])
        {
            let mut help = Group::with_title(Level::HELP.secondary_title(
                "to change the feature name to kebab case, convert the `features` key",
            ));
            help = help.element(
                Snippet::source(contents)
                    .path(manifest_path)
                    .patch(Patch::new(span.key, kebab_case.as_str())),
            );
            report.push(help);
        }

        pkg_stats.record_lint(lint_level);
        gctx.shell().print_report(&report, lint_level.force())?;
    }

    Ok(())
}
