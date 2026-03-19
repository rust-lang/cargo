use std::path::Path;

use cargo_util_schemas::manifest::InheritableField;
use cargo_util_schemas::manifest::StringOrBool;
use cargo_util_schemas::manifest::TomlToolLints;
use cargo_util_terminal::report::AnnotationKind;
use cargo_util_terminal::report::Group;
use cargo_util_terminal::report::Level;
use cargo_util_terminal::report::Origin;
use cargo_util_terminal::report::Patch;
use cargo_util_terminal::report::Snippet;

use crate::CargoResult;
use crate::GlobalContext;
use crate::core::Package;
use crate::lints::Lint;
use crate::lints::LintLevel;
use crate::lints::LintLevelSource;
use crate::lints::STYLE;
use crate::lints::get_key_value_span;
use crate::lints::rel_cwd_manifest_path;
use crate::util::toml::DEFAULT_README_FILES;

pub static LINT: &Lint = &Lint {
    name: "redundant_readme",
    desc: "explicit `package.readme` can be inferred",
    primary_group: &STYLE,
    msrv: Some(super::CARGO_LINTS_MSRV),
    feature_gate: None,
    docs: Some(
        r#"
### What it does

Checks for `package.readme` fields that can be inferred.

See also [`package.readme` reference documentation](manifest.md#the-readme-field).

### Why it is bad

Adds boilerplate.

### Drawbacks

It might not be obvious if they named their file correctly.

### Example

```toml
[package]
name = "foo"
readme = "README.md"
```

Should be written as:

```toml
[package]
name = "foo"
```
"#,
    ),
};

pub fn redundant_readme(
    pkg: &Package,
    manifest_path: &Path,
    cargo_lints: &TomlToolLints,
    error_count: &mut usize,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let (lint_level, source) = LINT.level(
        cargo_lints,
        pkg.rust_version(),
        pkg.manifest().unstable_features(),
    );

    if lint_level == LintLevel::Allow {
        return Ok(());
    }

    let manifest_path = rel_cwd_manifest_path(manifest_path, gctx);

    lint_package(pkg, &manifest_path, lint_level, source, error_count, gctx)
}

pub fn lint_package(
    pkg: &Package,
    manifest_path: &str,
    lint_level: LintLevel,
    source: LintLevelSource,
    error_count: &mut usize,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let manifest = pkg.manifest();

    // Must check `original_toml`, before any inferring happened
    let Some(original_toml) = manifest.original_toml() else {
        return Ok(());
    };
    let Some(original_pkg) = &original_toml.package else {
        return Ok(());
    };
    let Some(readme) = &original_pkg.readme else {
        return Ok(());
    };

    let InheritableField::Value(StringOrBool::String(readme)) = readme else {
        // Not checking inheritance because at most one package can be identified from the lint and
        // consistency of inheritance is likely best.
        return Ok(());
    };

    if !DEFAULT_README_FILES.contains(&readme.as_str()) {
        return Ok(());
    }

    let document = manifest.document();
    let contents = manifest.contents();
    let level = lint_level.to_diagnostic_level();
    let emitted_source = LINT.emitted_source(lint_level, source);

    let mut primary = Group::with_title(level.primary_title(LINT.desc));
    if let Some(document) = document
        && let Some(contents) = contents
        && let Some(span) = get_key_value_span(document, &["package", "readme"])
    {
        let span = span.key.start..span.value.end;
        primary = primary.element(
            Snippet::source(contents)
                .path(manifest_path)
                .annotation(AnnotationKind::Primary.span(span)),
        );
    } else {
        primary = primary.element(Origin::path(manifest_path));
    }
    primary = primary.element(Level::NOTE.message(emitted_source));
    let mut report = vec![primary];
    if let Some(document) = document
        && let Some(contents) = contents
        && let Some(span) = get_key_value_span(document, &["package", "readme"])
    {
        let span = if let Some(workspace_span) =
            get_key_value_span(document, &["package", "readme", "workspace"])
        {
            span.key.start..workspace_span.value.end
        } else {
            span.key.start..span.value.end
        };
        let mut help =
            Group::with_title(Level::HELP.secondary_title("consider removing `package.readme`"));
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
