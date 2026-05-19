use std::path::Path;

use cargo_util_schemas::manifest;

use crate::CargoResult;
use crate::GlobalContext;
use crate::core::MaybePackage;
use crate::core::Package;
use crate::core::Workspace;
use crate::diagnostics::DiagnosticStats;
use crate::diagnostics::ManifestFor;

pub enum ParsePassRule {
    DiagnosticManifest { rule: FnDiagnosticManifest },
    LintManifest { rule: FnLintManifest },
    DiagnosticWorkspace { rule: FnDiagnosticWorkspace },
    LintWorkspace { rule: FnLintWorkspace },
    DiagnosticPackage { rule: FnDiagnosticPackage },
    LintPackage { rule: FnLintPackage },
}

type FnDiagnosticManifest =
    fn(ManifestFor<'_>, &Path, &mut DiagnosticStats, &GlobalContext) -> CargoResult<()>;

type FnDiagnosticWorkspace = fn(
    &Workspace<'_>,
    &MaybePackage,
    &Path,
    &mut DiagnosticStats,
    &GlobalContext,
) -> CargoResult<()>;

type FnDiagnosticPackage =
    fn(&Workspace<'_>, &Package, &Path, &mut DiagnosticStats, &GlobalContext) -> CargoResult<()>;

type FnLintManifest = fn(
    manifest: ManifestFor<'_>,
    manifest_path: &Path,
    cargo_lints: &manifest::TomlToolLints,
    stats: &mut DiagnosticStats,
    gctx: &GlobalContext,
) -> CargoResult<()>;

type FnLintWorkspace = fn(
    &Workspace<'_>,
    &MaybePackage,
    &Path,
    &manifest::TomlToolLints,
    &mut DiagnosticStats,
    &GlobalContext,
) -> CargoResult<()>;

type FnLintPackage = fn(
    &Workspace<'_>,
    &Package,
    &Path,
    &manifest::TomlToolLints,
    &mut DiagnosticStats,
    &GlobalContext,
) -> CargoResult<()>;

pub fn emit_parse_diagnostics(
    workspace: &Workspace<'_>,
    rules: &[ParsePassRule],
) -> CargoResult<()> {
    let mut first_emitted_error = None;

    if let Err(e) = emit_parse_ws_diagnostics(workspace, rules) {
        first_emitted_error = Some(e);
    }

    for maybe_pkg in workspace.loaded_maybe() {
        if let MaybePackage::Package(pkg) = maybe_pkg {
            let path = pkg.manifest_path();
            if let Err(e) = emit_parse_pkg_diagnostics(workspace, pkg, &path, rules)
                && first_emitted_error.is_none()
            {
                first_emitted_error = Some(e);
            }
        }
    }

    if let Some(error) = first_emitted_error {
        Err(error)
    } else {
        Ok(())
    }
}

fn emit_parse_pkg_diagnostics(
    workspace: &Workspace<'_>,
    pkg: &Package,
    path: &Path,
    rules: &[ParsePassRule],
) -> CargoResult<()> {
    let mut stats = DiagnosticStats::new();

    let toml_lints = pkg
        .manifest()
        .normalized_toml()
        .lints
        .clone()
        .map(|lints| lints.lints)
        .unwrap_or(manifest::TomlLints::default());
    let cargo_lints = toml_lints
        .get("cargo")
        .cloned()
        .unwrap_or(manifest::TomlToolLints::default());

    for rule in rules {
        match rule {
            ParsePassRule::DiagnosticManifest { rule } => {
                rule(pkg.into(), &path, &mut stats, workspace.gctx())?;
            }
            ParsePassRule::LintManifest { rule } => {
                if workspace.gctx().cli_unstable().cargo_lints {
                    rule(
                        pkg.into(),
                        &path,
                        &cargo_lints,
                        &mut stats,
                        workspace.gctx(),
                    )?;
                }
            }
            ParsePassRule::DiagnosticWorkspace { .. } | ParsePassRule::LintWorkspace { .. } => {}
            ParsePassRule::DiagnosticPackage { rule } => {
                rule(workspace, pkg, &path, &mut stats, workspace.gctx())?;
            }
            ParsePassRule::LintPackage { rule } => {
                if workspace.gctx().cli_unstable().cargo_lints {
                    rule(
                        workspace,
                        pkg,
                        &path,
                        &cargo_lints,
                        &mut stats,
                        workspace.gctx(),
                    )?;
                }
            }
        }
    }

    stats.report_summary("parse", Some(&*pkg.name()), workspace.gctx())?;

    Ok(())
}

fn emit_parse_ws_diagnostics(
    workspace: &Workspace<'_>,
    rules: &[ParsePassRule],
) -> CargoResult<()> {
    let mut stats = DiagnosticStats::new();

    let cargo_lints = match workspace.root_maybe() {
        MaybePackage::Package(pkg) => {
            let toml = pkg.manifest().normalized_toml();
            if let Some(ws) = &toml.workspace {
                ws.lints.as_ref()
            } else {
                toml.lints.as_ref().map(|l| &l.lints)
            }
        }
        MaybePackage::Virtual(vm) => vm
            .normalized_toml()
            .workspace
            .as_ref()
            .unwrap()
            .lints
            .as_ref(),
    }
    .and_then(|t| t.get("cargo"))
    .cloned()
    .unwrap_or(manifest::TomlToolLints::default());

    for rule in rules {
        match rule {
            ParsePassRule::DiagnosticManifest { rule } => {
                rule(
                    (workspace, workspace.root_maybe()).into(),
                    workspace.root_manifest(),
                    &mut stats,
                    workspace.gctx(),
                )?;
            }
            ParsePassRule::LintManifest { rule } => {
                if workspace.gctx().cli_unstable().cargo_lints {
                    rule(
                        (workspace, workspace.root_maybe()).into(),
                        workspace.root_manifest(),
                        &cargo_lints,
                        &mut stats,
                        workspace.gctx(),
                    )?;
                }
            }
            ParsePassRule::DiagnosticWorkspace { rule } => {
                rule(
                    workspace,
                    workspace.root_maybe(),
                    workspace.root_manifest(),
                    &mut stats,
                    workspace.gctx(),
                )?;
            }
            ParsePassRule::LintWorkspace { rule } => {
                if workspace.gctx().cli_unstable().cargo_lints {
                    rule(
                        workspace,
                        workspace.root_maybe(),
                        workspace.root_manifest(),
                        &cargo_lints,
                        &mut stats,
                        workspace.gctx(),
                    )?;
                }
            }
            ParsePassRule::DiagnosticPackage { .. } | ParsePassRule::LintPackage { .. } => {}
        }
    }

    stats.report_summary("parse", None, workspace.gctx())?;
    Ok(())
}
