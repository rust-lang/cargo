//! Implementation of `cargo info`.

use anyhow::bail;
use cargo_util_schemas::core::{PackageIdSpec, PartialVersion};

use crate::core::registry::PackageRegistry;
use crate::core::{Dependency, Package, PackageId, PackageIdSpecQuery, Registry, Workspace};
use crate::ops::registry::info::view::pretty_view;
use crate::ops::registry::{RegistryOrIndex, RegistrySourceIds, get_source_id_with_package_id};
use crate::ops::resolve_ws;
use crate::sources::source::QueryKind;
use crate::sources::{IndexSummary, SourceConfigMap};
use crate::util::cache_lock::CacheLockMode;
use crate::util::command_prelude::root_manifest;
use crate::{CargoResult, GlobalContext};

mod view;

pub fn info(
    spec: &PackageIdSpec,
    gctx: &GlobalContext,
    reg_or_index: Option<RegistryOrIndex>,
) -> CargoResult<()> {
    let source_config = SourceConfigMap::new(gctx)?;
    let mut registry = PackageRegistry::new_with_source_config(gctx, source_config)?;
    // Make sure we get the lock before we download anything.
    let _lock = gctx.acquire_package_cache_lock(CacheLockMode::DownloadExclusive)?;
    registry.lock_patches();

    // If we can find it in workspace, use it as a specific version.
    let nearest_manifest_path = root_manifest(None, gctx).ok();
    let ws = nearest_manifest_path
        .as_ref()
        .and_then(|root| Workspace::new(root, gctx).ok());
    validate_locked_and_frozen_options(ws.is_some(), gctx)?;
    let nearest_package = ws.as_ref().and_then(|ws| {
        nearest_manifest_path
            .as_ref()
            .and_then(|path| ws.members().find(|p| p.manifest_path() == path))
    });
    let (mut package_id, is_member) = find_pkgid_in_ws(nearest_package, ws.as_ref(), spec);
    let (use_package_source_id, source_ids) =
        get_source_id_with_package_id(gctx, package_id, reg_or_index.as_ref())?;
    // If we don't use the package's source, we need to query the package ID from the specified registry.
    if !use_package_source_id {
        package_id = None;
    }

    let msrv_from_nearest_manifest_path_or_ws =
        try_get_msrv_from_nearest_manifest_or_ws(nearest_package, ws.as_ref());
    // If the workspace does not have a specific Rust version,
    // or if the command is not called within the workspace, then fallback to the global Rust version.
    let rustc_version = match msrv_from_nearest_manifest_path_or_ws {
        Some(msrv) => msrv,
        None => {
            let current_rustc = gctx.load_global_rustc(ws.as_ref())?.version;
            // Remove any pre-release identifiers for easier comparison.
            // Otherwise, the MSRV check will fail if the current Rust version is a nightly or beta version.
            semver::Version::new(
                current_rustc.major,
                current_rustc.minor,
                current_rustc.patch,
            )
            .into()
        }
    };
    // Only suggest cargo tree command when the package is not a workspace member.
    // For workspace members, `cargo tree --package <SPEC> --invert` is useless. It only prints itself.
    let suggest_cargo_tree_command = package_id.is_some() && !is_member;

    let summaries = query_summaries(spec, &mut registry, &source_ids)?;
    let package_id = match package_id {
        Some(id) => id,
        None => find_pkgid_in_summaries(&summaries, spec, &rustc_version, &source_ids)?,
    };

    let package = registry.get(&[package_id])?;
    let package = package.get_one(package_id)?;
    pretty_view(package, &summaries, suggest_cargo_tree_command, gctx)?;

    Ok(())
}

