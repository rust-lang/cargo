//! High-level APIs for executing the resolver.
//!
//! This module provides functions for running the resolver given a workspace, including loading
//! the `Cargo.lock` file and checking if it needs updating.
//!
//! There are roughly 3 main functions:
//!
//! - [`resolve_ws`]: A simple, high-level function with no options.
//! - [`resolve_ws_with_opts`]: A medium-level function with options like
//!   user-provided features. This is the most appropriate function to use in
//!   most cases.
//! - [`resolve_with_previous`]: A low-level function for running the resolver,
//!   providing the most power and flexibility.
//!
//! ### Data Structures
//!
//! - [`Workspace`]:
//!   Usually created by [`crate::util::command_prelude::ArgMatchesExt::workspace`] which discovers the root of the
//!   workspace, and loads all the workspace members as a [`Package`] object
//!   - [`Package`]
//!     Corresponds with `Cargo.toml` manifest (deserialized as [`Manifest`]) and its associated files.
//!     - [`Target`]s are crates such as the library, binaries, integration test, or examples.
//!       They are what is actually compiled by `rustc`.
//!       Each `Target` defines a crate root, like `src/lib.rs` or `examples/foo.rs`.
//!     - [`PackageId`] --- A unique identifier for a package.
//! - [`PackageRegistry`]:
//!   The primary interface for how the dependency
//!   resolver finds packages. It contains the `SourceMap`, and handles things
//!   like the `[patch]` table. The dependency resolver
//!   sends a query to the `PackageRegistry` to "get me all packages that match
//!   this dependency declaration". The `Registry` trait provides a generic interface
//!   to the `PackageRegistry`, but this is only used for providing an alternate
//!   implementation of the `PackageRegistry` for testing.
//! - [`SourceMap`]: Map of all available sources.
//!   - [`Source`]: An abstraction for something that can fetch packages (a remote
//!     registry, a git repo, the local filesystem, etc.). Check out the [source
//!     implementations] for all the details about registries, indexes, git
//!     dependencies, etc.
//!       * [`SourceId`]: A unique identifier for a source.
//!   - [`Summary`]: A of a [`Manifest`], and is essentially
//!     the information that can be found in a registry index. Queries against the
//!     `PackageRegistry` yields a `Summary`. The resolver uses the summary
//!     information to build the dependency graph.
//! - [`PackageSet`] --- Contains all the `Package` objects. This works with the
//!   [`Downloads`] struct to coordinate downloading packages. It has a reference
//!   to the `SourceMap` to get the `Source` objects which tell the `Downloads`
//!   struct which URLs to fetch.
//!
//! [`Package`]: crate::core::package
//! [`Target`]: crate::core::Target
//! [`Manifest`]: crate::core::Manifest
//! [`Source`]: crate::sources::source::Source
//! [`SourceMap`]: crate::sources::source::SourceMap
//! [`PackageRegistry`]: crate::core::registry::PackageRegistry
//! [source implementations]: crate::sources
//! [`Downloads`]: crate::core::package::Downloads

use crate::core::Dependency;
use crate::core::GitReference;
use crate::core::PackageId;
use crate::core::PackageIdSpec;
use crate::core::PackageIdSpecQuery;
use crate::core::PackageSet;
use crate::core::SourceId;
use crate::core::Workspace;
use crate::core::compiler::{CompileKind, RustcTargetData};
use crate::core::registry::{LockedPatchDependency, PackageRegistry};
use crate::core::resolver::features::{
    CliFeatures, FeatureOpts, FeatureResolver, ForceAllTargets, RequestedFeatures, ResolvedFeatures,
};
use crate::core::resolver::{
    self, HasDevUnits, Resolve, ResolveOpts, ResolveVersion, VersionOrdering, VersionPreferences,
};
use crate::core::summary::Summary;
use crate::ops;
use crate::sources::RecursivePathSource;
use crate::util::CanonicalUrl;
use crate::util::cache_lock::CacheLockMode;
use crate::util::context::FeatureUnification;
use crate::util::errors::CargoResult;
use annotate_snippets::Group;
use annotate_snippets::Level;
use anyhow::Context as _;
use cargo_util::paths;
use cargo_util_schemas::core::PartialVersion;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use tracing::{debug, trace};

