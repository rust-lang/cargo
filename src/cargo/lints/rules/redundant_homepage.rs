use std::path::Path;

use annotate_snippets::AnnotationKind;
use annotate_snippets::Group;
use annotate_snippets::Level;
use annotate_snippets::Origin;
use annotate_snippets::Patch;
use annotate_snippets::Snippet;
use cargo_util_schemas::manifest::InheritableField;
use cargo_util_schemas::manifest::TomlToolLints;

use crate::CargoResult;
use crate::GlobalContext;
use crate::core::Package;
use crate::lints::Lint;
use crate::lints::LintLevel;
use crate::lints::LintLevelReason;
use crate::lints::STYLE;
use crate::lints::get_key_value_span;
use crate::lints::rel_cwd_manifest_path;

pub const LINT: Lint = Lint {
    name: "redundant_homepage",
    desc: "`package.homepage` is redundant with another manifest field",
    primary_group: &STYLE,
    edition_lint_opts: None,
    feature_gate: None,
    docs: Some(
        r#"
### What it does

Checks if the value of `package.homepage` is already covered by another field.

See also [`package.homepage` reference documentation](manifest.md#the-homepage-field).

### Why it is bad

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

pub fn redundant_homepage(
    pkg: &Package,
    manifest_path: &Path,
    cargo_lints: &TomlToolLints,
    error_count: &mut usize,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let (lint_level, reason) = LINT.level(
        cargo_lints,
        pkg.manifest().edition(),
        pkg.manifest().unstable_features(),
    );

    if lint_level == LintLevel::Allow {
        return Ok(());
    }

    let manifest_path = rel_cwd_manifest_path(manifest_path, gctx);

    lint_package(pkg, &manifest_path, lint_level, reason, error_count, gctx)
}

pub fn lint_package(
    pkg: &Package,
    manifest_path: &str,
    lint_level: LintLevel,
    reason: LintLevelReason,
    error_count: &mut usize,
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
    let emitted_source = LINT.emitted_source(lint_level, reason);

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
    if let Some(document) = document
        && let Some(contents) = contents
        && let Some(span) = get_key_value_span(document, &["package", "homepage"])
    {
        let span = if let Some(workspace_span) =
            get_key_value_span(document, &["package", "homepage", "workspace"])
        {
            span.key.start..workspace_span.value.end
        } else {
            span.key.start..span.value.end
        };
        let mut help =
            Group::with_title(Level::HELP.secondary_title("consider removing `package.homepage`"));
        help = help.element(
            Snippet::source(contents)
                .path(manifest_path)
                .patch(Patch::new(span, "")),
        );
        report.push(help);
    }

    if lint_level.is_error() {
        *error_count += 1;
    }
    gctx.shell().print_report(&report, lint_level.force())?;

    Ok(())
}
