use crate::core::Registry as _;
use crate::core::dependency::Dependency;
use crate::core::registry::PackageRegistry;
use crate::core::resolver::features::{CliFeatures, HasDevUnits};
use crate::core::shell::Verbosity;
use crate::core::{PackageId, PackageIdSpec, PackageIdSpecQuery};
use crate::core::{Resolve, SourceId, Workspace};
use crate::ops;
use crate::sources::IndexSummary;
use crate::sources::source::QueryKind;
use crate::util::cache_lock::CacheLockMode;
use crate::util::context::GlobalContext;
use crate::util::toml_mut::dependency::{MaybeWorkspace, Source};
use crate::util::toml_mut::manifest::LocalManifest;
use crate::util::toml_mut::upgrade::upgrade_requirement;
use crate::util::{CargoResult, VersionExt};
use crate::util::{OptVersionReq, style};
use anyhow::Context as _;
use cargo_util_schemas::core::PartialVersion;
use indexmap::IndexMap;
use itertools::Itertools;
use semver::{Op, Version, VersionReq};
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};
use tracing::{debug, trace};

pub type UpgradeMap = HashMap<(String, SourceId), Version>;

pub struct UpdateOptions<'a> {
    pub gctx: &'a GlobalContext,
    pub to_update: Vec<String>,
    pub precise: Option<&'a str>,
    pub recursive: bool,
    pub dry_run: bool,
    pub workspace: bool,
}

pub fn generate_lockfile(ws: &Workspace<'_>) -> CargoResult<()> {
    let mut registry = ws.package_registry()?;
    let previous_resolve = None;
    let mut resolve = ops::resolve_with_previous(
        &mut registry,
        ws,
        &CliFeatures::new_all(true),
        HasDevUnits::Yes,
        previous_resolve,
        None,
        &[],
        true,
    )?;
    ops::write_pkg_lockfile(ws, &mut resolve)?;
    print_lockfile_changes(ws, previous_resolve, &resolve, &mut registry)?;
    Ok(())
}

pub fn update_lockfile(ws: &Workspace<'_>, opts: &UpdateOptions<'_>) -> CargoResult<()> {
    if opts.recursive && opts.precise.is_some() {
        anyhow::bail!("cannot specify both recursive and precise simultaneously")
    }

    if ws.members().count() == 0 {
        anyhow::bail!("you can't generate a lockfile for an empty workspace.")
    }

    // Updates often require a lot of modifications to the registry, so ensure
    // that we're synchronized against other Cargos.
    let _lock = ws
        .gctx()
        .acquire_package_cache_lock(CacheLockMode::DownloadExclusive)?;

    let previous_resolve = match ops::load_pkg_lockfile(ws)? {
        Some(resolve) => resolve,
        None => {
            match opts.precise {
                None => return generate_lockfile(ws),

                // Precise option specified, so calculate a previous_resolve required
                // by precise package update later.
                Some(_) => {
                    let mut registry = ws.package_registry()?;
                    ops::resolve_with_previous(
                        &mut registry,
                        ws,
                        &CliFeatures::new_all(true),
                        HasDevUnits::Yes,
                        None,
                        None,
                        &[],
                        true,
                    )?
                }
            }
        }
    };
    let mut registry = ws.package_registry()?;
    let mut to_avoid = HashSet::new();

    if opts.to_update.is_empty() {
        if !opts.workspace {
            to_avoid.extend(previous_resolve.iter());
            to_avoid.extend(previous_resolve.unused_patches());
        }
    } else {
        let mut sources = Vec::new();
        for name in opts.to_update.iter() {
            let pid = previous_resolve.query(name)?;
            if opts.recursive {
                fill_with_deps(&previous_resolve, pid, &mut to_avoid, &mut HashSet::new());
            } else {
                to_avoid.insert(pid);
                sources.push(match opts.precise {
                    Some(precise) => {
                        // TODO: see comment in `resolve.rs` as well, but this
                        //       seems like a pretty hokey reason to single out
                        //       the registry as well.
                        if pid.source_id().is_registry() {
                            pid.source_id().with_precise_registry_version(
                                pid.name(),
                                pid.version().clone(),
                                precise,
                            )?
                        } else {
                            pid.source_id().with_git_precise(Some(precise.to_string()))
                        }
                    }
                    None => pid.source_id().without_precise(),
                });
            }
            if let Ok(unused_id) =
                PackageIdSpec::query_str(name, previous_resolve.unused_patches().iter().cloned())
            {
                to_avoid.insert(unused_id);
            }
        }

        // Mirror `--workspace` and never avoid workspace members.
        // Filtering them out here so the above processes them normally
        // so their dependencies can be updated as requested
        to_avoid.retain(|id| {
            for package in ws.members() {
                let member_id = package.package_id();
                // Skip checking the `version` because `previous_resolve` might have a stale
                // value.
                // When dealing with workspace members, the other fields should be a
                // sufficiently unique match.
                if id.name() == member_id.name() && id.source_id() == member_id.source_id() {
                    return false;
                }
            }
            true
        });

        registry.add_sources(sources)?;
    }

    // Here we place an artificial limitation that all non-registry sources
    // cannot be locked at more than one revision. This means that if a Git
    // repository provides more than one package, they must all be updated in
    // step when any of them are updated.
    //
    // TODO: this seems like a hokey reason to single out the registry as being
    // different.
    let to_avoid_sources: HashSet<_> = to_avoid
        .iter()
        .map(|p| p.source_id())
        .filter(|s| !s.is_registry())
        .collect();

    let keep = |p: &PackageId| !to_avoid_sources.contains(&p.source_id()) && !to_avoid.contains(p);

    let mut resolve = ops::resolve_with_previous(
        &mut registry,
        ws,
        &CliFeatures::new_all(true),
        HasDevUnits::Yes,
        Some(&previous_resolve),
        Some(&keep),
        &[],
        true,
    )?;

    print_lockfile_updates(
        ws,
        &previous_resolve,
        &resolve,
        opts.precise.is_some(),
        &mut registry,
    )?;
    if opts.dry_run {
        opts.gctx
            .shell()
            .warn("not updating lockfile due to dry run")?;
    } else {
        ops::write_pkg_lockfile(ws, &mut resolve)?;
    }
    Ok(())
}

