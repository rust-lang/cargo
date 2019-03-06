use std::collections::HashSet;

use log::{debug, trace};

use crate::core::registry::PackageRegistry;
use crate::core::resolver::{self, Method, Resolve};
use crate::core::{PackageId, PackageIdSpec, PackageSet, Source, SourceId, Workspace};
use crate::ops;
use crate::sources::PathSource;
use crate::util::errors::{CargoResult, CargoResultExt};
use crate::util::profile;

const UNUSED_PATCH_WARNING: &str = "\
Check that the patched package version and available features are compatible
with the dependency requirements. If the patch has a different version from
what is locked in the Cargo.lock file, run `cargo update` to use the new
version. This may also occur with an optional dependency that is not enabled.";

/// Resolves all dependencies for the workspace using the previous
/// lock file as a guide if present.
///
/// This function will also write the result of resolution as a new
/// lock file.
pub fn resolve_ws<'a>(ws: &Workspace<'a>) -> CargoResult<(PackageSet<'a>, Resolve)> {
    let mut registry = PackageRegistry::new(ws.config())?;
    let resolve = resolve_with_registry(ws, &mut registry, true)?;
    let packages = get_resolved_packages(&resolve, registry)?;
    Ok((packages, resolve))
}

/// Resolves dependencies for some packages of the workspace,
/// taking into account `paths` overrides and activated features.
pub fn resolve_ws_precisely<'a>(
    ws: &Workspace<'a>,
    source: Option<Box<dyn Source + 'a>>,
    features: &[String],
    all_features: bool,
    no_default_features: bool,
    specs: &[PackageIdSpec],
) -> CargoResult<(PackageSet<'a>, Resolve)> {
    let features = Method::split_features(features);
    let method = if all_features {
        Method::Everything
    } else {
        Method::Required {
            dev_deps: true,
            features: &features,
            all_features: false,
            uses_default_features: !no_default_features,
        }
    };
    resolve_ws_with_method(ws, source, method, specs)
}

pub fn resolve_ws_with_method<'a>(
    ws: &Workspace<'a>,
    source: Option<Box<dyn Source + 'a>>,
    method: Method<'_>,
    specs: &[PackageIdSpec],
) -> CargoResult<(PackageSet<'a>, Resolve)> {
    let mut registry = PackageRegistry::new(ws.config())?;
    if let Some(source) = source {
        registry.add_preloaded(source);
    }
    let mut add_patches = true;

    let resolve = if ws.require_optional_deps() {
        // First, resolve the root_package's *listed* dependencies, as well as
        // downloading and updating all remotes and such.
        let resolve = resolve_with_registry(ws, &mut registry, false)?;
        add_patches = false;

        // Second, resolve with precisely what we're doing. Filter out
        // transitive dependencies if necessary, specify features, handle
        // overrides, etc.
        let _p = profile::start("resolving with overrides...");

        add_overrides(&mut registry, ws)?;

        for &(ref replace_spec, ref dep) in ws.root_replace() {
            if !resolve
                .iter()
                .any(|r| replace_spec.matches(r) && !dep.matches_id(r))
            {
                ws.config()
                    .shell()
                    .warn(format!("package replacement is not used: {}", replace_spec))?
            }
        }

        Some(resolve)
    } else {
        ops::load_pkg_lockfile(ws)?
    };

    let resolved_with_overrides = ops::resolve_with_previous(
        &mut registry,
        ws,
        method,
        resolve.as_ref(),
        None,
        specs,
        add_patches,
        true,
    )?;

    let packages = get_resolved_packages(&resolved_with_overrides, registry)?;

    Ok((packages, resolved_with_overrides))
}

fn resolve_with_registry<'cfg>(
    ws: &Workspace<'cfg>,
    registry: &mut PackageRegistry<'cfg>,
    warn: bool,
) -> CargoResult<Resolve> {
    let prev = ops::load_pkg_lockfile(ws)?;
    let resolve = resolve_with_previous(
        registry,
        ws,
        Method::Everything,
        prev.as_ref(),
        None,
        &[],
        true,
        warn,
    )?;

    if !ws.is_ephemeral() {
        ops::write_pkg_lockfile(ws, &resolve)?;
    }
    Ok(resolve)
}

