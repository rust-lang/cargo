use std::path::Path;

use cargo_util_schemas::manifest;

use crate::CargoResult;
use crate::GlobalContext;
use crate::core::MaybePackage;
use crate::core::Package;
use crate::core::Workspace;
use crate::diagnostics::DiagnosticStats;
use crate::diagnostics::ManifestFor;
use crate::diagnostics::rules::blanket_hint_mostly_unused;
use crate::diagnostics::rules::check_im_a_teapot;
use crate::diagnostics::rules::deferred_parse_diagnostics;
use crate::diagnostics::rules::implicit_minimum_version_req_pkg;
use crate::diagnostics::rules::implicit_minimum_version_req_ws;
use crate::diagnostics::rules::missing_lints_features;
use crate::diagnostics::rules::missing_lints_inheritance;
use crate::diagnostics::rules::non_kebab_case_bins;
use crate::diagnostics::rules::non_kebab_case_features;
use crate::diagnostics::rules::non_kebab_case_packages;
use crate::diagnostics::rules::non_snake_case_features;
use crate::diagnostics::rules::non_snake_case_packages;
use crate::diagnostics::rules::redundant_homepage;
use crate::diagnostics::rules::redundant_readme;
use crate::diagnostics::rules::text_direction_codepoint_in_comment;
use crate::diagnostics::rules::text_direction_codepoint_in_literal;
use crate::diagnostics::rules::unknown_lints;
use crate::diagnostics::rules::unused_build_dependencies_no_build_rs;
use crate::diagnostics::rules::unused_workspace_dependencies;
use crate::diagnostics::rules::unused_workspace_package_fields;

pub const PARSE_PASS_RULES: &[ParsePassRule] = &[
    ParsePassRule::DiagnosticManifest {
        rule: deferred_parse_diagnostics,
    },
    ParsePassRule::LintManifest {
        rule: missing_lints_features,
    },
    ParsePassRule::LintManifest {
        rule: unknown_lints,
    },
    ParsePassRule::LintWorkspace {
        rule: unused_workspace_package_fields,
    },
    ParsePassRule::LintWorkspace {
        rule: unused_workspace_dependencies,
    },
    ParsePassRule::LintWorkspace {
        rule: implicit_minimum_version_req_ws,
    },
    ParsePassRule::LintManifest {
        rule: text_direction_codepoint_in_comment,
    },
    ParsePassRule::LintManifest {
        rule: text_direction_codepoint_in_literal,
    },
    ParsePassRule::LintWorkspace {
        rule: blanket_hint_mostly_unused,
    },
    ParsePassRule::LintPackage {
        rule: check_im_a_teapot,
    },
    ParsePassRule::LintPackage {
        rule: implicit_minimum_version_req_pkg,
    },
    ParsePassRule::LintPackage {
        rule: non_kebab_case_packages,
    },
    ParsePassRule::LintPackage {
        rule: non_snake_case_packages,
    },
    ParsePassRule::LintPackage {
        rule: non_kebab_case_bins,
    },
    ParsePassRule::LintPackage {
        rule: non_kebab_case_features,
    },
    ParsePassRule::LintPackage {
        rule: non_snake_case_features,
    },
    ParsePassRule::LintPackage {
        rule: unused_build_dependencies_no_build_rs,
    },
    ParsePassRule::LintPackage {
        rule: redundant_readme,
    },
    ParsePassRule::LintPackage {
        rule: redundant_homepage,
    },
    ParsePassRule::LintPackage {
        rule: missing_lints_inheritance,
    },
];

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

pub fn emit_parse_diagnostics(workspace: &Workspace<'_>) -> CargoResult<()> {
    let mut first_emitted_error = None;

    if let Err(e) = emit_parse_ws_diagnostics(workspace) {
        first_emitted_error = Some(e);
    }

    for maybe_pkg in workspace.loaded_maybe() {
        if let MaybePackage::Package(pkg) = maybe_pkg {
            let path = pkg.manifest_path();
            if let Err(e) = emit_parse_pkg_diagnostics(workspace, pkg, &path)
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

    for rule in PARSE_PASS_RULES {
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

fn emit_parse_ws_diagnostics(workspace: &Workspace<'_>) -> CargoResult<()> {
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

    for rule in PARSE_PASS_RULES {
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