fn find_pkgid_in_ws(
    nearest_package: Option<&Package>,
    ws: Option<&Workspace<'_>>,
    spec: &PackageIdSpec,
) -> (Option<PackageId>, bool) {
    let Some(ws) = ws else {
        return (None, false);
    };

    if let Some(member) = ws.members().find(|p| spec.matches(p.package_id())) {
        return (Some(member.package_id()), true);
    }

    let Ok((_, resolve)) = resolve_ws(ws, false) else {
        return (None, false);
    };

    if let Some(package_id) = nearest_package
        .map(|p| p.package_id())
        .into_iter()
        .flat_map(|p| resolve.deps(p))
        .map(|(p, _)| p)
        .filter(|&p| spec.matches(p))
        .max_by_key(|&p| p.version())
    {
        return (Some(package_id), false);
    }

    if let Some(package_id) = ws
        .members()
        .map(|p| p.package_id())
        .flat_map(|p| resolve.deps(p))
        .map(|(p, _)| p)
        .filter(|&p| spec.matches(p))
        .max_by_key(|&p| p.version())
    {
        return (Some(package_id), false);
    }

    if let Some(package_id) = resolve
        .iter()
        .filter(|&p| spec.matches(p))
        .max_by_key(|&p| p.version())
    {
        return (Some(package_id), false);
    }

    (None, false)
}

fn find_pkgid_in_summaries(
    summaries: &[IndexSummary],
    spec: &PackageIdSpec,
    rustc_version: &PartialVersion,
    source_ids: &RegistrySourceIds,
) -> CargoResult<PackageId> {
    let summary = summaries
        .iter()
        .filter(|s| spec.matches(s.package_id()))
        .max_by(|s1, s2| {
            // Check the MSRV compatibility.
            let s1_matches = s1
                .as_summary()
                .rust_version()
                .map(|v| v.is_compatible_with(rustc_version))
                .unwrap_or_else(|| false);
            let s2_matches = s2
                .as_summary()
                .rust_version()
                .map(|v| v.is_compatible_with(rustc_version))
                .unwrap_or_else(|| false);
            // MSRV compatible version is preferred.
            match (s1_matches, s2_matches) {
                (true, false) => std::cmp::Ordering::Greater,
                (false, true) => std::cmp::Ordering::Less,
                // If both summaries match the current Rust version or neither do, try to
                // pick the latest version.
                _ => s1.package_id().version().cmp(s2.package_id().version()),
            }
        });

    match summary {
        Some(summary) => Ok(summary.package_id()),
        None => {
            anyhow::bail!(
                "could not find `{}` in registry `{}`",
                spec,
                source_ids.original.url()
            )
        }
    }
}

fn query_summaries(
    spec: &PackageIdSpec,
    registry: &mut PackageRegistry<'_>,
    source_ids: &RegistrySourceIds,
) -> CargoResult<Vec<IndexSummary>> {
    // Query without version requirement to get all index summaries.
    let dep = Dependency::parse(spec.name(), None, source_ids.original)?;
    loop {
        // Exact to avoid returning all for path/git
        match registry.query_vec(&dep, QueryKind::Exact) {
            std::task::Poll::Ready(res) => {
                break res;
            }
            std::task::Poll::Pending => registry.block_until_ready()?,
        }
    }
}

fn validate_locked_and_frozen_options(
    in_workspace: bool,
    gctx: &GlobalContext,
) -> Result<(), anyhow::Error> {
    // Only in workspace, we can use --frozen or --locked.
    if !in_workspace {
        if let Some(locked_flag) = gctx.locked_flag() {
            bail!("the option `{locked_flag}` can only be used within a workspace");
        }
    }
    Ok(())
}

fn try_get_msrv_from_nearest_manifest_or_ws(
    nearest_package: Option<&Package>,
    ws: Option<&Workspace<'_>>,
) -> Option<PartialVersion> {
    // Try to get the MSRV from the nearest manifest.
    let rust_version = nearest_package.and_then(|p| p.rust_version().map(|v| v.as_partial()));
    // If the nearest manifest does not have a specific Rust version, try to get it from the workspace.
    rust_version
        .or_else(|| ws.and_then(|ws| ws.lowest_rust_version().map(|v| v.as_partial())))
        .cloned()
}
