use std::path::Path;

use cargo_util_schemas::manifest;
use cargo_util_terminal::report::AnnotationKind;
use cargo_util_terminal::report::Group;
use cargo_util_terminal::report::Level;
use cargo_util_terminal::report::Snippet;
use tracing::instrument;

use super::find_lint_or_group;
use crate::CargoResult;
use crate::GlobalContext;
use crate::core::Feature;
use crate::core::MaybePackage;
use crate::diagnostics::DiagnosticStats;
use crate::diagnostics::ManifestFor;
use crate::diagnostics::get_key_value_span;
use crate::diagnostics::rel_cwd_manifest_path;

#[instrument(skip_all)]
pub(crate) fn diagnose_manifest(
    manifest: ManifestFor<'_>,
    manifest_path: &Path,
    stats: &mut DiagnosticStats,
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
        diagnose_manifest_inner(&manifest, manifest_path, cargo_lints, stats, gctx)?;
    }
    if let Some(cargo_lints) = pkg_lints {
        diagnose_manifest_inner(&manifest, manifest_path, cargo_lints, stats, gctx)?;
    }

    Ok(())
}

fn diagnose_manifest_inner(
    manifest: &ManifestFor<'_>,
    manifest_path: &Path,
    cargo_lints: &manifest::TomlToolLints,
    stats: &mut DiagnosticStats,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let manifest_path = rel_cwd_manifest_path(manifest_path, gctx);
    for lint_name in cargo_lints.keys().map(|name| name) {
        let Some((name, default_level, feature_gate)) = find_lint_or_group(lint_name) else {
            continue;
        };

        let (_, source, _) =
            crate::diagnostics::lint::level_priority(name, *default_level, cargo_lints);

        // Only run analysis on user-specified lints
        if !source.is_user_specified() {
            continue;
        }

        // Only run this on lints that are gated by a feature
        if let Some(feature_gate) = feature_gate
            && !manifest.unstable_features().is_enabled(feature_gate)
        {
            report_feature_not_enabled(name, feature_gate, &manifest, &manifest_path, stats, gctx)?;
        }
    }

    Ok(())
}

fn report_feature_not_enabled(
    lint_name: &str,
    feature_gate: &Feature,
    manifest: &ManifestFor<'_>,
    manifest_path: &str,
    stats: &mut DiagnosticStats,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let dash_feature_name = feature_gate.name().replace("_", "-");

    let mut error = Group::with_title(
        Level::ERROR.primary_title(format!("use of unstable lint `{lint_name}`")),
    );

    if let Some(document) = manifest.document()
        && let Some(contents) = manifest.contents()
    {
        let key_path = match manifest {
            ManifestFor::Package(_) => &["lints", "cargo", lint_name][..],
            ManifestFor::Workspace { .. } => &["workspace", "lints", "cargo", lint_name][..],
        };
        let Some(span) = get_key_value_span(document, key_path) else {
            // This lint is handled by either package or workspace lint.
            return Ok(());
        };

        error = error.element(Snippet::source(contents).path(manifest_path).annotation(
            AnnotationKind::Primary.span(span.key).label(format!(
                "this is behind `{dash_feature_name}`, which is not enabled"
            )),
        ))
    }

    let report = [error.element(Level::HELP.message(format!(
        "consider adding `cargo-features = [\"{dash_feature_name}\"]` to the top of the manifest"
    )))];

    stats.record_error();
    gctx.shell().print_report(&report, true)?;

    Ok(())
}
