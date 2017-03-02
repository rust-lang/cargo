use std::collections::HashSet;

use core::{PackageId, PackageIdSpec, PackageSet, Source, SourceId, Workspace};
use core::registry::PackageRegistry;
use core::resolver::{self, Resolve, Method};
use sources::PathSource;
use ops;
use util::{profile, human, CargoResult, ChainError};

/// Resolve all dependencies for the workspace using the previous
/// lockfile as a guide if present.
///
/// This function will also write the result of resolution as a new
/// lockfile.
pub fn resolve_ws<'a>(ws: &Workspace<'a>) -> CargoResult<(PackageSet<'a>, Resolve)> {
    let mut registry = PackageRegistry::new(ws.config())?;
    let resolve = resolve_with_registry(ws, &mut registry)?;
    let packages = get_resolved_packages(&resolve, registry);
    Ok((packages, resolve))
}

/// Resolves dependencies for some packages of the workspace,
/// taking into account `paths` overrides and activated features.
pub fn resolve_ws_precisely<'a>(ws: &Workspace<'a>,
                                source: Option<Box<Source + 'a>>,
                                features: &[String],
                                all_features: bool,
                                no_default_features: bool,
                                specs: &[PackageIdSpec])
                                -> CargoResult<(PackageSet<'a>, Resolve)> {
    let features = features.iter().flat_map(|s| {
        s.split_whitespace()
    }).map(|s| s.to_string()).collect::<Vec<String>>();

    let mut registry = PackageRegistry::new(ws.config())?;
    if let Some(source) = source {
        registry.add_preloaded(source);
    }

    let resolve = if ws.require_optional_deps() {
        // First, resolve the root_package's *listed* dependencies, as well as
        // downloading and updating all remotes and such.
        let resolve = resolve_with_registry(ws, &mut registry)?;

        // Second, resolve with precisely what we're doing. Filter out
        // transitive dependencies if necessary, specify features, handle
        // overrides, etc.
        let _p = profile::start("resolving w/ overrides...");

        add_overrides(&mut registry, ws)?;

        Some(resolve)
    } else {
        None
    };

    let method = if all_features {
        Method::Everything
    } else {
        Method::Required {
            dev_deps: true, // TODO: remove this option?
            features: &features,
            uses_default_features: !no_default_features,
        }
    };

    let resolved_with_overrides =
    ops::resolve_with_previous(&mut registry, ws,
                               method, resolve.as_ref(), None,
                               specs)?;

    for &(ref replace_spec, _) in ws.root_replace() {
        if !resolved_with_overrides.replacements().keys().any(|r| replace_spec.matches(r)) {
            ws.config().shell().warn(
                format!("package replacement is not used: {}", replace_spec)
            )?
        }
    }

    let packages = get_resolved_packages(&resolved_with_overrides, registry);

    Ok((packages, resolved_with_overrides))
}

fn resolve_with_registry(ws: &Workspace, registry: &mut PackageRegistry)
                         -> CargoResult<Resolve> {
    let prev = ops::load_pkg_lockfile(ws)?;
    let resolve = resolve_with_previous(registry, ws,
                                        Method::Everything,
                                        prev.as_ref(), None, &[])?;

    if !ws.is_ephemeral() {
        ops::write_pkg_lockfile(ws, &resolve)?;
    }
    Ok(resolve)
}


