use std::path::Path;

use cargo_util_schemas::manifest;

use crate::CargoResult;
use crate::GlobalContext;
use crate::core::MaybePackage;
use crate::core::Package;
use crate::core::Workspace;
use crate::diagnostics::DiagnosticStats;
use crate::diagnostics::Lint;
use crate::diagnostics::LintLevel;
use crate::diagnostics::LintLevelProduct;
use crate::diagnostics::ManifestFor;

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
    LintLevelProduct,
    stats: &mut DiagnosticStats,
    gctx: &GlobalContext,
) -> CargoResult<()>;

type FnLintWorkspace = fn(
    &Workspace<'_>,
    &MaybePackage,
    &Path,
    LintLevelProduct,
    &mut DiagnosticStats,
    &GlobalContext,
) -> CargoResult<()>;

type FnLintPackage = fn(
    &Workspace<'_>,
    &Package,
    &Path,
    LintLevelProduct,
    &mut DiagnosticStats,
    &GlobalContext,
) -> CargoResult<()>;

pub fn emit_parse_diagnostics(
    workspace: &Workspace<'_>,
    rules: &[ParsePassRule<'_>],
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
    rules: &[ParsePassRule<'_>],
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
                let manifest = pkg.into();
                rule(manifest, &path, &mut stats, workspace.gctx())?;
            }
            ParsePassRule::LintManifest { rule, lint } => {
                if workspace.gctx().cli_unstable().cargo_lints {
                    let manifest: ManifestFor<'_> = pkg.into();
                    let level = manifest.lint_level(&cargo_lints, lint);
                    if level.level != LintLevel::Allow {
                        rule(manifest, &path, level, &mut stats, workspace.gctx())?;
                    }
                }
            }
            ParsePassRule::DiagnosticWorkspace { .. } | ParsePassRule::LintWorkspace { .. } => {}
            ParsePassRule::DiagnosticPackage { rule } => {
                rule(workspace, pkg, &path, &mut stats, workspace.gctx())?;
            }
            ParsePassRule::LintPackage { rule, lint } => {
                if workspace.gctx().cli_unstable().cargo_lints {
                    let level = lint.level(
                        &cargo_lints,
                        pkg.rust_version(),
                        pkg.manifest().unstable_features(),
                    );

                    if level.level != LintLevel::Allow {
                        rule(workspace, pkg, &path, level, &mut stats, workspace.gctx())?;
                    }
                }
            }
        }
    }

    stats.report_summary("parse", Some(&*pkg.name()), workspace.gctx())?;

    Ok(())
}

fn emit_parse_ws_diagnostics(
    workspace: &Workspace<'_>,
    rules: &[ParsePassRule<'_>],
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
                let manifest = (workspace, workspace.root_maybe()).into();
                rule(
                    manifest,
                    workspace.root_manifest(),
                    &mut stats,
                    workspace.gctx(),
                )?;
            }
            ParsePassRule::LintManifest { rule, lint } => {
                if workspace.gctx().cli_unstable().cargo_lints {
                    let manifest: ManifestFor<'_> = (workspace, workspace.root_maybe()).into();
                    let level = manifest.lint_level(&cargo_lints, lint);
                    if level.level != LintLevel::Allow {
                        rule(
                            manifest,
                            workspace.root_manifest(),
                            level,
                            &mut stats,
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
                    &mut stats,
                    workspace.gctx(),
                )?;
            }
            ParsePassRule::LintWorkspace { rule, lint } => {
                if workspace.gctx().cli_unstable().cargo_lints {
                    let level = lint.level(
                        &cargo_lints,
                        workspace.lowest_rust_version(),
                        workspace.root_maybe().unstable_features(),
                    );
                    if level.level != LintLevel::Allow {
                        rule(
                            workspace,
                            workspace.root_maybe(),
                            workspace.root_manifest(),
                            level,
                            &mut stats,
                            workspace.gctx(),
                        )?;
                    }
                }
            }
            ParsePassRule::DiagnosticPackage { .. } | ParsePassRule::LintPackage { .. } => {}
        }
    }

    stats.report_summary("parse", None, workspace.gctx())?;
    Ok(())
}
