use crate::core::registry::PackageRegistry;
use crate::core::resolver::features::{CliFeatures, HasDevUnits};
use crate::core::shell::Verbosity;
use crate::core::Registry as _;
use crate::core::{PackageId, PackageIdSpec, PackageIdSpecQuery};
use crate::core::{Resolve, SourceId, Workspace};
use crate::ops;
use crate::sources::source::QueryKind;
use crate::util::cache_lock::CacheLockMode;
use crate::util::context::GlobalContext;
use crate::util::style;
use crate::util::CargoResult;
use anstyle::Style;
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashSet};
use tracing::debug;

pub struct UpdateOptions<'a> {
    pub gctx: &'a GlobalContext,
    pub to_update: Vec<String>,
    pub precise: Option<&'a str>,
    pub recursive: bool,
    pub dry_run: bool,
    pub workspace: bool,
}

pub fn generate_lockfile(ws: &Workspace<'_>) -> CargoResult<()> {
    let mut registry = PackageRegistry::new(ws.gctx())?;
    let mut resolve = ops::resolve_with_previous(
        &mut registry,
        ws,
        &CliFeatures::new_all(true),
        HasDevUnits::Yes,
        None,
        None,
        &[],
        true,
    )?;
    ops::write_pkg_lockfile(ws, &mut resolve)?;
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
                    let mut registry = PackageRegistry::new(opts.gctx)?;
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
    let mut registry = PackageRegistry::new(opts.gctx)?;
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
        to_avoid = to_avoid
            .into_iter()
            .filter(|id| {
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
            })
            .collect();

        registry.add_sources(sources)?;
    }

    let mut resolve = ops::resolve_with_previous(
        &mut registry,
        ws,
        &CliFeatures::new_all(true),
        HasDevUnits::Yes,
        Some(&previous_resolve),
        Some(&to_avoid),
        &[],
        true,
    )?;

    print_lockfile_update(opts.gctx, &previous_resolve, &resolve, &mut registry)?;
    if opts.dry_run {
        opts.gctx
            .shell()
            .warn("not updating lockfile due to dry run")?;
    } else {
        ops::write_pkg_lockfile(ws, &mut resolve)?;
    }
    Ok(())
}

