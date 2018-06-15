use std::collections::{BTreeMap, HashSet, HashMap};

use termcolor::Color::{self, Cyan, Green, Red};

use core::PackageId;
use core::registry::PackageRegistry;
use core::{Resolve, SourceId, Workspace};
use core::resolver::Method;
use ops;
use util::config::Config;
use util::CargoResult;

pub struct UpdateOptions<'a> {
    pub config: &'a Config,
    pub to_update: Vec<String>,
    pub precise: Option<&'a str>,
    pub aggressive: bool,
}

pub fn generate_lockfile(ws: &Workspace) -> CargoResult<()> {
    let mut registry = PackageRegistry::new(ws.config())?;
    let resolve = ops::resolve_with_previous(
        &mut registry,
        ws,
        Method::Everything,
        None,
        None,
        &[],
        true,
        true,
    )?;
    ops::write_pkg_lockfile(ws, &resolve)?;
    Ok(())
}

pub fn update_lockfile(ws: &Workspace, opts: &UpdateOptions) -> CargoResult<()> {
    if opts.aggressive && opts.precise.is_some() {
        bail!("cannot specify both aggressive and precise simultaneously")
    }

    if ws.members().is_empty() {
        bail!("you can't generate a lockfile for an empty workspace.")
    }

    if opts.config.cli_unstable().offline {
        bail!("you can't update in the offline mode");
    }

    let previous_resolve = match ops::load_pkg_lockfile(ws)? {
        Some(resolve) => resolve,
        None => return generate_lockfile(ws),
    };
    let mut registry = PackageRegistry::new(opts.config)?;
    let mut to_avoid = HashSet::new();

    if opts.to_update.is_empty() {
        to_avoid.extend(previous_resolve.iter());
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
                            format!("{}={}->{}", dep.name(), dep.version(), precise)
                        } else {
                            precise.to_string()
                        };
                        dep.source_id().clone().with_precise(Some(precise))
                    }
                    None => dep.source_id().clone().with_precise(None),
                });
            }
        }
        registry.add_sources(&sources)?;
    }

    let resolve = ops::resolve_with_previous(
        &mut registry,
        ws,
        Method::Everything,
        Some(&previous_resolve),
        Some(&to_avoid),
        &[],
        true,
        true,
    )?;

    // Summarize what is changing for the user.
    let print_change = |status: &str, msg: String, color: Color| {
        opts.config.shell().status_with_color(status, msg, color)
    };
    let diff = compare_dependency_graphs(&previous_resolve, &resolve);
    for (removed, added) in diff.removed_and_added {
        if removed.len() == 1 && added.len() == 1 {
            let msg = format_package_for_update(removed[0], added[0]);
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

    for (package, dependents) in diff.partly_added {
        let dependents = format_dependents(&dependents);
        print_change("Adding", format!("{} to {}", package, dependents), Cyan)?;
    }

    for ((old, new), dependents) in diff.partly_updated {
        let dependents = format_dependents(&dependents);
        let msg = format_package_for_update(old, new);
        print_change("Updating", format!("{} in {}", msg, dependents), Green)?;
    }

    for (package, dependents) in diff.partly_removed {
        let dependents = format_dependents(&dependents);
        print_change("Removing", format!("{} from {}", package, dependents), Red)?;
    }

    ops::write_pkg_lockfile(ws, &resolve)?;
    return Ok(());

    fn format_dependents(packages: &[&PackageId]) -> String {
        packages.iter()
            .map(|p| format!("{} v{}", p.name(), p.version()))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn format_package_for_update(removed: &PackageId, added: &PackageId) -> String {
        if removed.source_id().is_git() {
            format!(
                "{} -> #{}",
                removed,
                &added.source_id().precise().unwrap()[..8]
            )
        } else {
            format!("{} -> v{}", removed, added.version())
        }
    }

    fn fill_with_deps<'a>(
        resolve: &'a Resolve,
        dep: &'a PackageId,
        set: &mut HashSet<&'a PackageId>,
        visited: &mut HashSet<&'a PackageId>,
    ) {
        if !visited.insert(dep) {
            return;
        }
        set.insert(dep);
        for dep in resolve.deps_not_replaced(dep) {
            fill_with_deps(resolve, dep, set, visited);
        }
    }

    struct DependencyGraphDiff<'a> {
        /// completely new, updated and removed dependencies
        removed_and_added: Vec<(Vec<&'a PackageId>, Vec<&'a PackageId>)>,
        /// dependencies that are not new, but some crates were not using them before
        partly_added: BTreeMap<&'a PackageId, Vec<&'a PackageId>>,
        /// dependencies that are updated, but there are still some crates using old versions
        partly_updated: BTreeMap<(&'a PackageId, &'a PackageId), Vec<&'a PackageId>>,
        /// dependencies that are no longer used by some packages
        partly_removed: BTreeMap<&'a PackageId, Vec<&'a PackageId>>,
    }

    fn compare_dependency_graphs<'a>(
        previous_resolve: &'a Resolve,
        resolve: &'a Resolve,
    ) -> DependencyGraphDiff<'a> {
        fn key(dep: &PackageId) -> (&str, &SourceId) {
            (dep.name().as_str(), dep.source_id())
        }

        // Removes all package ids in `b` from `a`. Note that this is somewhat
        // more complicated because the equality for source ids does not take
        // precise versions into account (e.g. git shas), but we want to take
        // that into account here.
        fn vec_subtract<'a>(a: &[&'a PackageId], b: &[&'a PackageId]) -> Vec<&'a PackageId> {
            a.iter()
                .filter(|a| {
                    // If this package id is not found in `b`, then it's definitely
                    // in the subtracted set
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
                    // the package id to disambiguate
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

        // Map (package name, package source) to (removed versions, added versions).
        let mut changes = BTreeMap::new();
        let mut versions = BTreeMap::new();
        let mut old_key_to_dep = HashMap::new();
        let mut key_to_dep = HashMap::new();
        let empty = (Vec::new(), Vec::new());
        for dep in previous_resolve.iter() {
            let entry = key(dep);
            old_key_to_dep.insert(entry, dep);
            changes
                .entry(entry)
                .or_insert_with(|| empty.clone())
                .0
                .push(dep);
            for (id, _) in previous_resolve.deps(dep) {
                versions
                    .entry(key(id))
                    .or_insert_with(BTreeMap::new)
                    .entry(entry)
                    .or_insert_with(|| (None, None))
                    .0 = Some(id);
            }
        }
        for dep in resolve.iter() {
            let entry = key(dep);
            key_to_dep.insert(entry, dep);
            changes
                .entry(entry)
                .or_insert_with(|| empty.clone())
                .1
                .push(dep);

            for (id, _) in resolve.deps(dep) {
                versions
                    .entry(key(id))
                    .or_insert_with(BTreeMap::new)
                    .entry(entry)
                    .or_insert_with(|| (None, None))
                    .1 = Some(id);
            }
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

        let mut partly_added = BTreeMap::new();
        let mut partly_updated = BTreeMap::new();
        let mut partly_removed = BTreeMap::new();

        {
            let dependents_iter = changes.iter()
                .filter(|(_, v)| v.0.len() == 0 && v.1.len() == 0)
                .filter_map(|(k, _)| versions.get(k));

            for dependents in dependents_iter {
                for (dependent, versions) in dependents {
                    match versions {
                        (None, Some(new)) => {
                            partly_added.entry(*new)
                                .or_insert_with(Vec::new)
                                .push(key_to_dep[dependent]);
                        },
                        (Some(old), Some(new)) if old != new => {
                            partly_updated.entry((*old, *new))
                                .or_insert_with(Vec::new)
                                .push(key_to_dep[dependent]);
                        },
                        (Some(old), None) => {
                            partly_removed.entry(*old)
                                .or_insert_with(Vec::new)
                                .push(old_key_to_dep[dependent]);
                        },
                        _ => (),
                    }
                }
            }
        }

        DependencyGraphDiff {
            removed_and_added: changes.into_iter().map(|(_, v)| v).collect(),
            partly_added,
            partly_updated,
            partly_removed,
        }
    }
}