/// Prints lockfile change statuses.
///
/// This would acquire the package-cache lock, as it may update the index to
/// show users latest available versions.
pub fn print_lockfile_changes(
    ws: &Workspace<'_>,
    previous_resolve: Option<&Resolve>,
    resolve: &Resolve,
    registry: &mut PackageRegistry<'_>,
) -> CargoResult<()> {
    let _lock = ws
        .gctx()
        .acquire_package_cache_lock(CacheLockMode::DownloadExclusive)?;
    if let Some(previous_resolve) = previous_resolve {
        print_lockfile_sync(ws, previous_resolve, resolve, registry)
    } else {
        print_lockfile_generation(ws, resolve, registry)
    }
}
pub fn upgrade_manifests(
    ws: &mut Workspace<'_>,
    to_update: &Vec<String>,
) -> CargoResult<UpgradeMap> {
    let gctx = ws.gctx();
    let mut upgrades = HashMap::new();
    let mut upgrade_messages = HashSet::new();

    let to_update = to_update
        .iter()
        .map(|spec| {
            PackageIdSpec::parse(spec)
                .with_context(|| format!("invalid package ID specification: `{spec}`"))
        })
        .collect::<Result<Vec<_>, _>>()?;

    // Updates often require a lot of modifications to the registry, so ensure
    // that we're synchronized against other Cargos.
    let _lock = gctx.acquire_package_cache_lock(CacheLockMode::DownloadExclusive)?;

    let mut registry = ws.package_registry()?;
    registry.lock_patches();

    for member in ws.members_mut().sorted() {
        debug!("upgrading manifest for `{}`", member.name());

        *member.manifest_mut().summary_mut() = member
            .manifest()
            .summary()
            .clone()
            .try_map_dependencies(|d| {
                upgrade_dependency(
                    &gctx,
                    &to_update,
                    &mut registry,
                    &mut upgrades,
                    &mut upgrade_messages,
                    d,
                )
            })?;
    }

    Ok(upgrades)
}