fn print_lockfile_update(
    gctx: &GlobalContext,
    previous_resolve: &Resolve,
    resolve: &Resolve,
    registry: &mut PackageRegistry<'_>,
) -> CargoResult<()> {
    // Summarize what is changing for the user.
    let print_change = |status: &str, msg: String, color: &Style| {
        gctx.shell().status_with_color(status, msg, color)
    };
    let mut unchanged_behind = 0;
    for diff in PackageDiff::diff(&previous_resolve, &resolve) {
        fn format_latest(version: semver::Version) -> String {
            let warn = style::WARN;
            format!(" {warn}(latest: v{version}){warn:#}")
        }
        fn is_latest(candidate: &semver::Version, current: &semver::Version) -> bool {
            current < candidate
                // Only match pre-release if major.minor.patch are the same
                && (candidate.pre.is_empty()
                    || (candidate.major == current.major
                        && candidate.minor == current.minor
                        && candidate.patch == current.patch))
        }
        let possibilities = if let Some(query) = diff.alternatives_query() {
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

        if let Some((removed, added)) = diff.change() {
            let latest = if !possibilities.is_empty() {
                possibilities
                    .iter()
                    .map(|s| s.as_summary())
                    .filter(|s| is_latest(s.version(), added.version()))
                    .map(|s| s.version().clone())
                    .max()
                    .map(format_latest)
            } else {
                None
            }
            .unwrap_or_default();

            let msg = if removed.source_id().is_git() {
                format!(
                    "{removed} -> #{}",
                    &added.source_id().precise_git_fragment().unwrap()[..8],
                )
            } else {
                format!("{removed} -> v{}{latest}", added.version())
            };

            // If versions differ only in build metadata, we call it an "update"
            // regardless of whether the build metadata has gone up or down.
            // This metadata is often stuff like git commit hashes, which are
            // not meaningfully ordered.
            if removed.version().cmp_precedence(added.version()) == Ordering::Greater {
                print_change("Downgrading", msg, &style::WARN)?;
            } else {
                print_change("Updating", msg, &style::GOOD)?;
            }
        } else {
            for package in diff.removed.iter() {
                print_change("Removing", format!("{package}"), &style::ERROR)?;
            }
            for package in diff.added.iter() {
                let latest = if !possibilities.is_empty() {
                    possibilities
                        .iter()
                        .map(|s| s.as_summary())
                        .filter(|s| is_latest(s.version(), package.version()))
                        .map(|s| s.version().clone())
                        .max()
                        .map(format_latest)
                } else {
                    None
                }
                .unwrap_or_default();

                print_change("Adding", format!("{package}{latest}"), &style::NOTE)?;
            }
        }
        for package in &diff.unchanged {
            let latest = if !possibilities.is_empty() {
                possibilities
                    .iter()
                    .map(|s| s.as_summary())
                    .filter(|s| is_latest(s.version(), package.version()))
                    .map(|s| s.version().clone())
                    .max()
                    .map(format_latest)
            } else {
                None
            };

            if let Some(latest) = latest {
                unchanged_behind += 1;
                if gctx.shell().verbosity() == Verbosity::Verbose {
                    gctx.shell().status_with_color(
                        "Unchanged",
                        format!("{package}{latest}"),
                        &anstyle::Style::new().bold(),
                    )?;
                }
            }
        }
    }
    if gctx.shell().verbosity() == Verbosity::Verbose {
        gctx.shell().note(
            "to see how you depend on a package, run `cargo tree --invert --package <dep>@<ver>`",
        )?;
    } else {
        if 0 < unchanged_behind {
            gctx.shell().note(format!(
                "pass `--verbose` to see {unchanged_behind} unchanged dependencies behind latest"
            ))?;
        }
    }

    Ok(())
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

/// All resolved versions of a package name within a [`SourceId`]
#[derive(Default, Clone, Debug)]
pub struct PackageDiff {
    removed: Vec<PackageId>,
    added: Vec<PackageId>,
    unchanged: Vec<PackageId>,
}

impl PackageDiff {
    pub fn diff(previous_resolve: &Resolve, resolve: &Resolve) -> Vec<Self> {
        fn key(dep: PackageId) -> (&'static str, SourceId) {
            (dep.name().as_str(), dep.source_id())
        }

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
                .entry(key(dep))
                .or_insert_with(|| empty.clone())
                .removed
                .push(dep);
        }
        for dep in resolve.iter() {
            changes
                .entry(key(dep))
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

        changes.into_iter().map(|(_, v)| v).collect()
    }

    /// Guess if a package upgraded/downgraded
    ///
    /// All `PackageDiff` knows is that entries were added/removed within [`Resolve`].
    /// A package could be added or removed because of dependencies from other packages
    /// which makes it hard to definitively say "X was upgrade to N".
    pub fn change(&self) -> Option<(&PackageId, &PackageId)> {
        if self.removed.len() == 1 && self.added.len() == 1 {
            Some((&self.removed[0], &self.added[0]))
        } else {
            None
        }
    }

    /// For querying [`PackageRegistry`] for alternative versions to report to the user
    pub fn alternatives_query(&self) -> Option<crate::core::dependency::Dependency> {
        let package_id = [
            self.added.iter(),
            self.unchanged.iter(),
            self.removed.iter(),
        ]
        .into_iter()
        .flatten()
        .next()
        // Limit to registry as that is the only source with meaningful alternative versions
        .filter(|s| s.source_id().is_registry())?;
        let query = crate::core::dependency::Dependency::parse(
            package_id.name(),
            None,
            package_id.source_id(),
        )
        .expect("already a valid dependency");
        Some(query)
    }
}