/// Filter for keep using Package ID from previous lockfile.
type Keep<'a> = &'a dyn Fn(&PackageId) -> bool;

/// Result for `resolve_ws_with_opts`.
pub struct WorkspaceResolve<'gctx> {
    /// Packages to be downloaded.
    pub pkg_set: PackageSet<'gctx>,
    /// The resolve for the entire workspace.
    ///
    /// This may be `None` for things like `cargo install` and `-Zavoid-dev-deps`.
    /// This does not include `paths` overrides.
    pub workspace_resolve: Option<Resolve>,
    /// The narrowed resolve, with the specific features enabled.
    pub targeted_resolve: Resolve,
    /// Package specs requested for compilation along with specific features enabled. This usually
    /// has the length of one but there may be more specs with different features when using the
    /// `package` feature resolver.
    pub specs_and_features: Vec<SpecsAndResolvedFeatures>,
}

/// Pair of package specs requested for compilation along with enabled features.
pub struct SpecsAndResolvedFeatures {
    /// Packages that are supposed to be built.
    pub specs: Vec<PackageIdSpec>,
    /// The features activated per package.
    pub resolved_features: ResolvedFeatures,
}

const UNUSED_PATCH_WARNING: &str = "\
Check that the patched package version and available features are compatible
with the dependency requirements. If the patch has a different version from
what is locked in the Cargo.lock file, run `cargo update` to use the new
version. This may also occur with an optional dependency that is not enabled.";

/// Resolves all dependencies for the workspace using the previous
/// lock file as a guide if present.
///
/// This function will also write the result of resolution as a new lock file
/// (unless it is an ephemeral workspace such as `cargo install` or `cargo
/// package`).
///
/// This is a simple interface used by commands like `clean`, `fetch`, and
/// `package`, which don't specify any options or features.
pub fn resolve_ws<'a>(ws: &Workspace<'a>, dry_run: bool) -> CargoResult<(PackageSet<'a>, Resolve)> {
    let mut registry = ws.package_registry()?;
    let resolve = resolve_with_registry(ws, &mut registry, dry_run)?;
    let packages = get_resolved_packages(&resolve, registry)?;
    Ok((packages, resolve))
}