fn upgrade_dependency(
    gctx: &GlobalContext,
    to_update: &Vec<PackageIdSpec>,
    registry: &mut PackageRegistry<'_>,
    upgrades: &mut UpgradeMap,
    upgrade_messages: &mut HashSet<String>,
    dependency: Dependency,
) -> CargoResult<Dependency> {
    let name = dependency.package_name();
    let renamed_to = dependency.name_in_toml();

    if name != renamed_to {
        trace!("skipping dependency renamed from `{name}` to `{renamed_to}`");
        return Ok(dependency);
    }

    if !to_update.is_empty()
        && !to_update.iter().any(|spec| {
            spec.name() == name.as_str()
                && dependency.source_id().is_registry()
                && spec
                    .url()
                    .map_or(true, |url| url == dependency.source_id().url())
                && spec
                    .version()
                    .map_or(true, |v| dependency.version_req().matches(&v))
        })
    {
        trace!("skipping dependency `{name}` not selected for upgrading");
        return Ok(dependency);
    }

    if !dependency.source_id().is_registry() {
        trace!("skipping non-registry dependency: {name}");
        return Ok(dependency);
    }

    let version_req = dependency.version_req();

    let OptVersionReq::Req(current) = version_req else {
        trace!("skipping dependency `{name}` without a simple version requirement: {version_req}");
        return Ok(dependency);
    };

    let [comparator] = &current.comparators[..] else {
        trace!(
            "skipping dependency `{name}` with multiple version comparators: {:?}",
            &current.comparators
        );
        return Ok(dependency);
    };

    if comparator.op != Op::Caret {
        trace!("skipping non-caret dependency `{name}`: {comparator}");
        return Ok(dependency);
    }

    let query =
        crate::core::dependency::Dependency::parse(name, None, dependency.source_id().clone())?;

    let possibilities = {
        loop {
            match registry.query_vec(&query, QueryKind::Exact) {
                std::task::Poll::Ready(res) => {
                    break res?;
                }
                std::task::Poll::Pending => registry.block_until_ready()?,
            }
        }
    };

    let latest = if !possibilities.is_empty() {
        possibilities
            .iter()
            .map(|s| s.as_summary())
            .map(|s| s.version())
            .filter(|v| !v.is_prerelease())
            .max()
    } else {
        None
    };

    let Some(latest) = latest else {
        trace!("skipping dependency `{name}` without any published versions");
        return Ok(dependency);
    };

    if current.matches(&latest) {
        trace!("skipping dependency `{name}` without a breaking update available");
        return Ok(dependency);
    }

    let Some((new_req_string, _)) = upgrade_requirement(&current.to_string(), latest)? else {
        trace!("skipping dependency `{name}` because the version requirement didn't change");
        return Ok(dependency);
    };

    let upgrade_message = format!("{name} {current} -> {new_req_string}");
    trace!(upgrade_message);

    if upgrade_messages.insert(upgrade_message.clone()) {
        gctx.shell()
            .status_with_color("Upgrading", &upgrade_message, &style::GOOD)?;
    }

    upgrades.insert((name.to_string(), dependency.source_id()), latest.clone());

    let req = OptVersionReq::Req(VersionReq::parse(&latest.to_string())?);
    let mut dep = dependency.clone();
    dep.set_version_req(req);
    Ok(dep)
}

/// Update manifests with upgraded versions, and write to disk. Based on
/// cargo-edit. Returns true if any file has changed.
///
/// Some of the checks here are duplicating checks already done in
/// `upgrade_manifests/upgrade_dependency`. Why? Let's say `upgrade_dependency` has
/// found that dependency foo was eligible for an upgrade. But foo can occur in
/// multiple manifest files, and even multiple times in the same manifest file,
/// and may be pinned, renamed, etc. in some of the instances. So we still need
/// to check here which dependencies to actually modify. So why not drop the
/// upgrade map and redo all checks here? Because then we'd have to query the
/// registries again to find the latest versions.
pub fn write_manifest_upgrades(
    ws: &Workspace<'_>,
    upgrades: &UpgradeMap,
    dry_run: bool,
) -> CargoResult<bool> {
    if upgrades.is_empty() {
        return Ok(false);
    }

    let mut any_file_has_changed = false;

    let items = std::iter::once((ws.root_manifest(), ws.unstable_features()))
        .chain(ws.members().map(|member| {
            (
                member.manifest_path(),
                member.manifest().unstable_features(),
            )
        }))
        .collect::<Vec<_>>();

    for (manifest_path, unstable_features) in items {
        trace!("updating TOML manifest at `{manifest_path:?}` with upgraded dependencies");

        let crate_root = manifest_path
            .parent()
            .expect("manifest path is absolute")
            .to_owned();

        let mut local_manifest = LocalManifest::try_new(&manifest_path)?;
        let mut manifest_has_changed = false;

        for dep_table in local_manifest.get_dependency_tables_mut() {
            for (mut dep_key, dep_item) in dep_table.iter_mut() {
                let dep_key_str = dep_key.get();
                let dependency = crate::util::toml_mut::dependency::Dependency::from_toml(
                    ws.gctx(),
                    ws.root(),
                    &manifest_path,
                    unstable_features,
                    dep_key_str,
                    dep_item,
                )?;
                let name = &dependency.name;

                if let Some(renamed_to) = dependency.rename {
                    trace!("skipping dependency renamed from `{name}` to `{renamed_to}`");
                    continue;
                }

                let Some(current) = dependency.version() else {
                    trace!("skipping dependency without a version: {name}");
                    continue;
                };

                let (MaybeWorkspace::Other(source_id), Some(Source::Registry(source))) =
                    (dependency.source_id(ws.gctx())?, dependency.source())
                else {
                    trace!("skipping non-registry dependency: {name}");
                    continue;
                };

                let Some(latest) = upgrades.get(&(name.to_owned(), source_id)) else {
                    trace!("skipping dependency without an upgrade: {name}");
                    continue;
                };

                let Some((new_req_string, new_req)) = upgrade_requirement(current, latest)? else {
                    trace!(
                        "skipping dependency `{name}` because the version requirement didn't change"
                    );
                    continue;
                };

                let [comparator] = &new_req.comparators[..] else {
                    trace!(
                        "skipping dependency `{}` with multiple version comparators: {:?}",
                        name, new_req.comparators
                    );
                    continue;
                };

                if comparator.op != Op::Caret {
                    trace!("skipping non-caret dependency `{}`: {}", name, comparator);
                    continue;
                }

                let mut dep = dependency.clone();
                let mut source = source.clone();
                source.version = new_req_string;
                dep.source = Some(Source::Registry(source));

                trace!("upgrading dependency {name}");
                dep.update_toml(
                    ws.gctx(),
                    ws.root(),
                    &crate_root,
                    unstable_features,
                    &mut dep_key,
                    dep_item,
                )?;
                manifest_has_changed = true;
                any_file_has_changed = true;
            }
        }

        if manifest_has_changed && !dry_run {
            debug!("writing upgraded manifest to {}", manifest_path.display());
            local_manifest.write()?;
        }
    }

    Ok(any_file_has_changed)
}

