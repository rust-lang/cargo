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
use crate::core::Workspace;
use crate::lints::Lint;
use crate::lints::LintLevel;
use crate::lints::LintLevelReason;
use crate::lints::PEDANTIC;
use crate::lints::get_key_value_span;
use crate::lints::rel_cwd_manifest_path;

pub static LINT: &Lint = &Lint {
    name: "uninherited_repository",
    desc: "`package.repository` in a workspace member should be inherited from `[workspace.package]`",
    primary_group: &PEDANTIC,
    msrv: Some(super::CARGO_LINTS_MSRV),
    edition_lint_opts: None,
    feature_gate: None,
    docs: Some(
        r#"
### What it does

Checks for `package.repository` fields in workspace members that are set
explicitly instead of being inherited from `[workspace.package]`.

See also [`package.repository` reference documentation](manifest.md#the-repository-field).

### Why it is bad

A common mistake is setting `package.repository` to the URL of the git host's
file browser for the specific crate (e.g. a GitHub `/tree/` URL) rather than
the root of the repository. Moving the field to `[workspace.package]` and
inheriting it encourages using the correct root URL in one place and avoids
per-crate drift.

### Note

This lint fires for any explicit `repository` value in a workspace member,
regardless of whether the URL looks correct or not. It does not validate the
URL itself; it only checks that the field is inherited rather than set
explicitly.

### Drawbacks

A workspace that spans multiple repositories would need to suppress this lint,
since each member legitimately has a different repository URL.

### Example

```toml
[workspace]

[package]
repository = "https://github.com/rust-lang/cargo/tree/master/crates/cargo-test-macro"
```

Should be written as:

```toml
[workspace.package]
repository = "https://github.com/rust-lang/cargo"

[package]
repository.workspace = true
```
"#,
    ),
};

pub fn uninherited_repository(
    ws: &Workspace<'_>,
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

    // Use normalized_toml here: we only need to know whether a [workspace]
    // section exists at all.
    let has_workspace = ws.root_maybe().normalized_toml().workspace.is_some();

    if !has_workspace {
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

    // Use original_toml so we see the raw InheritableField before workspace
    // inheritance has been resolved into a plain Value.
    let Some(original_toml) = manifest.original_toml() else {
        return Ok(());
    };
    let Some(original_pkg) = &original_toml.package else {
        return Ok(());
    };
    let Some(repository) = &original_pkg.repository else {
        return Ok(());
    };

    let InheritableField::Value(_) = repository else {
        // Already inheriting from workspace so nothing to do.
        return Ok(());
    };

    let document = manifest.document();
    let contents = manifest.contents();
    let level = lint_level.to_diagnostic_level();
    let emitted_source = LINT.emitted_source(lint_level, reason);

    let mut primary = Group::with_title(level.primary_title(LINT.desc));
    if let Some(document) = document
        && let Some(contents) = contents
        && let Some(span) = get_key_value_span(document, &["package", "repository"])
    {
        primary = primary.element(
            Snippet::source(contents)
                .path(manifest_path)
                .annotation(AnnotationKind::Primary.span(span.key.start..span.value.end)),
        );
    } else {
        primary = primary.element(Origin::path(manifest_path));
    }
    primary = primary.element(Level::NOTE.message(emitted_source));
    let mut report = vec![primary];

    if let Some(document) = document
        && let Some(contents) = contents
        && let Some(span) = get_key_value_span(document, &["package", "repository"])
    {
        let remove_span = span.key.start..span.value.end;
        let mut help = Group::with_title(Level::HELP.secondary_title(
            "consider moving `repository` to `[workspace.package]` and inheriting it",
        ));
        help = help.element(
            Snippet::source(contents)
                .path(manifest_path)
                .patch(Patch::new(remove_span, "repository.workspace = true")),
        );
        report.push(help);
    }

    if lint_level.is_error() {
        *error_count += 1;
    }
    gctx.shell().print_report(&report, lint_level.force())?;

    Ok(())
}