/// Resolves dependencies for some packages of the workspace,
/// taking into account `paths` overrides and activated features.
///
/// This function will also write the result of resolution as a new lock file
/// (unless `Workspace::require_optional_deps` is false, such as `cargo
/// install` or `-Z avoid-dev-deps`), or it is an ephemeral workspace (`cargo
/// install` or `cargo package`).
///
/// `specs` may be empty, which indicates it should resolve all workspace
/// members. In this case, `opts.all_features` must be `true`.
pub fn resolve_ws_with_opts<'gctx>(
    ws: &Workspace<'gctx>,
    target_data: &mut RustcTargetData<'gctx>,
    requested_targets: &[CompileKind],
    cli_features: &CliFeatures,
    specs: &[PackageIdSpec],
    has_dev_units: HasDevUnits,
    force_all_targets: ForceAllTargets,
    dry_run: bool,
) -> CargoResult<WorkspaceResolve<'gctx>> {
    let feature_unification = ws.resolve_feature_unification();
    let individual_specs = match feature_unification {
        FeatureUnification::Selected => vec![specs.to_owned()],
        FeatureUnification::Workspace => {
            vec![ops::Packages::All(Vec::new()).to_package_id_specs(ws)?]
        }
        FeatureUnification::Package => specs.iter().map(|spec| vec![spec.clone()]).collect(),
    };
    let specs: Vec<_> = individual_specs
        .iter()
        .map(|specs| specs.iter())
        .flatten()
        .cloned()
        .collect();
    let specs = &specs[..];
    let mut registry = ws.package_registry()?;
    let (resolve, resolved_with_overrides) = if ws.ignore_lock() {
        let add_patches = true;
        let resolve = None;
        let resolved_with_overrides = resolve_with_previous(
            &mut registry,
            ws,
            cli_features,
            has_dev_units,
            resolve.as_ref(),
            None,
            specs,
            add_patches,
        )?;
        ops::print_lockfile_changes(ws, None, &resolved_with_overrides, &mut registry)?;
        (resolve, resolved_with_overrides)
    } else if ws.require_optional_deps() {
        // First, resolve the root_package's *listed* dependencies, as well as
        // downloading and updating all remotes and such.
        let resolve = resolve_with_registry(ws, &mut registry, dry_run)?;
        // No need to add patches again, `resolve_with_registry` has done it.
        let add_patches = false;

        // Second, resolve with precisely what we're doing. Filter out
        // transitive dependencies if necessary, specify features, handle
        // overrides, etc.
        add_overrides(&mut registry, ws)?;

        for (replace_spec, dep) in ws.root_replace() {
            if !resolve
                .iter()
                .any(|r| replace_spec.matches(r) && !dep.matches_id(r))
            {
                ws.gctx()
                    .shell()
                    .warn(format!("package replacement is not used: {}", replace_spec))?
            }

            let mut unused_fields = Vec::new();
            if dep.features().len() != 0 {
                unused_fields.push("`features`");
            }
            if !dep.uses_default_features() {
                unused_fields.push("`default-features`")
            }
            if !unused_fields.is_empty() {
                ws.gctx().shell().print_report(
                    &[Level::WARNING
                        .secondary_title(format!(
                            "unused field in replacement for `{}`: {}",
                            dep.package_name(),
                            unused_fields.join(", ")
                        ))
                        .element(Level::NOTE.message(format!(
                            "configure {} in the `dependencies` entry",
                            unused_fields.join(", ")
                        )))],
                    false,
                )?;
            }
        }

        let resolved_with_overrides = resolve_with_previous(
            &mut registry,
            ws,
            cli_features,
            has_dev_units,
            Some(&resolve),
            None,
            specs,
            add_patches,
        )?;
        (Some(resolve), resolved_with_overrides)
    } else {
        let add_patches = true;
        let resolve = ops::load_pkg_lockfile(ws)?;
        let resolved_with_overrides = resolve_with_previous(
            &mut registry,
            ws,
            cli_features,
            has_dev_units,
            resolve.as_ref(),
            None,
            specs,
            add_patches,
        )?;
        // Skipping `print_lockfile_changes` as there are cases where this prints irrelevant
        // information
        (resolve, resolved_with_overrides)
    };

    let pkg_set = get_resolved_packages(&resolved_with_overrides, registry)?;

    let members_with_features = ws.members_with_features(specs, cli_features)?;
    let member_ids = members_with_features
        .iter()
        .map(|(p, _fts)| p.package_id())
        .collect::<Vec<_>>();
    pkg_set.download_accessible(
        &resolved_with_overrides,
        &member_ids,
        has_dev_units,
        requested_targets,
        target_data,
        force_all_targets,
    )?;

    let mut specs_and_features = Vec::new();

    for specs in individual_specs {
        let feature_opts = FeatureOpts::new(ws, has_dev_units, force_all_targets)?;

        // We want to narrow the features to the current specs so that stuff like `cargo check -p a
        // -p b -F a/a,b/b` works and the resolver does not contain that `a` does not have feature
        // `b` and vice-versa. However, resolver v1 needs to see even features of unselected
        // packages turned on if it was because of working directory being inside the unselected
        // package, because they might turn on a feature of a selected package.
        let narrowed_features = match feature_unification {
            FeatureUnification::Package => {
                let mut narrowed_features = cli_features.clone();
                let enabled_features = members_with_features
                    .iter()
                    .filter_map(|(package, cli_features)| {
                        specs
                            .iter()
                            .any(|spec| spec.matches(package.package_id()))
                            .then_some(cli_features.features.iter())
                    })
                    .flatten()
                    .cloned()
                    .collect();
                narrowed_features.features = Rc::new(enabled_features);
                Cow::Owned(narrowed_features)
            }
            FeatureUnification::Selected | FeatureUnification::Workspace => {
                Cow::Borrowed(cli_features)
            }
        };

        let resolved_features = FeatureResolver::resolve(
            ws,
            target_data,
            &resolved_with_overrides,
            &pkg_set,
            &*narrowed_features,
            &specs,
            requested_targets,
            feature_opts,
        )?;

        pkg_set.warn_no_lib_packages_and_artifact_libs_overlapping_deps(
            ws,
            &resolved_with_overrides,
            &member_ids,
            has_dev_units,
            requested_targets,
            target_data,
            force_all_targets,
        )?;

        specs_and_features.push(SpecsAndResolvedFeatures {
            specs,
            resolved_features,
        });
    }

    Ok(WorkspaceResolve {
        pkg_set,
        workspace_resolve: resolve,
        targeted_resolve: resolved_with_overrides,
        specs_and_features,
    })
}