fn print_lockfile_generation(
    ws: &Workspace<'_>,
    resolve: &Resolve,
    registry: &mut PackageRegistry<'_>,
) -> CargoResult<()> {
    let mut changes = PackageChange::new(ws, resolve);
    let num_pkgs: usize = changes
        .values()
        .filter(|change| change.kind.is_new() && !change.is_member.unwrap_or(false))
        .count();
    if num_pkgs == 0 {
        // nothing worth reporting
        return Ok(());
    }
    annotate_required_rust_version(ws, resolve, &mut changes);

    status_locking(ws, num_pkgs)?;
    for change in changes.values() {
        if change.is_member.unwrap_or(false) {
            continue;
        };
        match change.kind {
            PackageChangeKind::Added => {
                let possibilities = if let Some(query) = change.alternatives_query() {
                    loop {
                        match registry.query_vec(&query, QueryKind::Exact) {
                            std::task::Poll::Ready(res) => {
                                break res?;
                            }
                            std::task::Poll::Pending => registry.block_until_ready()?,
                        }
                    }
                } else {
                    vec![]
                };

                let required_rust_version = report_required_rust_version(resolve, change);
                let latest = report_latest(&possibilities, change);
                let note = required_rust_version.or(latest);

                if let Some(note) = note {
                    ws.gctx().shell().status_with_color(
                        change.kind.status(),
                        format!("{change}{note}"),
                        &change.kind.style(),
                    )?;
                }
            }
            PackageChangeKind::Upgraded
            | PackageChangeKind::Downgraded
            | PackageChangeKind::Removed
            | PackageChangeKind::Unchanged => {
                unreachable!("without a previous resolve, everything should be added")
            }
        }
    }

    Ok(())
}

fn print_lockfile_sync(
    ws: &Workspace<'_>,
    previous_resolve: &Resolve,
    resolve: &Resolve,
    registry: &mut PackageRegistry<'_>,
) -> CargoResult<()> {
    let mut changes = PackageChange::diff(ws, previous_resolve, resolve);
    let num_pkgs: usize = changes
        .values()
        .filter(|change| change.kind.is_new() && !change.is_member.unwrap_or(false))
        .count();
    if num_pkgs == 0 {
        // nothing worth reporting
        return Ok(());
    }
    annotate_required_rust_version(ws, resolve, &mut changes);

    status_locking(ws, num_pkgs)?;
    for change in changes.values() {
        if change.is_member.unwrap_or(false) {
            continue;
        };
        match change.kind {
            PackageChangeKind::Added
            | PackageChangeKind::Upgraded
            | PackageChangeKind::Downgraded => {
                let possibilities = if let Some(query) = change.alternatives_query() {
                    loop {
                        match registry.query_vec(&query, QueryKind::Exact) {
                            std::task::Poll::Ready(res) => {
                                break res?;
                            }
                            std::task::Poll::Pending => registry.block_until_ready()?,
                        }
                    }
                } else {
                    vec![]
                };

                let required_rust_version = report_required_rust_version(resolve, change);
                let latest = report_latest(&possibilities, change);
                let note = required_rust_version.or(latest).unwrap_or_default();

                ws.gctx().shell().status_with_color(
                    change.kind.status(),
                    format!("{change}{note}"),
                    &change.kind.style(),
                )?;
            }
            PackageChangeKind::Removed | PackageChangeKind::Unchanged => {}
        }
    }

    Ok(())
}