/// Resolve all dependencies for a package using an optional previous instance
/// of resolve to guide the resolution process.
///
/// This also takes an optional hash set, `to_avoid`, which is a list of package
/// ids that should be avoided when consulting the previous instance of resolve
/// (often used in pairings with updates).
///
/// The previous resolve normally comes from a lockfile. This function does not
/// read or write lockfiles from the filesystem.
pub fn resolve_with_previous<'a>(registry: &mut PackageRegistry,
                                 ws: &Workspace,
                                 method: Method,
                                 previous: Option<&'a Resolve>,
                                 to_avoid: Option<&HashSet<&'a PackageId>>,
                                 specs: &[PackageIdSpec])
                                 -> CargoResult<Resolve> {
    // Here we place an artificial limitation that all non-registry sources
    // cannot be locked at more than one revision. This means that if a git
    // repository provides more than one package, they must all be updated in
    // step when any of them are updated.
    //
    // TODO: This seems like a hokey reason to single out the registry as being
    //       different
    let mut to_avoid_sources = HashSet::new();
    if let Some(to_avoid) = to_avoid {
        to_avoid_sources.extend(to_avoid.iter()
                                        .map(|p| p.source_id())
                                        .filter(|s| !s.is_registry()));
    }

    // In the case where a previous instance of resolve is available, we
    // want to lock as many packages as possible to the previous version
    // without disturbing the graph structure. To this end we perform
    // two actions here:
    //
    // 1. We inform the package registry of all locked packages. This
    //    involves informing it of both the locked package's id as well
    //    as the versions of all locked dependencies. The registry will
    //    then takes this information into account when it is queried.
    //
    // 2. The specified package's summary will have its dependencies
    //    modified to their precise variants. This will instruct the
    //    first step of the resolution process to not query for ranges
    //    but rather for precise dependency versions.
    //
    //    This process must handle altered dependencies, however, as
    //    it's possible for a manifest to change over time to have
    //    dependencies added, removed, or modified to different version
    //    ranges. To deal with this, we only actually lock a dependency
    //    to the previously resolved version if the dependency listed
    //    still matches the locked version.
    if let Some(r) = previous {
        trace!("previous: {:?}", r);
        for node in r.iter().filter(|p| keep(p, to_avoid, &to_avoid_sources)) {
            let deps = r.deps_not_replaced(node)
                        .filter(|p| keep(p, to_avoid, &to_avoid_sources))
                        .cloned().collect();
            registry.register_lock(node.clone(), deps);
        }
    }

    let mut summaries = Vec::new();
    for member in ws.members() {
        registry.add_sources(&[member.package_id().source_id().clone()])?;
        let method_to_resolve = match method {
            // When everything for a workspace we want to be sure to resolve all
            // members in the workspace, so propagate the `Method::Everything`.
            Method::Everything => Method::Everything,

            // If we're not resolving everything though then we're constructing the
            // exact crate graph we're going to build. Here we don't necessarily
            // want to keep around all workspace crates as they may not all be
            // built/tested.
            //
            // Additionally, the `method` specified represents command line
            // flags, which really only matters for the current package
            // (determined by the cwd). If other packages are specified (via
            // `-p`) then the command line flags like features don't apply to
            // them.
            //
            // As a result, if this `member` is the current member of the
            // workspace, then we use `method` specified. Otherwise we use a
            // base method with no features specified but using default features
            // for any other packages specified with `-p`.
            Method::Required { dev_deps, .. } => {
                let base = Method::Required {
                    dev_deps: dev_deps,
                    features: &[],
                    uses_default_features: true,
                };
                let member_id = member.package_id();
                match ws.current_opt() {
                    Some(current) if member_id == current.package_id() => method,
                    _ => {
                        if specs.iter().any(|spec| spec.matches(member_id)) {
                            base
                        } else {
                            continue
                        }
                    }
                }
            }
        };

        let summary = registry.lock(member.summary().clone());
        summaries.push((summary, method_to_resolve));
    }

    let root_replace = ws.root_replace();

    let replace = match previous {
        Some(r) => {
            root_replace.iter().map(|&(ref spec, ref dep)| {
                for (key, val) in r.replacements().iter() {
                    if spec.matches(key) &&
                       dep.matches_id(val) &&
                       keep(&val, to_avoid, &to_avoid_sources) {
                        return (spec.clone(), dep.clone().lock_to(val))
                    }
                }
                (spec.clone(), dep.clone())
            }).collect::<Vec<_>>()
        }
        None => root_replace.to_vec(),
    };

    let mut resolved = resolver::resolve(&summaries, &replace, registry)?;
    if let Some(previous) = previous {
        resolved.merge_from(previous)?;
    }
    return Ok(resolved);

    fn keep<'a>(p: &&'a PackageId,
                to_avoid_packages: Option<&HashSet<&'a PackageId>>,
                to_avoid_sources: &HashSet<&'a SourceId>)
                -> bool {
        !to_avoid_sources.contains(&p.source_id()) && match to_avoid_packages {
            Some(set) => !set.contains(p),
            None => true,
        }
    }
}

/// Read the `paths` configuration variable to discover all path overrides that
/// have been configured.
fn add_overrides<'a>(registry: &mut PackageRegistry<'a>,
                     ws: &Workspace<'a>) -> CargoResult<()> {
    let paths = match ws.config().get_list("paths")? {
        Some(list) => list,
        None => return Ok(())
    };

    let paths = paths.val.iter().map(|&(ref s, ref p)| {
        // The path listed next to the string is the config file in which the
        // key was located, so we want to pop off the `.cargo/config` component
        // to get the directory containing the `.cargo` folder.
        (p.parent().unwrap().parent().unwrap().join(s), p)
    });

    for (path, definition) in paths {
        let id = SourceId::for_path(&path)?;
        let mut source = PathSource::new_recursive(&path, &id, ws.config());
        source.update().chain_error(|| {
            human(format!("failed to update path override `{}` \
                           (defined in `{}`)", path.display(),
                          definition.display()))
        })?;
        registry.add_override(Box::new(source));
    }
    Ok(())
}

fn get_resolved_packages<'a>(resolve: &Resolve,
                             registry: PackageRegistry<'a>)
                             -> PackageSet<'a> {
    let ids: Vec<PackageId> = resolve.iter().cloned().collect();
    registry.get(&ids)
}