#[tracing::instrument(skip_all)]
fn resolve_with_registry<'gctx>(
    ws: &Workspace<'gctx>,
    registry: &mut PackageRegistry<'gctx>,
    dry_run: bool,
) -> CargoResult<Resolve> {
    let prev = ops::load_pkg_lockfile(ws)?;
    let mut resolve = resolve_with_previous(
        registry,
        ws,
        &CliFeatures::new_all(true),
        HasDevUnits::Yes,
        prev.as_ref(),
        None,
        &[],
        true,
    )?;

    let print = if !ws.is_ephemeral() && ws.require_optional_deps() {
        if !dry_run {
            ops::write_pkg_lockfile(ws, &mut resolve)?
        } else {
            true
        }
    } else {
        // This mostly represents
        // - `cargo install --locked` and the only change is the package is no longer local but
        //   from the registry which is noise
        // - publish of libraries
        false
    };
    if print {
        ops::print_lockfile_changes(ws, prev.as_ref(), &resolve, registry)?;
    }
    Ok(resolve)
}

/// Resolves all dependencies for a package using an optional previous instance
/// of resolve to guide the resolution process.
///
/// This also takes an optional filter `keep_previous`, which informs the `registry`
/// which package ID should be locked to the previous instance of resolve
/// (often used in pairings with updates). See comments in [`register_previous_locks`]
/// for scenarios that might override this.
///
/// The previous resolve normally comes from a lock file. This function does not
/// read or write lock files from the filesystem.
///
/// `specs` may be empty, which indicates it should resolve all workspace
/// members. In this case, `opts.all_features` must be `true`.
///
/// If `register_patches` is true, then entries from the `[patch]` table in
/// the manifest will be added to the given `PackageRegistry`.
#[tracing::instrument(skip_all)]
pub fn resolve_with_previous<'gctx>(
    registry: &mut PackageRegistry<'gctx>,
    ws: &Workspace<'gctx>,
    cli_features: &CliFeatures,
    has_dev_units: HasDevUnits,
    previous: Option<&Resolve>,
    keep_previous: Option<Keep<'_>>,
    specs: &[PackageIdSpec],
    register_patches: bool,
) -> CargoResult<Resolve> {
    // We only want one Cargo at a time resolving a crate graph since this can
    // involve a lot of frobbing of the global caches.
    let _lock = ws
        .gctx()
        .acquire_package_cache_lock(CacheLockMode::DownloadExclusive)?;

    // Some packages are already loaded when setting up a workspace. This
    // makes it so anything that was already loaded will not be loaded again.
    // Without this there were cases where members would be parsed multiple times
    ws.preload(registry);

    // In case any members were not already loaded or the Workspace is_ephemeral.
    for member in ws.members() {
        registry.add_sources(Some(member.package_id().source_id()))?;
    }

    // Try to keep all from previous resolve if no instruction given.
    let keep_previous = keep_previous.unwrap_or(&|_| true);

    // While registering patches, we will record preferences for particular versions
    // of various packages.
    let mut version_prefs = VersionPreferences::default();
    if ws.gctx().cli_unstable().minimal_versions {
        version_prefs.version_ordering(VersionOrdering::MinimumVersionsFirst)
    }
    if ws.resolve_honors_rust_version() {
        let mut rust_versions: Vec<_> = ws
            .members()
            .filter_map(|p| p.rust_version().map(|rv| rv.as_partial().clone()))
            .collect();
        if rust_versions.is_empty() {
            let rustc = ws.gctx().load_global_rustc(Some(ws))?;
            let rust_version: PartialVersion = rustc.version.clone().into();
            rust_versions.push(rust_version);
        }
        version_prefs.rust_versions(rust_versions);
    }

    let avoid_patch_ids = if register_patches {
        register_patch_entries(registry, ws, previous, &mut version_prefs, keep_previous)?
    } else {
        HashSet::new()
    };

    // Refine `keep` with patches that should avoid locking.
    let keep = |p: &PackageId| keep_previous(p) && !avoid_patch_ids.contains(p);

    let dev_deps = ws.require_optional_deps() || has_dev_units == HasDevUnits::Yes;

    if let Some(r) = previous {
        trace!("previous: {:?}", r);

        // In the case where a previous instance of resolve is available, we
        // want to lock as many packages as possible to the previous version
        // without disturbing the graph structure.
        register_previous_locks(ws, registry, r, &keep, dev_deps);

        // Prefer to use anything in the previous lock file, aka we want to have conservative updates.
        let _span = tracing::span!(tracing::Level::TRACE, "prefer_package_id").entered();
        for id in r.iter().filter(keep) {
            debug!("attempting to prefer {}", id);
            version_prefs.prefer_package_id(id);
        }
    }

    if register_patches {
        registry.lock_patches();
    }

    let summaries: Vec<(Summary, ResolveOpts)> = {
        let _span = tracing::span!(tracing::Level::TRACE, "registry.lock").entered();
        ws.members_with_features(specs, cli_features)?
            .into_iter()
            .map(|(member, features)| {
                let summary = registry.lock(member.summary().clone());
                (
                    summary,
                    ResolveOpts {
                        dev_deps,
                        features: RequestedFeatures::CliFeatures(features),
                    },
                )
            })
            .collect()
    };

    let replace = lock_replacements(ws, previous, &keep);

    let mut resolved = resolver::resolve(
        &summaries,
        &replace,
        registry,
        &version_prefs,
        ResolveVersion::with_rust_version(ws.lowest_rust_version()),
        Some(ws.gctx()),
    )?;

    let patches = registry.patches().values().flat_map(|v| v.iter());
    resolved.register_used_patches(patches);

    if register_patches && !resolved.unused_patches().is_empty() {
        emit_warnings_of_unused_patches(ws, &resolved, registry)?;
    }

    if let Some(previous) = previous {
        resolved.merge_from(previous)?;
    }
    let gctx = ws.gctx();
    let mut deferred = gctx.deferred_global_last_use()?;
    deferred.save_no_error(gctx);
    Ok(resolved)
}