fn print_lockfile_updates(
    ws: &Workspace<'_>,
    previous_resolve: &Resolve,
    resolve: &Resolve,
    precise: bool,
    registry: &mut PackageRegistry<'_>,
) -> CargoResult<()> {
    let mut changes = PackageChange::diff(ws, previous_resolve, resolve);
    let num_pkgs: usize = changes
        .values()
        .filter(|change| change.kind.is_new())
        .count();
    annotate_required_rust_version(ws, resolve, &mut changes);

    if !precise {
        status_locking(ws, num_pkgs)?;
    }
    let mut unchanged_behind = 0;
    for change in changes.values() {
        let possibilities = if let Some(query) = change.alternatives_query() {
            loop {
                match registry.query_vec(&query, QueryKind::Exact) {
                    std::task::Poll::Ready(res) => {
                        break res?;
                    }
                    std::task::Poll::Pending => registry.block_until_ready()?,
                }
            }
        } else {
            vec![]
        };

        match change.kind {
            PackageChangeKind::Added
            | PackageChangeKind::Upgraded
            | PackageChangeKind::Downgraded => {
                let required_rust_version = report_required_rust_version(resolve, change);
                let latest = report_latest(&possibilities, change);
                let note = required_rust_version.or(latest).unwrap_or_default();

                ws.gctx().shell().status_with_color(
                    change.kind.status(),
                    format!("{change}{note}"),
                    &change.kind.style(),
                )?;
            }
            PackageChangeKind::Removed => {
                ws.gctx().shell().status_with_color(
                    change.kind.status(),
                    format!("{change}"),
                    &change.kind.style(),
                )?;
            }
            PackageChangeKind::Unchanged => {
                let required_rust_version = report_required_rust_version(resolve, change);
                let latest = report_latest(&possibilities, change);
                let note = required_rust_version.as_deref().or(latest.as_deref());

                if let Some(note) = note {
                    if latest.is_some() {
                        unchanged_behind += 1;
                    }
                    if ws.gctx().shell().verbosity() == Verbosity::Verbose {
                        ws.gctx().shell().status_with_color(
                            change.kind.status(),
                            format!("{change}{note}"),
                            &change.kind.style(),
                        )?;
                    }
                }
            }
        }
    }

    if ws.gctx().shell().verbosity() == Verbosity::Verbose {
        ws.gctx()
            .shell()
            .note("to see how you depend on a package, run `cargo tree --invert <dep>@<ver>`")?;
    } else {
        if 0 < unchanged_behind {
            ws.gctx().shell().note(format!(
                "pass `--verbose` to see {unchanged_behind} unchanged dependencies behind latest"
            ))?;
        }
    }

    Ok(())
}

fn status_locking(ws: &Workspace<'_>, num_pkgs: usize) -> CargoResult<()> {
    use std::fmt::Write as _;

    let plural = if num_pkgs == 1 { "" } else { "s" };

    let mut cfg = String::new();
    // Don't have a good way to describe `direct_minimal_versions` atm
    if !ws.gctx().cli_unstable().direct_minimal_versions {
        write!(&mut cfg, " to")?;
        if ws.gctx().cli_unstable().minimal_versions {
            write!(&mut cfg, " earliest")?;
        } else {
            write!(&mut cfg, " latest")?;
        }

        if let Some(rust_version) = required_rust_version(ws) {
            write!(&mut cfg, " Rust {rust_version}")?;
        }
        write!(&mut cfg, " compatible version{plural}")?;
    }

    ws.gctx()
        .shell()
        .status("Locking", format!("{num_pkgs} package{plural}{cfg}"))?;
    Ok(())
}

fn required_rust_version(ws: &Workspace<'_>) -> Option<PartialVersion> {
    if !ws.resolve_honors_rust_version() {
        return None;
    }

    if let Some(ver) = ws.lowest_rust_version() {
        Some(ver.clone().into_partial())
    } else {
        let rustc = ws.gctx().load_global_rustc(Some(ws)).ok()?;
        let rustc_version = rustc.version.clone().into();
        Some(rustc_version)
    }
}

fn report_required_rust_version(resolve: &Resolve, change: &PackageChange) -> Option<String> {
    if change.package_id.source_id().is_path() {
        return None;
    }
    let summary = resolve.summary(change.package_id);
    let package_rust_version = summary.rust_version()?;
    let required_rust_version = change.required_rust_version.as_ref()?;
    if package_rust_version.is_compatible_with(required_rust_version) {
        return None;
    }

    let error = style::ERROR;
    Some(format!(
        " {error}(requires Rust {package_rust_version}){error:#}"
    ))
}