/// Resolves all dependencies for a package using an optional previous instance.
/// of resolve to guide the resolution process.
///
/// This also takes an optional hash set, `to_avoid`, which is a list of package
/// IDs that should be avoided when consulting the previous instance of resolve
/// (often used in pairings with updates).
///
/// The previous resolve normally comes from a lock file. This function does not
/// read or write lock files from the filesystem.
pub fn resolve_with_previous<'cfg>(
    registry: &mut PackageRegistry<'cfg>,
    ws: &Workspace<'cfg>,
    method: Method<'_>,
    previous: Option<&Resolve>,
    to_avoid: Option<&HashSet<PackageId>>,
    specs: &[PackageIdSpec],
    register_patches: bool,
    warn: bool,
) -> CargoResult<Resolve> {
    // Here we place an artificial limitation that all non-registry sources
    // cannot be locked at more than one revision. This means that if a Git
    // repository provides more than one package, they must all be updated in
    // step when any of them are updated.
    //
    // TODO: this seems like a hokey reason to single out the registry as being
    // different.
    let mut to_avoid_sources: HashSet<SourceId> = HashSet::new();
    if let Some(to_avoid) = to_avoid {
        to_avoid_sources.extend(
            to_avoid
                .iter()
                .map(|p| p.source_id())
                .filter(|s| !s.is_registry()),
        );
    }

    let keep = |p: &PackageId| {
        !to_avoid_sources.contains(&p.source_id())
            && match to_avoid {
                Some(set) => !set.contains(p),
                None => true,
            }
    };

    // In the case where a previous instance of resolve is available, we
    // want to lock as many packages as possible to the previous version
    // without disturbing the graph structure.
    let mut try_to_use = HashSet::new();
    if let Some(r) = previous {
        trace!("previous: {:?}", r);
        register_previous_locks(ws, registry, r, &keep);

        // Everything in the previous lock file we want to keep is prioritized
        // in dependency selection if it comes up, aka we want to have
        // conservative updates.
        try_to_use.extend(r.iter().filter(keep).inspect(|id| {
            debug!("attempting to prefer {}", id);
        }));
    }

    if register_patches {
        for (url, patches) in ws.root_patch() {
            let previous = match previous {
                Some(r) => r,
                None => {
                    registry.patch(url, patches)?;
                    continue;
                }
            };
            let patches = patches
                .iter()
                .map(|dep| {
                    let unused = previous.unused_patches().iter().cloned();
                    let candidates = previous.iter().chain(unused);
                    match candidates.filter(keep).find(|&id| dep.matches_id(id)) {
                        Some(id) => {
                            let mut dep = dep.clone();
                            dep.lock_to(id);
                            dep
                        }
                        None => dep.clone(),
                    }
                })
                .collect::<Vec<_>>();
            registry.patch(url, &patches)?;
        }

        registry.lock_patches();
    }

    for member in ws.members() {
        registry.add_sources(Some(member.package_id().source_id()))?;
    }

    let mut summaries = Vec::new();
    if ws.config().cli_unstable().package_features {
        let mut members = Vec::new();
        match method {
            Method::Everything => members.extend(ws.members()),
            Method::Required {
                features,
                all_features,
                uses_default_features,
                ..
            } => {
                if specs.len() > 1 && !features.is_empty() {
                    failure::bail!("cannot specify features for more than one package");
                }
                members.extend(
                    ws.members()
                        .filter(|m| specs.iter().any(|spec| spec.matches(m.package_id()))),
                );
                // Edge case: running `cargo build -p foo`, where `foo` is not a member
                // of current workspace. Add all packages from workspace to get `foo`
                // into the resolution graph.
                if members.is_empty() {
                    if !(features.is_empty() && !all_features && uses_default_features) {
                        failure::bail!("cannot specify features for packages outside of workspace");
                    }
                    members.extend(ws.members());
                }
            }
        }
        for member in members {
            let summary = registry.lock(member.summary().clone());
            summaries.push((summary, method))
        }
    } else {
        for member in ws.members() {
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
                Method::Required {
                    dev_deps,
                    all_features,
                    ..
                } => {
                    let base = Method::Required {
                        dev_deps,
                        features: &[],
                        all_features,
                        uses_default_features: true,
                    };
                    let member_id = member.package_id();
                    match ws.current_opt() {
                        Some(current) if member_id == current.package_id() => method,
                        _ => {
                            if specs.iter().any(|spec| spec.matches(member_id)) {
                                base
                            } else {
                                continue;
                            }
                        }
                    }
                }
            };

            let summary = registry.lock(member.summary().clone());
            summaries.push((summary, method_to_resolve));
        }
    };

    let root_replace = ws.root_replace();

    let replace = match previous {
        Some(r) => root_replace
            .iter()
            .map(|&(ref spec, ref dep)| {
                for (&key, &val) in r.replacements().iter() {
                    if spec.matches(key) && dep.matches_id(val) && keep(&val) {
                        let mut dep = dep.clone();
                        dep.lock_to(val);
                        return (spec.clone(), dep);
                    }
                }
                (spec.clone(), dep.clone())
            })
            .collect::<Vec<_>>(),
        None => root_replace.to_vec(),
    };

    ws.preload(registry);
    let mut resolved = resolver::resolve(
        &summaries,
        &replace,
        registry,
        &try_to_use,
        Some(ws.config()),
        warn,
        false, // TODO: use "public and private dependencies" feature flag
    )?;
    resolved.register_used_patches(registry.patches());
    if register_patches {
        // It would be good if this warning was more targeted and helpful
        // (such as showing close candidates that failed to match). However,
        // that's not terribly easy to do, so just show a general help
        // message.
        let warnings: Vec<String> = resolved
            .unused_patches()
            .iter()
            .map(|pkgid| format!("Patch `{}` was not used in the crate graph.", pkgid))
            .collect();
        if !warnings.is_empty() {
            ws.config().shell().warn(format!(
                "{}\n{}",
                warnings.join("\n"),
                UNUSED_PATCH_WARNING
            ))?;
        }
    }
    if let Some(previous) = previous {
        resolved.merge_from(previous)?;
    }
    Ok(resolved)
}

