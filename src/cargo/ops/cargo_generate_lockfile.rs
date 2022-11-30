use crate::core::registry::PackageRegistry;
use crate::core::resolver::features::{CliFeatures, HasDevUnits};
use crate::core::{PackageId, PackageIdSpec};
use crate::core::{Resolve, SourceId, Workspace};
use crate::ops;
use crate::util::config::Config;
use crate::util::CargoResult;
use anyhow::Context;
use log::debug;
use std::collections::{BTreeMap, HashSet};
use termcolor::Color::{self, Cyan, Green, Red};

pub struct UpdateOptions<'a> {
    pub config: &'a Config,
    pub to_update: Vec<String>,
    pub precise: Option<&'a str>,
    pub aggressive: bool,
    pub dry_run: bool,
    pub workspace: bool,
}

pub fn generate_lockfile(ws: &Workspace<'_>) -> CargoResult<()> {
    let mut registry = PackageRegistry::new(ws.config())?;
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
    if opts.aggressive && opts.precise.is_some() {
        anyhow::bail!("cannot specify both aggressive and precise simultaneously")
    }

    if ws.members().count() == 0 {
        anyhow::bail!("you can't generate a lockfile for an empty workspace.")
    }

    // Updates often require a lot of modifications to the registry, so ensure
    // that we're synchronized against other Cargos.
    let _lock = ws.config().acquire_package_cache_lock()?;

    let previous_resolve = match ops::load_pkg_lockfile(ws)? {
        Some(resolve) => resolve,
        None => {
            match opts.precise {
                None => return generate_lockfile(ws),

                // Precise option specified, so calculate a previous_resolve required
                // by precise package update later.
                Some(_) => {
                    let mut registry = PackageRegistry::new(opts.config)?;
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
    let mut registry = PackageRegistry::new(opts.config)?;
    let mut to_avoid = HashSet::new();

    if opts.to_update.is_empty() {
        if !opts.workspace {
            to_avoid.extend(previous_resolve.iter());
            to_avoid.extend(previous_resolve.unused_patches());
        }
    } else {
        let mut sources = Vec::new();
        for name in opts.to_update.iter() {
            let dep = previous_resolve.query(name)?;
            if opts.aggressive {
                fill_with_deps(&previous_resolve, dep, &mut to_avoid, &mut HashSet::new());
            } else {
                to_avoid.insert(dep);
                sources.push(match opts.precise {
                    Some(precise) => {
                        // TODO: see comment in `resolve.rs` as well, but this
                        //       seems like a pretty hokey reason to single out
                        //       the registry as well.
                        let precise = if dep.source_id().is_registry() {
                            semver::Version::parse(precise).with_context(|| {
                                format!("invalid version format for precise version `{}`", precise)
                            })?;
                            format!("{}={}->{}", dep.name(), dep.version(), precise)
                        } else {
                            precise.to_string()
                        };
                        dep.source_id().with_precise(Some(precise))
                    }
                    None => dep.source_id().with_precise(None),
                });
            }
            if let Ok(unused_id) =
                PackageIdSpec::query_str(name, previous_resolve.unused_patches().iter().cloned())
            {
                to_avoid.insert(unused_id);
            }
        }

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

    // Summarize what is changing for the user.
    let print_change = |status: &str, msg: String, color: Color| {
        opts.config.shell().status_with_color(status, msg, color)
    };
    for (removed, added) in compare_dependency_graphs(&previous_resolve, &resolve) {
        if removed.len() == 1 && added.len() == 1 {
            let msg = if removed[0].source_id().is_git() {
                format!(
                    "{} -> #{}",
                    removed[0],
                    &added[0].source_id().precise().unwrap()[..8]
                )
            } else {
                format!("{} -> v{}", removed[0], added[0].version())
            };
            print_change("Updating", msg, Green)?;
        } else {
            for package in removed.iter() {
                print_change("Removing", format!("{}", package), Red)?;
            }
            for package in added.iter() {
                print_change("Adding", format!("{}", package), Cyan)?;
            }
        }
    }
    if opts.dry_run {
        opts.config
            .shell()
            .warn("not updating lockfile due to dry run")?;
    } else {
        ops::write_pkg_lockfile(ws, &mut resolve)?;
    }
    return Ok(());

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

    fn compare_dependency_graphs(
        previous_resolve: &Resolve,
        resolve: &Resolve,
    ) -> Vec<(Vec<PackageId>, Vec<PackageId>)> {
        fn key(dep: PackageId) -> (&'static str, SourceId) {
            (dep.name().as_str(), dep.source_id())
        }

        // Removes all package IDs in `b` from `a`. Note that this is somewhat
        // more complicated because the equality for source IDs does not take
        // precise versions into account (e.g., git shas), but we want to take
        // that into account here.
        fn vec_subtract(a: &[PackageId], b: &[PackageId]) -> Vec<PackageId> {
            a.iter()
                .filter(|a| {
                    // If this package ID is not found in `b`, then it's definitely
                    // in the subtracted set.
                    let i = match b.binary_search(a) {
                        Ok(i) => i,
                        Err(..) => return true,
                    };

                    // If we've found `a` in `b`, then we iterate over all instances
                    // (we know `b` is sorted) and see if they all have different
                    // precise versions. If so, then `a` isn't actually in `b` so
                    // we'll let it through.
                    //
                    // Note that we only check this for non-registry sources,
                    // however, as registries contain enough version information in
                    // the package ID to disambiguate.
                    if a.source_id().is_registry() {
                        return false;
                    }
                    b[i..]
                        .iter()
                        .take_while(|b| a == b)
                        .all(|b| a.source_id().precise() != b.source_id().precise())
                })
                .cloned()
                .collect()
        }

        // Map `(package name, package source)` to `(removed versions, added versions)`.
        let mut changes = BTreeMap::new();
        let empty = (Vec::new(), Vec::new());
        for dep in previous_resolve.iter() {
            changes
                .entry(key(dep))
                .or_insert_with(|| empty.clone())
                .0
                .push(dep);
        }
        for dep in resolve.iter() {
            changes
                .entry(key(dep))
                .or_insert_with(|| empty.clone())
                .1
                .push(dep);
        }

        for v in changes.values_mut() {
            let (ref mut old, ref mut new) = *v;
            old.sort();
            new.sort();
            let removed = vec_subtract(old, new);
            let added = vec_subtract(new, old);
            *old = removed;
            *new = added;
        }
        debug!("{:#?}", changes);

        changes.into_iter().map(|(_, v)| v).collect()
    }
}