fn report_latest(possibilities: &[IndexSummary], change: &PackageChange) -> Option<String> {
    let package_id = change.package_id;
    if !package_id.source_id().is_registry() {
        return None;
    }

    let version_req = package_id.version().to_caret_req();
    let required_rust_version = change.required_rust_version.as_ref();

    let compat_ver_compat_msrv_summary = possibilities
        .iter()
        .map(|s| s.as_summary())
        .filter(|s| {
            if let (Some(summary_rust_version), Some(required_rust_version)) =
                (s.rust_version(), required_rust_version)
            {
                summary_rust_version.is_compatible_with(required_rust_version)
            } else {
                true
            }
        })
        .filter(|s| package_id.version() != s.version() && version_req.matches(s.version()))
        .max_by_key(|s| s.version());
    if let Some(summary) = compat_ver_compat_msrv_summary {
        let warn = style::WARN;
        let version = summary.version();
        let report = format!(" {warn}(available: v{version}){warn:#}");
        return Some(report);
    }

    if !change.is_transitive.unwrap_or(true) {
        let incompat_ver_compat_msrv_summary = possibilities
            .iter()
            .map(|s| s.as_summary())
            .filter(|s| {
                if let (Some(summary_rust_version), Some(required_rust_version)) =
                    (s.rust_version(), required_rust_version)
                {
                    summary_rust_version.is_compatible_with(required_rust_version)
                } else {
                    true
                }
            })
            .filter(|s| is_latest(s.version(), package_id.version()))
            .max_by_key(|s| s.version());
        if let Some(summary) = incompat_ver_compat_msrv_summary {
            let warn = style::WARN;
            let version = summary.version();
            let report = format!(" {warn}(available: v{version}){warn:#}");
            return Some(report);
        }
    }

    let compat_ver_summary = possibilities
        .iter()
        .map(|s| s.as_summary())
        .filter(|s| package_id.version() != s.version() && version_req.matches(s.version()))
        .max_by_key(|s| s.version());
    if let Some(summary) = compat_ver_summary {
        let msrv_note = summary
            .rust_version()
            .map(|rv| format!(", requires Rust {rv}"))
            .unwrap_or_default();
        let warn = style::NOP;
        let version = summary.version();
        let report = format!(" {warn}(available: v{version}{msrv_note}){warn:#}");
        return Some(report);
    }

    if !change.is_transitive.unwrap_or(true) {
        let incompat_ver_summary = possibilities
            .iter()
            .map(|s| s.as_summary())
            .filter(|s| is_latest(s.version(), package_id.version()))
            .max_by_key(|s| s.version());
        if let Some(summary) = incompat_ver_summary {
            let msrv_note = summary
                .rust_version()
                .map(|rv| format!(", requires Rust {rv}"))
                .unwrap_or_default();
            let warn = style::NOP;
            let version = summary.version();
            let report = format!(" {warn}(available: v{version}{msrv_note}){warn:#}");
            return Some(report);
        }
    }

    None
}

fn is_latest(candidate: &semver::Version, current: &semver::Version) -> bool {
    current < candidate
                // Only match pre-release if major.minor.patch are the same
                && (candidate.pre.is_empty()
                    || (candidate.major == current.major
                        && candidate.minor == current.minor
                        && candidate.patch == current.patch))
}

fn fill_with_deps<'a>(
    resolve: &'a Resolve,
    dep: PackageId,
    set: &mut HashSet<PackageId>,
    visited: &mut HashSet<PackageId>,
) {
    if !visited.insert(dep) {
        return;
    }
    set.insert(dep);
    for (dep, _) in resolve.deps_not_replaced(dep) {
        fill_with_deps(resolve, dep, set, visited);
    }
}

#[derive(Clone, Debug)]
struct PackageChange {
    package_id: PackageId,
    previous_id: Option<PackageId>,
    kind: PackageChangeKind,
    is_member: Option<bool>,
    is_transitive: Option<bool>,
    required_rust_version: Option<PartialVersion>,
}

impl PackageChange {
    pub fn new(ws: &Workspace<'_>, resolve: &Resolve) -> IndexMap<PackageId, Self> {
        let diff = PackageDiff::new(resolve);
        Self::with_diff(diff, ws, resolve)
    }

    pub fn diff(
        ws: &Workspace<'_>,
        previous_resolve: &Resolve,
        resolve: &Resolve,
    ) -> IndexMap<PackageId, Self> {
        let diff = PackageDiff::diff(previous_resolve, resolve);
        Self::with_diff(diff, ws, resolve)
    }

