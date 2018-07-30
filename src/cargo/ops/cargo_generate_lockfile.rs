use std::collections::{BTreeMap, HashSet};

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

    let diffs = compare_dependency_graphs(&previous_resolve, &resolve);

    for diff in diffs {
        for ((old, new), _) in diff.updated {
            let msg = format_package_for_update(old, new);
            print_change("Updating", format!("{}", msg), Green)?;
        }

        for (package, _) in diff.removed {
            print_change("Removing", format!("{}", package), Red)?;
        }

        for (package, _) in diff.added {
            print_change("Adding", format!("{}", package), Cyan)?;
        }
    }

    ops::write_pkg_lockfile(ws, &resolve)?;
    return Ok(());

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

    fn equal_packages(a: &PackageId, b: &PackageId) -> bool {
        if a.source_id().is_registry() && b.source_id().is_registry() {
            a.version() == b.version()
        } else {
            a.source_id().precise() == b.source_id().precise()
        }
    }
    
    struct DependencyGraphDiff<'a> {
        /// dependencies that are not new
        added: BTreeMap<&'a PackageId, Vec<&'a PackageId>>,
        /// dependencies that are updated
        updated: BTreeMap<(&'a PackageId, &'a PackageId), Vec<&'a PackageId>>,
        /// dependencies that are no longer used
        removed: BTreeMap<&'a PackageId, Vec<&'a PackageId>>,
    }

    fn compare_dependency_graphs<'a>(
        previous_resolve: &'a Resolve,
        resolve: &'a Resolve,
    ) -> Vec<DependencyGraphDiff<'a>> {
        fn key(dep: &PackageId) -> (&str, &SourceId) {
            (dep.name().as_str(), dep.source_id())
        }

        // Map (package name, package source) to (removed versions, added versions).
        let mut changes = BTreeMap::new();
        for dep in previous_resolve.iter() {
            for (id, _) in previous_resolve.deps(dep) {
                changes 
                    .entry(key(id))
                    .or_insert_with(BTreeMap::new)
                    .entry(dep)
                    .or_insert_with(|| (None, None))
                    .0 = Some(id);
            }
        }
        for dep in resolve.iter() {
            for (id, _) in resolve.deps(dep) {
                changes 
                    .entry(key(id))
                    .or_insert_with(BTreeMap::new)
                    .entry(dep)
                    .or_insert_with(|| (None, None))
                    .1 = Some(id);
            }
        }

        debug!("{:#?}", changes);

        let mut diffs = Vec::new();

        for (_, dependents) in changes {
            let mut added = BTreeMap::new();
            let mut updated = BTreeMap::new();
            let mut removed = BTreeMap::new();
            let mut unchanged = HashSet::new();

            for (dependent, versions) in dependents {
                match versions {
                    (None, Some(new)) => {
                        added.entry(new)
                            .or_insert_with(Vec::new)
                            .push(dependent);
                    },
                    (Some(old), Some(new)) if !equal_packages(old, new) => {
                        updated.entry((old, new))
                            .or_insert_with(Vec::new)
                            .push(dependent);
                    },
                    (Some(old), None) => {
                        removed.entry(old)
                            .or_insert_with(Vec::new)
                            .push(dependent);
                    },
                    (Some(_), Some(new)) => {
                        unchanged.insert(new);
                    },
                    _ => (),
                }
            }

            // if it is still one of our dependencies,
            // there is no reason to print the message
            for dep in unchanged {
                added.remove(dep);
                removed.remove(dep);
            }

            // if it is removed or added, but also updated,
            // there is no reason to print the message
            for ((old, new), _) in updated.iter() {
                added.remove(new);
                removed.remove(old);
            }

            // handle cases when crate and it's dependency has been updated at the same time
            // e.g.
            //
            // foo 0.1.0
            //   -> bar 0.1.0
            //
            // foo 0.1.1
            //   -> bar 0.1.1
            if removed.len() == 1 && added.len() == 1 {
                for ((old, _), (new, dependents)) in removed.iter().zip(added.iter()) {
                    if !equal_packages(old, new) {
                        updated.entry((*old, *new))
                            .or_insert_with(Vec::new)
                            .extend(dependents);
                    }
                }
                added.clear();
                removed.clear();
            }

            let diff = DependencyGraphDiff {
                added,
                updated,
                removed,
            };

            diffs.push(diff);
        }

        diffs
    }
}
