use std::path::Path;

use cargo_util_schemas::manifest;

use crate::CargoResult;
use crate::GlobalContext;
use crate::core::MaybePackage;
use crate::core::Package;
use crate::core::Workspace;
use crate::diagnostics::GlobalDiagnosticStats;
use crate::diagnostics::Lint;
use crate::diagnostics::LintLevel;
use crate::diagnostics::LintLevelProduct;
use crate::diagnostics::ManifestFor;
use crate::diagnostics::PassOutput;
use crate::diagnostics::ScopedDiagnosticStats;

#[derive(Clone)]
pub enum ParsePassRule<'r> {
    DiagnosticManifest {
        rule: FnDiagnosticManifest,
    },
    LintManifest {
        rule: FnLintManifest,
        lint: &'r Lint,
    },
    DiagnosticWorkspace {
        rule: FnDiagnosticWorkspace,
    },
    LintWorkspace {
        rule: FnLintWorkspace,
        lint: &'r Lint,
    },
    DiagnosticPackage {
        rule: FnDiagnosticPackage,
    },
    LintPackage {
        rule: FnLintPackage,
        lint: &'r Lint,
    },
}

type FnDiagnosticManifest = fn(
    &Workspace<'_>,
    ManifestFor<'_>,
    &Path,
    &mut ScopedDiagnosticStats<'_>,
    &GlobalContext,
) -> CargoResult<()>;

type FnDiagnosticWorkspace = fn(
    &Workspace<'_>,
    &MaybePackage,
    &Path,
    &mut ScopedDiagnosticStats<'_>,
    &GlobalContext,
) -> CargoResult<()>;

type FnDiagnosticPackage = fn(
    &Workspace<'_>,
    &Package,
    &Path,
    &mut ScopedDiagnosticStats<'_>,
    &GlobalContext,
) -> CargoResult<()>;

type FnLintManifest = fn(
    &Workspace<'_>,
    manifest: ManifestFor<'_>,
    manifest_path: &Path,
    LintLevelProduct,
    stats: &mut ScopedDiagnosticStats<'_>,
    gctx: &GlobalContext,
) -> CargoResult<()>;

type FnLintWorkspace = fn(
    &Workspace<'_>,
    &MaybePackage,
    &Path,
    LintLevelProduct,
    &mut ScopedDiagnosticStats<'_>,
    &GlobalContext,
) -> CargoResult<()>;

type FnLintPackage = fn(
    &Workspace<'_>,
    &Package,
    &Path,
    LintLevelProduct,
    &mut ScopedDiagnosticStats<'_>,
    &GlobalContext,
) -> CargoResult<()>;

pub fn emit_parse_diagnostics(
    workspace: &Workspace<'_>,
    rules: &[ParsePassRule<'_>],
) -> CargoResult<PassOutput> {
    let mut stats = GlobalDiagnosticStats::new();

    if is_local_workspace(workspace) {
        emit_parse_ws_diagnostics(workspace, rules, &mut stats)?;
    }

    for maybe_pkg in workspace.loaded_maybe() {
        if let MaybePackage::Package(pkg) = maybe_pkg {
            if is_local_package(pkg) {
                let path = pkg.manifest_path();
                emit_parse_pkg_diagnostics(workspace, pkg, &path, rules, &mut stats)?;
            }
        }
    }

    stats.ok()
}

fn is_local_workspace(workspace: &Workspace<'_>) -> bool {
    workspace
        .root_maybe()
        .as_package()
        .map(is_local_package)
        .unwrap_or_else(|| workspace.members().any(is_local_package))
}

fn is_local_package(pkg: &Package) -> bool {
    pkg.package_id().source_id().is_path()
}

fn emit_parse_pkg_diagnostics(
    workspace: &Workspace<'_>,
    pkg: &Package,
    path: &Path,
    rules: &[ParsePassRule<'_>],
    global_stats: &mut GlobalDiagnosticStats,
) -> CargoResult<()> {
    let mut pkg_stats = global_stats.scope();

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
                let manifest = pkg.into();
                rule(workspace, manifest, &path, &mut pkg_stats, workspace.gctx())?;
            }
            ParsePassRule::LintManifest { rule, lint } => {
                if workspace.gctx().cli_unstable().cargo_lints {
                    let manifest: ManifestFor<'_> = pkg.into();
                    let level = manifest.lint_level(&cargo_lints, lint, workspace.gctx());
                    if level.level != LintLevel::Allow {
                        rule(
                            workspace,
                            manifest,
                            &path,
                            level,
                            &mut pkg_stats,
                            workspace.gctx(),
                        )?;
                    }
                }
            }
            ParsePassRule::DiagnosticWorkspace { .. } | ParsePassRule::LintWorkspace { .. } => {}
            ParsePassRule::DiagnosticPackage { rule } => {
                rule(workspace, pkg, &path, &mut pkg_stats, workspace.gctx())?;
            }
            ParsePassRule::LintPackage { rule, lint } => {
                if workspace.gctx().cli_unstable().cargo_lints {
                    let level = lint.level(
                        &cargo_lints,
                        pkg.rust_version(),
                        pkg.manifest().unstable_features(),
                        workspace.gctx(),
                    );

                    if level.level != LintLevel::Allow {
                        rule(
                            workspace,
                            pkg,
                            &path,
                            level,
                            &mut pkg_stats,
                            workspace.gctx(),
                        )?;
                    }
                }
            }
        }
    }

    pkg_stats.report_summary("parse", Some(&*pkg.name()), workspace.gctx())?;

    Ok(())
}

fn emit_parse_ws_diagnostics(
    workspace: &Workspace<'_>,
    rules: &[ParsePassRule<'_>],
    global_stats: &mut GlobalDiagnosticStats,
) -> CargoResult<()> {
    let mut pkg_stats = global_stats.scope();

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
                let manifest = (workspace, workspace.root_maybe()).into();
                rule(
                    workspace,
                    manifest,
                    workspace.root_manifest(),
                    &mut pkg_stats,
                    workspace.gctx(),
                )?;
            }
            ParsePassRule::LintManifest { rule, lint } => {
                if workspace.gctx().cli_unstable().cargo_lints {
                    let manifest: ManifestFor<'_> = (workspace, workspace.root_maybe()).into();
                    let level = manifest.lint_level(&cargo_lints, lint, workspace.gctx());
                    if level.level != LintLevel::Allow {
                        rule(
                            workspace,
                            manifest,
                            workspace.root_manifest(),
                            level,
                            &mut pkg_stats,
                            workspace.gctx(),
                        )?;
                    }
                }
            }
            ParsePassRule::DiagnosticWorkspace { rule } => {
                rule(
                    workspace,
                    workspace.root_maybe(),
                    workspace.root_manifest(),
                    &mut pkg_stats,
                    workspace.gctx(),
                )?;
            }
            ParsePassRule::LintWorkspace { rule, lint } => {
                if workspace.gctx().cli_unstable().cargo_lints {
                    let level = lint.level(
                        &cargo_lints,
                        workspace.lowest_rust_version(),
                        workspace.root_maybe().unstable_features(),
                        workspace.gctx(),
                    );
                    if level.level != LintLevel::Allow {
                        rule(
                            workspace,
                            workspace.root_maybe(),
                            workspace.root_manifest(),
                            level,
                            &mut pkg_stats,
                            workspace.gctx(),
                        )?;
                    }
                }
            }
            ParsePassRule::DiagnosticPackage { .. } | ParsePassRule::LintPackage { .. } => {}
        }
    }

    pkg_stats.report_summary("parse", None, workspace.gctx())?;
    Ok(())
}