    fn with_diff(
        diff: impl Iterator<Item = PackageDiff>,
        ws: &Workspace<'_>,
        resolve: &Resolve,
    ) -> IndexMap<PackageId, Self> {
        let member_ids: HashSet<_> = ws.members().map(|p| p.package_id()).collect();

        let mut changes = IndexMap::new();
        for diff in diff {
            if let Some((previous_id, package_id)) = diff.change() {
                // If versions differ only in build metadata, we call it an "update"
                // regardless of whether the build metadata has gone up or down.
                // This metadata is often stuff like git commit hashes, which are
                // not meaningfully ordered.
                let kind = if previous_id.version().cmp_precedence(package_id.version())
                    == Ordering::Greater
                {
                    PackageChangeKind::Downgraded
                } else {
                    PackageChangeKind::Upgraded
                };
                let is_member = Some(member_ids.contains(&package_id));
                let is_transitive = Some(true);
                let change = Self {
                    package_id,
                    previous_id: Some(previous_id),
                    kind,
                    is_member,
                    is_transitive,
                    required_rust_version: None,
                };
                changes.insert(change.package_id, change);
            } else {
                for package_id in diff.removed {
                    let kind = PackageChangeKind::Removed;
                    let is_member = None;
                    let is_transitive = None;
                    let change = Self {
                        package_id,
                        previous_id: None,
                        kind,
                        is_member,
                        is_transitive,
                        required_rust_version: None,
                    };
                    changes.insert(change.package_id, change);
                }
                for package_id in diff.added {
                    let kind = PackageChangeKind::Added;
                    let is_member = Some(member_ids.contains(&package_id));
                    let is_transitive = Some(true);
                    let change = Self {
                        package_id,
                        previous_id: None,
                        kind,
                        is_member,
                        is_transitive,
                        required_rust_version: None,
                    };
                    changes.insert(change.package_id, change);
                }
            }
            for package_id in diff.unchanged {
                let kind = PackageChangeKind::Unchanged;
                let is_member = Some(member_ids.contains(&package_id));
                let is_transitive = Some(true);
                let change = Self {
                    package_id,
                    previous_id: None,
                    kind,
                    is_member,
                    is_transitive,
                    required_rust_version: None,
                };
                changes.insert(change.package_id, change);
            }
        }

        for member_id in &member_ids {
            let Some(change) = changes.get_mut(member_id) else {
                continue;
            };
            change.is_transitive = Some(false);
            for (direct_dep_id, _) in resolve.deps(*member_id) {
                let Some(change) = changes.get_mut(&direct_dep_id) else {
                    continue;
                };
                change.is_transitive = Some(false);
            }
        }

        changes
    }

    /// For querying [`PackageRegistry`] for alternative versions to report to the user
    fn alternatives_query(&self) -> Option<crate::core::dependency::Dependency> {
        if !self.package_id.source_id().is_registry() {
            return None;
        }

        let query = crate::core::dependency::Dependency::parse(
            self.package_id.name(),
            None,
            self.package_id.source_id(),
        )
        .expect("already a valid dependency");
        Some(query)
    }
}

impl std::fmt::Display for PackageChange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let package_id = self.package_id;
        if let Some(previous_id) = self.previous_id {
            if package_id.source_id().is_git() {
                write!(
                    f,
                    "{previous_id} -> #{}",
                    &package_id.source_id().precise_git_fragment().unwrap()[..8],
                )
            } else {
                write!(f, "{previous_id} -> v{}", package_id.version())
            }
        } else {
            write!(f, "{package_id}")
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum PackageChangeKind {
    Added,
    Removed,
    Upgraded,
    Downgraded,
    Unchanged,
}

impl PackageChangeKind {
    pub fn is_new(&self) -> bool {
        match self {
            Self::Added | Self::Upgraded | Self::Downgraded => true,
            Self::Removed | Self::Unchanged => false,
        }
    }

    pub fn status(&self) -> &'static str {
        match self {
            Self::Added => "Adding",
            Self::Removed => "Removing",
            Self::Upgraded => "Updating",
            Self::Downgraded => "Downgrading",
            Self::Unchanged => "Unchanged",
        }
    }

    pub fn style(&self) -> anstyle::Style {
        match self {
            Self::Added => style::UPDATE_ADDED,
            Self::Removed => style::UPDATE_REMOVED,
            Self::Upgraded => style::UPDATE_UPGRADED,
            Self::Downgraded => style::UPDATE_DOWNGRADED,
            Self::Unchanged => style::UPDATE_UNCHANGED,
        }
    }
}

/// All resolved versions of a package name within a [`SourceId`]
#[derive(Default, Clone, Debug)]
pub struct PackageDiff {
    removed: Vec<PackageId>,
    added: Vec<PackageId>,
    unchanged: Vec<PackageId>,
}

