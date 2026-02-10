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

pub static LINT: &Lint = &Lint {
    name: "non_kebab_case_packages",
    desc: "packages should have a kebab-case name",
    primary_group: &RESTRICTION,
    msrv: None,
    edition_lint_opts: None,
    feature_gate: None,
    docs: Some(
        r#"
### What it does

Detect package names that are not kebab-case.

### Why it is bad

Having multiple naming styles within a workspace can be confusing.

### Drawbacks

Users have to mentally translate package names to namespaces in Rust.

### Example

```toml
[package]
name = "foo_bar"
```

Should be written as:

```toml
[package]
name = "foo-bar"
```
"#,
    ),
};

pub fn non_kebab_case_packages(
    pkg: &Package,
    manifest_path: &Path,
    cargo_lints: &TomlToolLints,
    error_count: &mut usize,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let (lint_level, reason) = LINT.level(
        cargo_lints,
        pkg.rust_version(),
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

    let original_name = &*manifest.name();
    let kebab_case = heck::ToKebabCase::to_kebab_case(original_name);
    if kebab_case == original_name {
        return Ok(());
    }

    let document = manifest.document();
    let contents = manifest.contents();
    let level = lint_level.to_diagnostic_level();
    let emitted_source = LINT.emitted_source(lint_level, reason);

    let mut primary = Group::with_title(level.primary_title(LINT.desc));
    if let Some(document) = document
        && let Some(contents) = contents
        && let Some(span) = get_key_value_span(document, &["package", "name"])
    {
        primary = primary.element(
            Snippet::source(contents)
                .path(manifest_path)
                .annotation(AnnotationKind::Primary.span(span.value)),
        );
    } else {
        primary = primary.element(Origin::path(manifest_path));
    }
    primary = primary.element(Level::NOTE.message(emitted_source));
    let mut report = vec![primary];
    if let Some(document) = document
        && let Some(contents) = contents
        && let Some(span) = get_key_value_span(document, &["package", "name"])
    {
        let mut help =
            Group::with_title(Level::HELP.secondary_title(
                "to change the package name to kebab case, convert `package.name`",
            ));
        help = help.element(
            Snippet::source(contents)
                .path(manifest_path)
                .patch(Patch::new(span.value, format!("\"{kebab_case}\""))),
        );
        report.push(help);
    } else {
        let path = pkg.manifest_path();
        let display_path = path.as_os_str().to_string_lossy();
        let end = display_path.len() - if display_path.ends_with(".rs") { 3 } else { 0 };
        let start = path
            .parent()
            .map(|p| {
                let p = p.as_os_str().to_string_lossy();
                // Account for trailing slash that was removed
                p.len() + if p.is_empty() { 0 } else { 1 }
            })
            .unwrap_or(0);
        let help = Level::HELP
            .secondary_title("to change the package name to kebab case, convert the file stem")
            .element(Snippet::source(display_path).patch(Patch::new(start..end, kebab_case)));
        report.push(help);
    }

    if lint_level.is_error() {
        *error_count += 1;
    }
    gctx.shell().print_report(&report, lint_level.force())?;

    Ok(())
}