/// Read the `paths` configuration variable to discover all path overrides that
/// have been configured.
pub fn add_overrides<'a>(
    registry: &mut PackageRegistry<'a>,
    ws: &Workspace<'a>,
) -> CargoResult<()> {
    let paths = match ws.config().get_list("paths")? {
        Some(list) => list,
        None => return Ok(()),
    };

    let paths = paths.val.iter().map(|&(ref s, ref p)| {
        // The path listed next to the string is the config file in which the
        // key was located, so we want to pop off the `.cargo/config` component
        // to get the directory containing the `.cargo` folder.
        (p.parent().unwrap().parent().unwrap().join(s), p)
    });

    for (path, definition) in paths {
        let id = SourceId::for_path(&path)?;
        let mut source = PathSource::new_recursive(&path, id, ws.config());
        source.update().chain_err(|| {
            format!(
                "failed to update path override `{}` \
                 (defined in `{}`)",
                path.display(),
                definition.display()
            )
        })?;
        registry.add_override(Box::new(source));
    }
    Ok(())
}

pub fn get_resolved_packages<'a>(
    resolve: &Resolve,
    registry: PackageRegistry<'a>,
) -> CargoResult<PackageSet<'a>> {
    let ids: Vec<PackageId> = resolve.iter().collect();
    registry.get(&ids)
}