/// Read the `paths` configuration variable to discover all path overrides that
/// have been configured.
#[tracing::instrument(skip_all)]
pub fn add_overrides<'a>(
    registry: &mut PackageRegistry<'a>,
    ws: &Workspace<'a>,
) -> CargoResult<()> {
    let gctx = ws.gctx();
    let Some(paths) = gctx.paths_overrides()? else {
        return Ok(());
    };

    let paths = paths.val.iter().map(|(s, def)| {
        // The path listed next to the string is the config file in which the
        // key was located, so we want to pop off the `.cargo/config` component
        // to get the directory containing the `.cargo` folder.
        (paths::normalize_path(&def.root(gctx.cwd()).join(s)), def)
    });

    for (path, definition) in paths {
        let id = SourceId::for_path(&path)?;
        let mut source = RecursivePathSource::new(&path, id, ws.gctx());
        source.load().with_context(|| {
            format!(
                "failed to update path override `{}` \
                 (defined in `{}`)",
                path.display(),
                definition
            )
        })?;
        registry.add_override(Box::new(source));
    }
    Ok(())
}

pub fn get_resolved_packages<'gctx>(
    resolve: &Resolve,
    registry: PackageRegistry<'gctx>,
) -> CargoResult<PackageSet<'gctx>> {
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
#[tracing::instrument(skip_all)]
fn register_previous_locks(
    ws: &Workspace<'_>,
    registry: &mut PackageRegistry<'_>,
    resolve: &Resolve,
    keep: Keep<'_>,
    dev_deps: bool,
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
    // line". That is, if we run `cargo update foo` then `keep(bar)` will return
    // `true`, whereas `keep(foo)` will return `false` (roughly speaking).
    //
    // This isn't actually quite what we want, however. Instead we want to
    // further refine this `keep` function with *all transitive dependencies* of
    // the packages we're not keeping. For example, consider a case like this:
    //
    // * There's a crate `log`.
    // * There's a crate `serde` which depends on `log`.
    //
    // Let's say we then run `cargo update serde`. This may *also* want to
    // update the `log` dependency as our newer version of `serde` may have a
    // new minimum version required for `log`. Now this isn't always guaranteed
    // to work. What'll happen here is we *won't* lock the `log` dependency nor
    // the `log` crate itself, but we will inform the registry "please prefer
    // this version of `log`". That way if our newer version of serde works with
    // the older version of `log`, we conservatively won't update `log`. If,
    // however, nothing else in the dependency graph depends on `log` and the
    // newer version of `serde` requires a new version of `log` it'll get pulled
    // in (as we didn't accidentally lock it to an old version).
    let mut avoid_locking = HashSet::new();
    registry.add_to_yanked_whitelist(resolve.iter().filter(keep));
    for node in resolve.iter() {
        if !keep(&node) {
            add_deps(resolve, node, &mut avoid_locking);
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
    {
        let _span = tracing::span!(tracing::Level::TRACE, "poison").entered();
        let mut path_deps = ws.members().cloned().collect::<Vec<_>>();
        let mut visited = HashSet::new();
        while let Some(member) = path_deps.pop() {
            if !visited.insert(member.package_id()) {
                continue;
            }
            let is_ws_member = ws.is_member(&member);
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
                if !is_ws_member && (dep.is_optional() || !dep.is_transitive()) {
                    continue;
                }

                // If dev-dependencies aren't being resolved, skip them.
                if !dep.is_transitive() && !dev_deps {
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
    }

    // Additionally, here we process all path dependencies listed in the previous
    // resolve. They can not only have their dependencies change but also
    // the versions of the package change as well. If this ends up happening
    // then we want to make sure we don't lock a package ID node that doesn't
    // actually exist. Note that we don't do transitive visits of all the
    // package's dependencies here as that'll be covered below to poison those
    // if they changed.
    //
    // This must come after all other `add_deps` calls to ensure it recursively walks the tree when
    // called.
    for node in resolve.iter() {
        if let Some(pkg) = path_pkg(node.source_id()) {
            if pkg.package_id() != node {
                avoid_locking.insert(node);
            }
        }
    }

    // Alright now that we've got our new, fresh, shiny, and refined `keep`
    // function let's put it to action. Take a look at the previous lock file,
    // filter everything by this callback, and then shove everything else into
    // the registry as a locked dependency.
    let keep = |id: &PackageId| keep(id) && !avoid_locking.contains(id);

    registry.clear_lock();
    {
        let _span = tracing::span!(tracing::Level::TRACE, "register_lock").entered();
        for node in resolve.iter().filter(keep) {
            let deps = resolve
                .deps_not_replaced(node)
                .map(|p| p.0)
                .filter(keep)
                .collect::<Vec<_>>();

            // In the v2 lockfile format and prior the `branch=master` dependency
            // directive was serialized the same way as the no-branch-listed
            // directive. Nowadays in Cargo, however, these two directives are
            // considered distinct and are no longer represented the same way. To
            // maintain compatibility with older lock files we register locked nodes
            // for *both* the master branch and the default branch.
            //
            // Note that this is only applicable for loading older resolves now at
            // this point. All new lock files are encoded as v3-or-later, so this is
            // just compat for loading an old lock file successfully.
            if let Some(node) = master_branch_git_source(node, resolve) {
                registry.register_lock(node, deps.clone());
            }

            registry.register_lock(node, deps);
        }
    }

    /// Recursively add `node` and all its transitive dependencies to `set`.
    fn add_deps(resolve: &Resolve, node: PackageId, set: &mut HashSet<PackageId>) {
        if !set.insert(node) {
            return;
        }
        debug!("ignoring any lock pointing directly at {}", node);
        for (dep, _) in resolve.deps_not_replaced(node) {
            add_deps(resolve, dep, set);
        }
    }
}

fn master_branch_git_source(id: PackageId, resolve: &Resolve) -> Option<PackageId> {
    if resolve.version() <= ResolveVersion::V2 {
        let source = id.source_id();
        if let Some(GitReference::DefaultBranch) = source.git_reference() {
            let new_source =
                SourceId::for_git(source.url(), GitReference::Branch("master".to_string()))
                    .unwrap()
                    .with_precise_from(source);
            return Some(id.with_source_id(new_source));
        }
    }
    None
}

/// Emits warnings of unused patches case by case.
///
/// This function does its best to provide more targeted and helpful
/// (such as showing close candidates that failed to match). However, that's
/// not terribly easy to do, so just show a general help message if we cannot.
fn emit_warnings_of_unused_patches(
    ws: &Workspace<'_>,
    resolve: &Resolve,
    registry: &PackageRegistry<'_>,
) -> CargoResult<()> {
    const MESSAGE: &str = "was not used in the crate graph";

    // Patch package with the source URLs being patch
    let mut patch_pkgid_to_urls = HashMap::new();
    for (url, summaries) in registry.patches().iter() {
        for summary in summaries.iter() {
            patch_pkgid_to_urls
                .entry(summary.package_id())
                .or_insert_with(HashSet::new)
                .insert(url);
        }
    }

    // pkg name -> all source IDs of under the same pkg name
    let mut source_ids_grouped_by_pkg_name = HashMap::new();
    for pkgid in resolve.iter() {
        source_ids_grouped_by_pkg_name
            .entry(pkgid.name())
            .or_insert_with(HashSet::new)
            .insert(pkgid.source_id());
    }

    let mut unemitted_unused_patches = Vec::new();
    for unused in resolve.unused_patches().iter() {
        // Show alternative source URLs if the source URLs being patched
        // cannot be found in the crate graph.
        match (
            source_ids_grouped_by_pkg_name.get(&unused.name()),
            patch_pkgid_to_urls.get(unused),
        ) {
            (Some(ids), Some(patched_urls))
                if ids
                    .iter()
                    .all(|id| !patched_urls.contains(id.canonical_url())) =>
            {
                let mut help = "perhaps you meant one of the following:".to_owned();
                for id in ids {
                    help.push_str("\n\t");
                    help.push_str(&id.display_registry_name());
                }
                ws.gctx().shell().print_report(
                    &[Level::WARNING
                        .secondary_title(format!("patch `{unused}` {MESSAGE}"))
                        .element(Level::HELP.message(help))],
                    false,
                )?;
            }
            _ => unemitted_unused_patches.push(unused),
        }
    }

    // Show general help message.
    if !unemitted_unused_patches.is_empty() {
        let mut warnings: Vec<_> = unemitted_unused_patches
            .iter()
            .map(|pkgid| {
                Group::with_title(
                    Level::WARNING.secondary_title(format!("patch `{pkgid}` {MESSAGE}")),
                )
            })
            .collect();
        warnings.push(Group::with_title(
            Level::HELP.secondary_title(UNUSED_PATCH_WARNING),
        ));
        ws.gctx().shell().print_report(&warnings, false)?;
    }

    return Ok(());
}

/// Informs `registry` and `version_pref` that `[patch]` entries are available
/// and preferable for the dependency resolution.
///
/// This returns a set of PackageIds of `[patch]` entries, and some related
/// locked PackageIds, for which locking should be avoided (but which will be
/// preferred when searching dependencies, via [`VersionPreferences::prefer_patch_deps`]).
#[tracing::instrument(level = "debug", skip_all, ret)]
fn register_patch_entries(
    registry: &mut PackageRegistry<'_>,
    ws: &Workspace<'_>,
    previous: Option<&Resolve>,
    version_prefs: &mut VersionPreferences,
    keep_previous: Keep<'_>,
) -> CargoResult<HashSet<PackageId>> {
    let mut avoid_patch_ids = HashSet::new();
    for (url, patches) in ws.root_patch()?.iter() {
        for patch in patches {
            version_prefs.prefer_dependency(patch.clone());
        }
        let Some(previous) = previous else {
            let patches: Vec<_> = patches.iter().map(|p| (p, None)).collect();
            let unlock_ids = registry.patch(url, &patches)?;
            // Since nothing is locked, this shouldn't possibly return anything.
            assert!(unlock_ids.is_empty());
            continue;
        };

        // This is a list of pairs where the first element of the pair is
        // the raw `Dependency` which matches what's listed in `Cargo.toml`.
        // The second element is, if present, the "locked" version of
        // the `Dependency` as well as the `PackageId` that it previously
        // resolved to. This second element is calculated by looking at the
        // previous resolve graph, which is primarily what's done here to
        // build the `registrations` list.
        let mut registrations = Vec::new();
        for dep in patches {
            let candidates = || {
                previous
                    .iter()
                    .chain(previous.unused_patches().iter().cloned())
                    .filter(&keep_previous)
            };

            let lock = match candidates().find(|id| dep.matches_id(*id)) {
                // If we found an exactly matching candidate in our list of
                // candidates, then that's the one to use.
                Some(package_id) => {
                    let mut locked_dep = dep.clone();
                    locked_dep.lock_to(package_id);
                    Some(LockedPatchDependency {
                        dependency: locked_dep,
                        package_id,
                        alt_package_id: None,
                    })
                }
                None => {
                    // If the candidate does not have a matching source id
                    // then we may still have a lock candidate. If we're
                    // loading a v2-encoded resolve graph and `dep` is a
                    // git dep with `branch = 'master'`, then this should
                    // also match candidates without `branch = 'master'`
                    // (which is now treated separately in Cargo).
                    //
                    // In this scenario we try to convert candidates located
                    // in the resolve graph to explicitly having the
                    // `master` branch (if they otherwise point to
                    // `DefaultBranch`). If this works and our `dep`
                    // matches that then this is something we'll lock to.
                    match candidates().find(|&id| match master_branch_git_source(id, previous) {
                        Some(id) => dep.matches_id(id),
                        None => false,
                    }) {
                        Some(id_using_default) => {
                            let id_using_master = id_using_default.with_source_id(
                                dep.source_id()
                                    .with_precise_from(id_using_default.source_id()),
                            );

                            let mut locked_dep = dep.clone();
                            locked_dep.lock_to(id_using_master);
                            Some(LockedPatchDependency {
                                dependency: locked_dep,
                                package_id: id_using_master,
                                // Note that this is where the magic
                                // happens, where the resolve graph
                                // probably has locks pointing to
                                // DefaultBranch sources, and by including
                                // this here those will get transparently
                                // rewritten to Branch("master") which we
                                // have a lock entry for.
                                alt_package_id: Some(id_using_default),
                            })
                        }

                        // No locked candidate was found
                        None => None,
                    }
                }
            };

            registrations.push((dep, lock));
        }

        let canonical = CanonicalUrl::new(url)?;
        for (orig_patch, unlock_id) in registry.patch(url, &registrations)? {
            // Avoid the locked patch ID.
            avoid_patch_ids.insert(unlock_id);
            // Also avoid the thing it is patching.
            avoid_patch_ids.extend(previous.iter().filter(|id| {
                orig_patch.matches_ignoring_source(*id)
                    && *id.source_id().canonical_url() == canonical
            }));
        }
    }

    Ok(avoid_patch_ids)
}

/// Locks each `[replace]` entry to a specific Package ID
/// if the lockfile contains any corresponding previous replacement.
fn lock_replacements(
    ws: &Workspace<'_>,
    previous: Option<&Resolve>,
    keep: Keep<'_>,
) -> Vec<(PackageIdSpec, Dependency)> {
    let root_replace = ws.root_replace();
    let replace = match previous {
        Some(r) => root_replace
            .iter()
            .map(|(spec, dep)| {
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
    replace
}
