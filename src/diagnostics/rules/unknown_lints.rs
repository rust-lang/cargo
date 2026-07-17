use std::path::Path;

use cargo_util_schemas::manifest::TomlToolLints;
use cargo_util_terminal::report::AnnotationKind;
use cargo_util_terminal::report::Group;
use cargo_util_terminal::report::Level;
use cargo_util_terminal::report::Origin;
use cargo_util_terminal::report::Snippet;
use tracing::instrument;

use super::LINT_GROUPS;
use super::LINTS;
use super::SUSPICIOUS;
use super::find_lint_or_group;
use crate::CargoResult;
use crate::GlobalContext;
use crate::diagnostics::Lint;
use crate::diagnostics::LintLevelProduct;
use crate::diagnostics::ManifestFor;
use crate::diagnostics::ScopedDiagnosticStats;
use crate::diagnostics::get_key_value_span;
use crate::diagnostics::workspace_rel_path;
use crate::workspace::MaybePackage;
use crate::workspace::Workspace;

pub static LINT: &Lint = &Lint {
    name: "unknown_lints",
    desc: "unknown lint",
    primary_group: &SUSPICIOUS,
    msrv: Some(super::CARGO_LINTS_MSRV),
    feature_gate: None,
    docs: Some(
        r#"
### What it does
Checks for unknown lints in the `[lints.cargo]` table

### Why is this bad?
- The lint name could be misspelled, leading to confusion as to why it is
  not working as expected
- The unknown lint could end up causing an error if `cargo` decides to make
  a lint with the same name in the future

### Example
```toml
[lints.cargo]
this-lint-does-not-exist = "warn"
```
"#,
    ),
};

#[instrument(skip_all)]
pub(crate) fn lint_manifest(
    ws: &Workspace<'_>,
    manifest: ManifestFor<'_>,
    manifest_path: &Path,
    level: LintLevelProduct,
    pkg_stats: &mut ScopedDiagnosticStats<'_>,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let normalized_toml = match &manifest {
        ManifestFor::Package(pkg) => pkg.manifest().normalized_toml(),
        ManifestFor::Workspace {
            maybe_pkg: MaybePackage::Virtual(vm),
            ..
        } => vm.normalized_toml(),
        ManifestFor::Workspace {
            maybe_pkg: MaybePackage::Package(_),
            ..
        } => {
            // For real manifests, lint as a package, rather than a workspace
            return Ok(());
        }
    };

    let ws_lints = normalized_toml
        .workspace
        .as_ref()
        .and_then(|ws| ws.lints.as_ref())
        .and_then(|lints| lints.get("cargo"));
    let pkg_lints = normalized_toml
        .lints
        .as_ref()
        .map(|lints| &lints.lints)
        .and_then(|lints| lints.get("cargo"));

    if let Some(cargo_lints) = ws_lints {
        lint_manifest_inner(
            ws,
            &manifest,
            manifest_path,
            &level,
            cargo_lints,
            pkg_stats,
            gctx,
        )?;
    }
    if let Some(cargo_lints) = pkg_lints {
        lint_manifest_inner(
            ws,
            &manifest,
            manifest_path,
            &level,
            cargo_lints,
            pkg_stats,
            gctx,
        )?;
    }

    Ok(())
}

fn lint_manifest_inner(
    ws: &Workspace<'_>,
    manifest: &ManifestFor<'_>,
    manifest_path: &Path,
    level: &LintLevelProduct,
    cargo_lints: &TomlToolLints,
    pkg_stats: &mut ScopedDiagnosticStats<'_>,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let LintLevelProduct {
        level: lint_level,
        source,
    } = level;

    let manifest_path = workspace_rel_path(ws, manifest_path);
    let mut unknown_lints = Vec::new();
    for lint_name in cargo_lints.keys().map(|name| name) {
        let Some(_) = find_lint_or_group(lint_name) else {
            unknown_lints.push(lint_name);
            continue;
        };
    }

    let level = lint_level.to_diagnostic_level();
    let mut emitted_source = None;
    for lint_name in unknown_lints {
        let title = format!("{}: `{lint_name}`", LINT.desc);
        let underscore_lint_name = lint_name.replace("-", "_");
        let matching = if let Some(lint) = LINTS.iter().find(|l| l.name == underscore_lint_name) {
            Some((lint.name, "lint"))
        } else if let Some(group) = LINT_GROUPS.iter().find(|g| g.name == underscore_lint_name) {
            Some((group.name, "group"))
        } else {
            None
        };
        let help =
            matching.map(|(name, kind)| format!("there is a {kind} with a similar name: `{name}`"));

        let key_path = match manifest {
            ManifestFor::Package(_) => &["lints", "cargo", lint_name][..],
            ManifestFor::Workspace { .. } => &["workspace", "lints", "cargo", lint_name][..],
        };

        let mut report = Vec::new();
        let mut group = Group::with_title(level.clone().primary_title(title));

        if let Some(document) = manifest.document()
            && let Some(contents) = manifest.contents()
        {
            let Some(span) = get_key_value_span(document, key_path) else {
                // This lint is handled by either package or workspace lint.
                return Ok(());
            };
            group = group.element(
                Snippet::source(contents)
                    .path(&manifest_path)
                    .annotation(AnnotationKind::Primary.span(span.key)),
            );
        } else {
            group = group.element(Origin::path(&manifest_path));
        }

        if emitted_source.is_none() {
            emitted_source = Some(LINT.emitted_source(*lint_level, *source));
            group = group.element(Level::NOTE.message(emitted_source.as_ref().unwrap()));
        }
        if let Some(help) = help.as_ref() {
            group = group.element(Level::HELP.message(help));
        }
        report.push(group);

        pkg_stats.record_lint(*lint_level);
        gctx.shell().print_report(&report, lint_level.force())?;
    }

    Ok(())
}