/// In this function we're responsible for informing the `registry` of all
/// locked dependencies from the previous lock file we had, `resolve`.
///
/// This gets particularly tricky for a couple of reasons. The first is that we
/// want all updates to be conservative, so we actually want to take the
/// `resolve` into account (and avoid unnecessary registry updates and such).
/// the second, however, is that we want to be resilient to updates of
/// manifests. For example if a dependency is added or a version is changed we
/// want to make sure that we properly re-resolve (conservatively) instead of
/// providing an opaque error.
///
/// The logic here is somewhat subtle, but there should be more comments below to
/// clarify things.
///
/// Note that this function, at the time of this writing, is basically the
/// entire fix for issue #4127.
fn register_previous_locks(
    ws: &Workspace<'_>,
    registry: &mut PackageRegistry<'_>,
    resolve: &Resolve,
    keep: &dyn Fn(&PackageId) -> bool,
) {
    let path_pkg = |id: SourceId| {
        if !id.is_path() {
            return None;
        }
        if let Ok(path) = id.url().to_file_path() {
            if let Ok(pkg) = ws.load(&path.join("Cargo.toml")) {
                return Some(pkg);
            }
        }
        None
    };

    // Ok so we've been passed in a `keep` function which basically says "if I
    // return `true` then this package wasn't listed for an update on the command
    // line". That is, if we run `cargo update -p foo` then `keep(bar)` will return
    // `true`, whereas `keep(foo)` will return `false` (roughly speaking).
    //
    // This isn't actually quite what we want, however. Instead we want to
    // further refine this `keep` function with *all transitive dependencies* of
    // the packages we're not keeping. For example, consider a case like this:
    //
    // * There's a crate `log`.
    // * There's a crate `serde` which depends on `log`.
    //
    // Let's say we then run `cargo update -p serde`. This may *also* want to
    // update the `log` dependency as our newer version of `serde` may have a
    // new minimum version required for `log`. Now this isn't always guaranteed
    // to work. What'll happen here is we *won't* lock the `log` dependency nor
    // the `log` crate itself, but we will inform the registry "please prefer
    // this version of `log`". That way if our newer version of serde works with
    // the older version of `log`, we conservatively won't update `log`. If,
    // however, nothing else in the dependency graph depends on `log` and the
    // newer version of `serde` requires a new version of `log` it'll get pulled
    // in (as we didn't accidentally lock it to an old version).
    //
    // Additionally, here we process all path dependencies listed in the previous
    // resolve. They can not only have their dependencies change but also
    // the versions of the package change as well. If this ends up happening
    // then we want to make sure we don't lock a package ID node that doesn't
    // actually exist. Note that we don't do transitive visits of all the
    // package's dependencies here as that'll be covered below to poison those
    // if they changed.
    let mut avoid_locking = HashSet::new();
    registry.add_to_yanked_whitelist(resolve.iter().filter(keep));
    for node in resolve.iter() {
        if !keep(&node) {
            add_deps(resolve, node, &mut avoid_locking);
        } else if let Some(pkg) = path_pkg(node.source_id()) {
            if pkg.package_id() != node {
                avoid_locking.insert(node);
            }
        }
    }

    // Ok, but the above loop isn't the entire story! Updates to the dependency
    // graph can come from two locations, the `cargo update` command or
    // manifests themselves. For example a manifest on the filesystem may
    // have been updated to have an updated version requirement on `serde`. In
    // this case both `keep(serde)` and `keep(log)` return `true` (the `keep`
    // that's an argument to this function). We, however, don't want to keep
    // either of those! Otherwise we'll get obscure resolve errors about locked
    // versions.
    //
    // To solve this problem we iterate over all packages with path sources
    // (aka ones with manifests that are changing) and take a look at all of
    // their dependencies. If any dependency does not match something in the
    // previous lock file, then we're guaranteed that the main resolver will
    // update the source of this dependency no matter what. Knowing this we
    // poison all packages from the same source, forcing them all to get
    // updated.
    //
    // This may seem like a heavy hammer, and it is! It means that if you change
    // anything from crates.io then all of crates.io becomes unlocked. Note,
    // however, that we still want conservative updates. This currently happens
    // because the first candidate the resolver picks is the previously locked
    // version, and only if that fails to activate to we move on and try
    // a different version. (giving the guise of conservative updates)
    //
    // For example let's say we had `serde = "0.1"` written in our lock file.
    // When we later edit this to `serde = "0.1.3"` we don't want to lock serde
    // at its old version, 0.1.1. Instead we want to allow it to update to
    // `0.1.3` and update its own dependencies (like above). To do this *all
    // crates from crates.io* are not locked (aka added to `avoid_locking`).
    // For dependencies like `log` their previous version in the lock file will
    // come up first before newer version, if newer version are available.
    let mut path_deps = ws.members().cloned().collect::<Vec<_>>();
    let mut visited = HashSet::new();
    while let Some(member) = path_deps.pop() {
        if !visited.insert(member.package_id()) {
            continue;
        }
        for dep in member.dependencies() {
            // If this dependency didn't match anything special then we may want
            // to poison the source as it may have been added. If this path
            // dependencies is **not** a workspace member, however, and it's an
            // optional/non-transitive dependency then it won't be necessarily
            // be in our lock file. If this shows up then we avoid poisoning
            // this source as otherwise we'd repeatedly update the registry.
            //
            // TODO: this breaks adding an optional dependency in a
            // non-workspace member and then simultaneously editing the
            // dependency on that crate to enable the feature. For now,
            // this bug is better than the always-updating registry though.
            if !ws
                .members()
                .any(|pkg| pkg.package_id() == member.package_id())
                && (dep.is_optional() || !dep.is_transitive())
            {
                continue;
            }

            // If this is a path dependency, then try to push it onto our
            // worklist.
            if let Some(pkg) = path_pkg(dep.source_id()) {
                path_deps.push(pkg);
                continue;
            }

            // If we match *anything* in the dependency graph then we consider
            // ourselves all ok, and assume that we'll resolve to that.
            if resolve.iter().any(|id| dep.matches_ignoring_source(id)) {
                continue;
            }

            // Ok if nothing matches, then we poison the source of these
            // dependencies and the previous lock file.
            debug!(
                "poisoning {} because {} looks like it changed {}",
                dep.source_id(),
                member.package_id(),
                dep.package_name()
            );
            for id in resolve
                .iter()
                .filter(|id| id.source_id() == dep.source_id())
            {
                add_deps(resolve, id, &mut avoid_locking);
            }
        }
    }

    // Alright now that we've got our new, fresh, shiny, and refined `keep`
    // function let's put it to action. Take a look at the previous lock file,
    // filter everything by this callback, and then shove everything else into
    // the registry as a locked dependency.
    let keep = |id: &PackageId| keep(id) && !avoid_locking.contains(id);

    for node in resolve.iter().filter(keep) {
        let deps = resolve.deps_not_replaced(node).filter(keep).collect();
        registry.register_lock(node, deps);
    }

    /// Recursively add `node` and all its transitive dependencies to `set`.
    fn add_deps(resolve: &Resolve, node: PackageId, set: &mut HashSet<PackageId>) {
        if !set.insert(node) {
            return;
        }
        debug!("ignoring any lock pointing directly at {}", node);
        for dep in resolve.deps_not_replaced(node) {
            add_deps(resolve, dep, set);
        }
    }
}
