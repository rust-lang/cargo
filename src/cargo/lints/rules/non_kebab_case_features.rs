use std::path::Path;

use annotate_snippets::AnnotationKind;
use annotate_snippets::Group;
use annotate_snippets::Level;
use annotate_snippets::Origin;
use annotate_snippets::Patch;
use annotate_snippets::Snippet;
use cargo_util_schemas::manifest::TomlToolLints;

use crate::CargoResult;
use crate::GlobalContext;
use crate::core::Package;
use crate::lints::Lint;
use crate::lints::LintLevel;
use crate::lints::LintLevelReason;
use crate::lints::RESTRICTION;
use crate::lints::get_key_value_span;
use crate::lints::rel_cwd_manifest_path;

pub const LINT: Lint = Lint {
    name: "non_kebab_case_features",
    desc: "features should have a kebab-case name",
    primary_group: &RESTRICTION,
    edition_lint_opts: None,
    feature_gate: None,
    docs: Some(
        r#"
### What it does

Detect feature names that are not kebab-case.

### Why it is bad

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

pub fn non_kebab_case_features(
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
        let emitted_source = LINT.emitted_source(lint_level, reason);

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

        if lint_level.is_error() {
            *error_count += 1;
        }
        gctx.shell().print_report(&report, lint_level.force())?;
    }

    Ok(())
}
