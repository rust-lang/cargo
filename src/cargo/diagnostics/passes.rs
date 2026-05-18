use std::path::Path;

use cargo_util_schemas::manifest;

use crate::CargoResult;
use crate::core::MaybePackage;
use crate::core::Package;
use crate::core::Workspace;
use crate::diagnostics::DiagnosticStats;
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

    deferred_parse_diagnostics(pkg.into(), path, &mut stats, workspace.gctx())?;

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

    if workspace.gctx().cli_unstable().cargo_lints {
        missing_lints_features(
            pkg.into(),
            &path,
            &cargo_lints,
            &mut stats,
            workspace.gctx(),
        )?;
        unknown_lints(
            pkg.into(),
            &path,
            &cargo_lints,
            &mut stats,
            workspace.gctx(),
        )?;

        check_im_a_teapot(pkg, &path, &cargo_lints, &mut stats, workspace.gctx())?;
        implicit_minimum_version_req_pkg(pkg, &path, &cargo_lints, &mut stats, workspace.gctx())?;
        non_kebab_case_packages(pkg, &path, &cargo_lints, &mut stats, workspace.gctx())?;
        non_snake_case_packages(pkg, &path, &cargo_lints, &mut stats, workspace.gctx())?;
        non_kebab_case_bins(
            workspace,
            pkg,
            &path,
            &cargo_lints,
            &mut stats,
            workspace.gctx(),
        )?;
        non_kebab_case_features(pkg, &path, &cargo_lints, &mut stats, workspace.gctx())?;
        non_snake_case_features(pkg, &path, &cargo_lints, &mut stats, workspace.gctx())?;
        unused_build_dependencies_no_build_rs(
            pkg,
            &path,
            &cargo_lints,
            &mut stats,
            workspace.gctx(),
        )?;
        redundant_readme(pkg, &path, &cargo_lints, &mut stats, workspace.gctx())?;
        redundant_homepage(pkg, &path, &cargo_lints, &mut stats, workspace.gctx())?;
        missing_lints_inheritance(
            workspace,
            pkg,
            &path,
            &cargo_lints,
            &mut stats,
            workspace.gctx(),
        )?;
        text_direction_codepoint_in_comment(
            pkg.into(),
            &path,
            &cargo_lints,
            &mut stats,
            workspace.gctx(),
        )?;
        text_direction_codepoint_in_literal(
            pkg.into(),
            &path,
            &cargo_lints,
            &mut stats,
            workspace.gctx(),
        )?;
    }

    stats.report_summary("parse", Some(&*pkg.name()), workspace.gctx())?;

    Ok(())
}

fn emit_parse_ws_diagnostics(workspace: &Workspace<'_>) -> CargoResult<()> {
    let mut stats = DiagnosticStats::new();

    deferred_parse_diagnostics(
        (workspace, workspace.root_maybe()).into(),
        workspace.root_manifest(),
        &mut stats,
        workspace.gctx(),
    )?;

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

    if workspace.gctx().cli_unstable().cargo_lints {
        missing_lints_features(
            (workspace, workspace.root_maybe()).into(),
            workspace.root_manifest(),
            &cargo_lints,
            &mut stats,
            workspace.gctx(),
        )?;
        unknown_lints(
            (workspace, workspace.root_maybe()).into(),
            workspace.root_manifest(),
            &cargo_lints,
            &mut stats,
            workspace.gctx(),
        )?;

        unused_workspace_package_fields(
            workspace,
            workspace.root_maybe(),
            workspace.root_manifest(),
            &cargo_lints,
            &mut stats,
            workspace.gctx(),
        )?;
        unused_workspace_dependencies(
            workspace,
            workspace.root_maybe(),
            workspace.root_manifest(),
            &cargo_lints,
            &mut stats,
            workspace.gctx(),
        )?;
        implicit_minimum_version_req_ws(
            workspace,
            workspace.root_maybe(),
            workspace.root_manifest(),
            &cargo_lints,
            &mut stats,
            workspace.gctx(),
        )?;
        text_direction_codepoint_in_comment(
            (workspace, workspace.root_maybe()).into(),
            workspace.root_manifest(),
            &cargo_lints,
            &mut stats,
            workspace.gctx(),
        )?;
        text_direction_codepoint_in_literal(
            (workspace, workspace.root_maybe()).into(),
            workspace.root_manifest(),
            &cargo_lints,
            &mut stats,
            workspace.gctx(),
        )?;
    }

    // This is a short term hack to allow `blanket_hint_mostly_unused`
    // to run without requiring `-Zcargo-lints`, which should hopefully
    // improve the testing experience while we are collecting feedback
    if workspace.gctx().cli_unstable().profile_hint_mostly_unused {
        blanket_hint_mostly_unused(
            workspace,
            workspace.root_maybe(),
            workspace.root_manifest(),
            &cargo_lints,
            &mut stats,
            workspace.gctx(),
        )?;
    }

    stats.report_summary("parse", None, workspace.gctx())?;
    Ok(())
}