impl PackageDiff {
    pub fn new(resolve: &Resolve) -> impl Iterator<Item = Self> {
        let mut changes = BTreeMap::new();
        let empty = Self::default();
        for dep in resolve.iter() {
            changes
                .entry(Self::key(dep))
                .or_insert_with(|| empty.clone())
                .added
                .push(dep);
        }

        changes.into_iter().map(|(_, v)| v)
    }

    pub fn diff(previous_resolve: &Resolve, resolve: &Resolve) -> impl Iterator<Item = Self> {
        fn vec_subset(a: &[PackageId], b: &[PackageId]) -> Vec<PackageId> {
            a.iter().filter(|a| !contains_id(b, a)).cloned().collect()
        }

        fn vec_intersection(a: &[PackageId], b: &[PackageId]) -> Vec<PackageId> {
            a.iter().filter(|a| contains_id(b, a)).cloned().collect()
        }

        // Check if a PackageId is present `b` from `a`.
        //
        // Note that this is somewhat more complicated because the equality for source IDs does not
        // take precise versions into account (e.g., git shas), but we want to take that into
        // account here.
        fn contains_id(haystack: &[PackageId], needle: &PackageId) -> bool {
            let Ok(i) = haystack.binary_search(needle) else {
                return false;
            };

            // If we've found `a` in `b`, then we iterate over all instances
            // (we know `b` is sorted) and see if they all have different
            // precise versions. If so, then `a` isn't actually in `b` so
            // we'll let it through.
            //
            // Note that we only check this for non-registry sources,
            // however, as registries contain enough version information in
            // the package ID to disambiguate.
            if needle.source_id().is_registry() {
                return true;
            }
            haystack[i..]
                .iter()
                .take_while(|b| &needle == b)
                .any(|b| needle.source_id().has_same_precise_as(b.source_id()))
        }

        // Map `(package name, package source)` to `(removed versions, added versions)`.
        let mut changes = BTreeMap::new();
        let empty = Self::default();
        for dep in previous_resolve.iter() {
            changes
                .entry(Self::key(dep))
                .or_insert_with(|| empty.clone())
                .removed
                .push(dep);
        }
        for dep in resolve.iter() {
            changes
                .entry(Self::key(dep))
                .or_insert_with(|| empty.clone())
                .added
                .push(dep);
        }

        for v in changes.values_mut() {
            let Self {
                removed: ref mut old,
                added: ref mut new,
                unchanged: ref mut other,
            } = *v;
            old.sort();
            new.sort();
            let removed = vec_subset(old, new);
            let added = vec_subset(new, old);
            let unchanged = vec_intersection(new, old);
            *old = removed;
            *new = added;
            *other = unchanged;
        }
        debug!("{:#?}", changes);

        changes.into_iter().map(|(_, v)| v)
    }

    fn key(dep: PackageId) -> (&'static str, SourceId) {
        (dep.name().as_str(), dep.source_id())
    }

    /// Guess if a package upgraded/downgraded
    ///
    /// All `PackageDiff` knows is that entries were added/removed within [`Resolve`].
    /// A package could be added or removed because of dependencies from other packages
    /// which makes it hard to definitively say "X was upgrade to N".
    pub fn change(&self) -> Option<(PackageId, PackageId)> {
        if self.removed.len() == 1 && self.added.len() == 1 {
            Some((self.removed[0], self.added[0]))
        } else {
            None
        }
    }
}

fn annotate_required_rust_version(
    ws: &Workspace<'_>,
    resolve: &Resolve,
    changes: &mut IndexMap<PackageId, PackageChange>,
) {
    let rustc = ws.gctx().load_global_rustc(Some(ws)).ok();
    let rustc_version: Option<PartialVersion> =
        rustc.as_ref().map(|rustc| rustc.version.clone().into());

    if ws.resolve_honors_rust_version() {
        let mut queue: std::collections::VecDeque<_> = ws
            .members()
            .map(|p| {
                (
                    p.rust_version()
                        .map(|r| r.clone().into_partial())
                        .or_else(|| rustc_version.clone()),
                    p.package_id(),
                )
            })
            .collect();
        while let Some((required_rust_version, current_id)) = queue.pop_front() {
            let Some(required_rust_version) = required_rust_version else {
                continue;
            };
            if let Some(change) = changes.get_mut(&current_id) {
                if let Some(existing) = change.required_rust_version.as_ref() {
                    if *existing <= required_rust_version {
                        // Stop early; we already walked down this path with a better match
                        continue;
                    }
                }
                change.required_rust_version = Some(required_rust_version.clone());
            }
            queue.extend(
                resolve
                    .deps(current_id)
                    .map(|(dep, _)| (Some(required_rust_version.clone()), dep)),
            );
        }
    } else {
        for change in changes.values_mut() {
            change.required_rust_version = rustc_version.clone();
        }
    }
}
