use std::collections::{HashMap, HashSet};

use crate::core::PackageSet;
use crate::core::{Dependency, PackageId, Source, SourceId, SourceMap, Summary};
use crate::sources::config::SourceConfigMap;
use crate::util::errors::CargoResult;
use crate::util::interning::InternedString;
use crate::util::{profile, CanonicalUrl, Config, VersionReqExt};
use anyhow::{bail, Context as _};
use log::{debug, trace};
use semver::VersionReq;
use url::Url;

/// Source of information about a group of packages.
///
/// See also `core::Source`.
pub trait Registry {
    /// Attempt to find the packages that match a dependency request.
    fn query(
        &mut self,
        dep: &Dependency,
        f: &mut dyn FnMut(Summary),
        fuzzy: bool,
    ) -> CargoResult<()>;

    fn query_vec(&mut self, dep: &Dependency, fuzzy: bool) -> CargoResult<Vec<Summary>> {
        let mut ret = Vec::new();
        self.query(dep, &mut |s| ret.push(s), fuzzy)?;
        Ok(ret)
    }

    fn describe_source(&self, source: SourceId) -> String;
    fn is_replaced(&self, source: SourceId) -> bool;
}

/// This structure represents a registry of known packages. It internally
/// contains a number of `Box<Source>` instances which are used to load a
/// `Package` from.
///
/// The resolution phase of Cargo uses this to drive knowledge about new
/// packages as well as querying for lists of new packages. It is here that
/// sources are updated (e.g., network operations) and overrides are
/// handled.
///
/// The general idea behind this registry is that it is centered around the
/// `SourceMap` structure, contained within which is a mapping of a `SourceId` to
/// a `Source`. Each `Source` in the map has been updated (using network
/// operations if necessary) and is ready to be queried for packages.
pub struct PackageRegistry<'cfg> {
    config: &'cfg Config,
    sources: SourceMap<'cfg>,

    // A list of sources which are considered "overrides" which take precedent
    // when querying for packages.
    overrides: Vec<SourceId>,

    // Note that each SourceId does not take into account its `precise` field
    // when hashing or testing for equality. When adding a new `SourceId`, we
    // want to avoid duplicates in the `SourceMap` (to prevent re-updating the
    // same git repo twice for example), but we also want to ensure that the
    // loaded source is always updated.
    //
    // Sources with a `precise` field normally don't need to be updated because
    // their contents are already on disk, but sources without a `precise` field
    // almost always need to be updated. If we have a cached `Source` for a
    // precise `SourceId`, then when we add a new `SourceId` that is not precise
    // we want to ensure that the underlying source is updated.
    //
    // This is basically a long-winded way of saying that we want to know
    // precisely what the keys of `sources` are, so this is a mapping of key to
    // what exactly the key is.
    source_ids: HashMap<SourceId, (SourceId, Kind)>,

    locked: LockedMap,
    yanked_whitelist: HashSet<PackageId>,
    source_config: SourceConfigMap<'cfg>,

    patches: HashMap<CanonicalUrl, Vec<Summary>>,
    patches_locked: bool,
    patches_available: HashMap<CanonicalUrl, Vec<PackageId>>,
}

/// A map of all "locked packages" which is filled in when parsing a lock file
/// and is used to guide dependency resolution by altering summaries as they're
/// queried from this source.
///
/// This map can be thought of as a glorified `Vec<MySummary>` where `MySummary`
/// has a `PackageId` for which package it represents as well as a list of
/// `PackageId` for the resolved dependencies. The hash map is otherwise
/// structured though for easy access throughout this registry.
type LockedMap = HashMap<
    // The first level of key-ing done in this hash map is the source that
    // dependencies come from, identified by a `SourceId`.
    // The next level is keyed by the name of the package...
    (SourceId, InternedString),
    // ... and the value here is a list of tuples. The first element of each
    // tuple is a package which has the source/name used to get to this
    // point. The second element of each tuple is the list of locked
    // dependencies that the first element has.
    Vec<(PackageId, Vec<PackageId>)>,
>;

#[derive(PartialEq, Eq, Clone, Copy)]
enum Kind {
    Override,
    Locked,
    Normal,
}

/// Argument to `PackageRegistry::patch` which is information about a `[patch]`
/// directive that we found in a lockfile, if present.
pub struct LockedPatchDependency {
    /// The original `Dependency` directive, except "locked" so it's version
    /// requirement is `=foo` and its `SourceId` has a "precise" listed.
    pub dependency: Dependency,
    /// The `PackageId` that was previously found in a lock file which
    /// `dependency` matches.
    pub package_id: PackageId,
    /// Something only used for backwards compatibility with the v2 lock file
    /// format where `branch=master` is considered the same as `DefaultBranch`.
    /// For more comments on this see the code in `ops/resolve.rs`.
    pub alt_package_id: Option<PackageId>,
}

impl<'cfg> PackageRegistry<'cfg> {
    pub fn new(config: &'cfg Config) -> CargoResult<PackageRegistry<'cfg>> {
        let source_config = SourceConfigMap::new(config)?;
        Ok(PackageRegistry {
            config,
            sources: SourceMap::new(),
            source_ids: HashMap::new(),
            overrides: Vec::new(),
            source_config,
            locked: HashMap::new(),
            yanked_whitelist: HashSet::new(),
            patches: HashMap::new(),
            patches_locked: false,
            patches_available: HashMap::new(),
        })
    }

    pub fn get(self, package_ids: &[PackageId]) -> CargoResult<PackageSet<'cfg>> {
        trace!("getting packages; sources={}", self.sources.len());
        PackageSet::new(package_ids, self.sources, self.config)
    }

    fn ensure_loaded(&mut self, namespace: SourceId, kind: Kind) -> CargoResult<()> {
        match self.source_ids.get(&namespace) {
            // We've previously loaded this source, and we've already locked it,
            // so we're not allowed to change it even if `namespace` has a
            // slightly different precise version listed.
            Some((_, Kind::Locked)) => {
                debug!("load/locked   {}", namespace);
                return Ok(());
            }

            // If the previous source was not a precise source, then we can be
            // sure that it's already been updated if we've already loaded it.
            Some((previous, _)) if previous.precise().is_none() => {
                debug!("load/precise  {}", namespace);
                return Ok(());
            }

            // If the previous source has the same precise version as we do,
            // then we're done, otherwise we need to need to move forward
            // updating this source.
            Some((previous, _)) => {
                if previous.precise() == namespace.precise() {
                    debug!("load/match    {}", namespace);
                    return Ok(());
                }
                debug!("load/mismatch {}", namespace);
            }
            None => {
                debug!("load/missing  {}", namespace);
            }
        }

        self.load(namespace, kind)?;
        Ok(())
    }

    pub fn add_sources(&mut self, ids: impl IntoIterator<Item = SourceId>) -> CargoResult<()> {
        for id in ids {
            self.ensure_loaded(id, Kind::Locked)?;
        }
        Ok(())
    }

    pub fn add_preloaded(&mut self, source: Box<dyn Source + 'cfg>) {
        self.add_source(source, Kind::Locked);
    }

    fn add_source(&mut self, source: Box<dyn Source + 'cfg>, kind: Kind) {
        let id = source.source_id();
        self.sources.insert(source);
        self.source_ids.insert(id, (id, kind));
    }

    pub fn add_override(&mut self, source: Box<dyn Source + 'cfg>) {
        self.overrides.push(source.source_id());
        self.add_source(source, Kind::Override);
    }

    pub fn add_to_yanked_whitelist(&mut self, iter: impl Iterator<Item = PackageId>) {
        let pkgs = iter.collect::<Vec<_>>();
        for (_, source) in self.sources.sources_mut() {
            source.add_to_yanked_whitelist(&pkgs);
        }
        self.yanked_whitelist.extend(pkgs);
    }

    /// remove all residual state from previous lock files.
    pub fn clear_lock(&mut self) {
        trace!("clear_lock");
        self.locked = HashMap::new();
    }

    pub fn register_lock(&mut self, id: PackageId, deps: Vec<PackageId>) {
        trace!("register_lock: {}", id);
        for dep in deps.iter() {
            trace!("\t-> {}", dep);
        }
        let sub_vec = self
            .locked
            .entry((id.source_id(), id.name()))
            .or_insert_with(Vec::new);
        sub_vec.push((id, deps));
    }

    /// Insert a `[patch]` section into this registry.
    ///
    /// This method will insert a `[patch]` section for the `url` specified,
    /// with the given list of dependencies. The `url` specified is the URL of
    /// the source to patch (for example this is `crates-io` in the manifest).
    /// The `deps` is an array of all the entries in the `[patch]` section of
    /// the manifest.
    ///
    /// Here the `deps` will be resolved to a precise version and stored
    /// internally for future calls to `query` below. `deps` should be a tuple
    /// where the first element is the patch definition straight from the
    /// manifest, and the second element is an optional variant where the
    /// patch has been locked. This locked patch is the patch locked to
    /// a specific version found in Cargo.lock. This will be `None` if
    /// `Cargo.lock` doesn't exist, or the patch did not match any existing
    /// entries in `Cargo.lock`.
    ///
    /// Note that the patch list specified here *will not* be available to
    /// `query` until `lock_patches` is called below, which should be called
    /// once all patches have been added.
    ///
    /// The return value is a `Vec` of patches that should *not* be locked.
    /// This happens when the patch is locked, but the patch has been updated
    /// so the locked value is no longer correct.
    pub fn patch(
        &mut self,
        url: &Url,
        deps: &[(&Dependency, Option<LockedPatchDependency>)],
    ) -> CargoResult<Vec<(Dependency, PackageId)>> {
        // NOTE: None of this code is aware of required features. If a patch
        // is missing a required feature, you end up with an "unused patch"
        // warning, which is very hard to understand. Ideally the warning
        // would be tailored to indicate *why* it is unused.
        let canonical = CanonicalUrl::new(url)?;

        // Return value of patches that shouldn't be locked.
        let mut unlock_patches = Vec::new();

        // First up we need to actually resolve each `deps` specification to
        // precisely one summary. We're not using the `query` method below as it
        // internally uses maps we're building up as part of this method
        // (`patches_available` and `patches). Instead we're going straight to
        // the source to load information from it.
        //
        // Remember that each dependency listed in `[patch]` has to resolve to
        // precisely one package, so that's why we're just creating a flat list
        // of summaries which should be the same length as `deps` above.
        let unlocked_summaries = deps
            .iter()
            .map(|(orig_patch, locked)| {
                // Remove double reference in orig_patch. Is there maybe a
                // magic pattern that could avoid this?
                let orig_patch = *orig_patch;
                // Use the locked patch if it exists, otherwise use the original.
                let dep = match locked {
                    Some(lock) => &lock.dependency,
                    None => orig_patch,
                };
                debug!(
                    "registering a patch for `{}` with `{}`",
                    url,
                    dep.package_name()
                );

                if dep.features().len() != 0 || !dep.uses_default_features() {
                    self.source_config.config().shell().warn(format!(
                        "patch for `{}` uses the features mechanism. \
                        default-features and features will not take effect because the patch dependency does not support this mechanism",
                        dep.package_name()
                    ))?;
                }

                // Go straight to the source for resolving `dep`. Load it as we
                // normally would and then ask it directly for the list of summaries
                // corresponding to this `dep`.
                self.ensure_loaded(dep.source_id(), Kind::Normal)
                    .with_context(|| {
                        format!(
                            "failed to load source for dependency `{}`",
                            dep.package_name()
                        )
                    })?;

                let source = self
                    .sources
                    .get_mut(dep.source_id())
                    .expect("loaded source not present");
                let summaries = source.query_vec(dep)?;
                let (summary, should_unlock) = summary_for_patch(
                    orig_patch, locked, summaries, source,
                )
                .with_context(|| {
                    format!(
                        "patch for `{}` in `{}` failed to resolve",
                        orig_patch.package_name(),
                        url,
                    )
                })?;
                debug!(
                    "patch summary is {:?} should_unlock={:?}",
                    summary, should_unlock
                );
                if let Some(unlock_id) = should_unlock {
                    unlock_patches.push((orig_patch.clone(), unlock_id));
                }

                if *summary.package_id().source_id().canonical_url() == canonical {
                    anyhow::bail!(
                        "patch for `{}` in `{}` points to the same source, but \
                         patches must point to different sources",
                        dep.package_name(),
                        url
                    );
                }
                Ok(summary)
            })
            .collect::<CargoResult<Vec<_>>>()
            .with_context(|| format!("failed to resolve patches for `{}`", url))?;

        let mut name_and_version = HashSet::new();
        for summary in unlocked_summaries.iter() {
            let name = summary.package_id().name();
            let version = summary.package_id().version();
            if !name_and_version.insert((name, version)) {
                bail!(
                    "cannot have two `[patch]` entries which both resolve \
                     to `{} v{}`",
                    name,
                    version
                );
            }
        }

        // Calculate a list of all patches available for this source which is
        // then used later during calls to `lock` to rewrite summaries to point
        // directly at these patched entries.
        //
        // Note that this is somewhat subtle where the list of `ids` for a
        // canonical URL is extend with possibly two ids per summary. This is done
        // to handle the transition from the v2->v3 lock file format where in
        // v2 DefeaultBranch was either DefaultBranch or Branch("master") for
        // git dependencies. In this case if `summary.package_id()` is
        // Branch("master") then alt_package_id will be DefaultBranch. This
        // signifies that there's a patch available for either of those
        // dependency directives if we see them in the dependency graph.
        //
        // This is a bit complicated and hopefully an edge case we can remove
        // in the future, but for now it hopefully doesn't cause too much
        // harm...
        let mut ids = Vec::new();
        for (summary, (_, lock)) in unlocked_summaries.iter().zip(deps) {
            ids.push(summary.package_id());
            if let Some(lock) = lock {
                ids.extend(lock.alt_package_id);
            }
        }
        self.patches_available.insert(canonical.clone(), ids);

        // Note that we do not use `lock` here to lock summaries! That step
        // happens later once `lock_patches` is invoked. In the meantime though
        // we want to fill in the `patches_available` map (later used in the
        // `lock` method) and otherwise store the unlocked summaries in
        // `patches` to get locked in a future call to `lock_patches`.
        self.patches.insert(canonical, unlocked_summaries);

        Ok(unlock_patches)
    }

    /// Lock all patch summaries added via `patch`, making them available to
    /// resolution via `query`.
    ///
    /// This function will internally `lock` each summary added via `patch`
    /// above now that the full set of `patch` packages are known. This'll allow
    /// us to correctly resolve overridden dependencies between patches
    /// hopefully!
    pub fn lock_patches(&mut self) {
        assert!(!self.patches_locked);
        for summaries in self.patches.values_mut() {
            for summary in summaries {
                debug!("locking patch {:?}", summary);
                *summary = lock(&self.locked, &self.patches_available, summary.clone());
            }
        }
        self.patches_locked = true;
    }

    pub fn patches(&self) -> Vec<Summary> {
        self.patches
            .values()
            .flat_map(|v| v.iter().cloned())
            .collect()
    }

    fn load(&mut self, source_id: SourceId, kind: Kind) -> CargoResult<()> {
        (|| {
            debug!("loading source {}", source_id);
            let source = self.source_config.load(source_id, &self.yanked_whitelist)?;
            assert_eq!(source.source_id(), source_id);

            if kind == Kind::Override {
                self.overrides.push(source_id);
            }
            self.add_source(source, kind);

            // Ensure the source has fetched all necessary remote data.
            let _p = profile::start(format!("updating: {}", source_id));
            self.sources.get_mut(source_id).unwrap().update()
        })()
        .with_context(|| format!("Unable to update {}", source_id))?;
        Ok(())
    }

    fn query_overrides(&mut self, dep: &Dependency) -> CargoResult<Option<Summary>> {
        for &s in self.overrides.iter() {
            let src = self.sources.get_mut(s).unwrap();
            let dep = Dependency::new_override(dep.package_name(), s);
            let mut results = src.query_vec(&dep)?;
            if !results.is_empty() {
                return Ok(Some(results.remove(0)));
            }
        }
        Ok(None)
    }

    /// This function is used to transform a summary to another locked summary
    /// if possible. This is where the concept of a lock file comes into play.
    ///
    /// If a summary points at a package ID which was previously locked, then we
    /// override the summary's ID itself, as well as all dependencies, to be
    /// rewritten to the locked versions. This will transform the summary's
    /// source to a precise source (listed in the locked version) as well as
    /// transforming all of the dependencies from range requirements on
    /// imprecise sources to exact requirements on precise sources.
    ///
    /// If a summary does not point at a package ID which was previously locked,
    /// or if any dependencies were added and don't have a previously listed
    /// version, we still want to avoid updating as many dependencies as
    /// possible to keep the graph stable. In this case we map all of the
    /// summary's dependencies to be rewritten to a locked version wherever
    /// possible. If we're unable to map a dependency though, we just pass it on
    /// through.
    pub fn lock(&self, summary: Summary) -> Summary {
        assert!(self.patches_locked);
        lock(&self.locked, &self.patches_available, summary)
    }

    fn warn_bad_override(
        &self,
        override_summary: &Summary,
        real_summary: &Summary,
    ) -> CargoResult<()> {
        let mut real_deps = real_summary.dependencies().iter().collect::<Vec<_>>();

        let boilerplate = "\
This is currently allowed but is known to produce buggy behavior with spurious
recompiles and changes to the crate graph. Path overrides unfortunately were
never intended to support this feature, so for now this message is just a
warning. In the future, however, this message will become a hard error.

To change the dependency graph via an override it's recommended to use the
`[patch]` feature of Cargo instead of the path override feature. This is
documented online at the url below for more information.

https://doc.rust-lang.org/cargo/reference/overriding-dependencies.html
";

        for dep in override_summary.dependencies() {
            if let Some(i) = real_deps.iter().position(|d| dep == *d) {
                real_deps.remove(i);
                continue;
            }
            let msg = format!(
                "path override for crate `{}` has altered the original list of\n\
                 dependencies; the dependency on `{}` was either added or\n\
                 modified to not match the previously resolved version\n\n\
                 {}",
                override_summary.package_id().name(),
                dep.package_name(),
                boilerplate
            );
            self.source_config.config().shell().warn(&msg)?;
            return Ok(());
        }

        if let Some(dep) = real_deps.get(0) {
            let msg = format!(
                "path override for crate `{}` has altered the original list of\n\
                 dependencies; the dependency on `{}` was removed\n\n\
                 {}",
                override_summary.package_id().name(),
                dep.package_name(),
                boilerplate
            );
            self.source_config.config().shell().warn(&msg)?;
            return Ok(());
        }

        Ok(())
    }
}

impl<'cfg> Registry for PackageRegistry<'cfg> {
    fn query(
        &mut self,
        dep: &Dependency,
        f: &mut dyn FnMut(Summary),
        fuzzy: bool,
    ) -> CargoResult<()> {
        assert!(self.patches_locked);
        let (override_summary, n, to_warn) = {
            // Look for an override and get ready to query the real source.
            let override_summary = self.query_overrides(dep)?;

            // Next up on our list of candidates is to check the `[patch]`
            // section of the manifest. Here we look through all patches
            // relevant to the source that `dep` points to, and then we match
            // name/version. Note that we don't use `dep.matches(..)` because
            // the patches, by definition, come from a different source.
            // This means that `dep.matches(..)` will always return false, when
            // what we really care about is the name/version match.
            let mut patches = Vec::<Summary>::new();
            if let Some(extra) = self.patches.get(dep.source_id().canonical_url()) {
                patches.extend(
                    extra
                        .iter()
                        .filter(|s| dep.matches_ignoring_source(s.package_id()))
                        .cloned(),
                );
            }

            // A crucial feature of the `[patch]` feature is that we *don't*
            // query the actual registry if we have a "locked" dependency. A
            // locked dep basically just means a version constraint of `=a.b.c`,
            // and because patches take priority over the actual source then if
            // we have a candidate we're done.
            if patches.len() == 1 && dep.is_locked() {
                let patch = patches.remove(0);
                match override_summary {
                    Some(summary) => (summary, 1, Some(patch)),
                    None => {
                        f(patch);
                        return Ok(());
                    }
                }
            } else {
                if !patches.is_empty() {
                    debug!(
                        "found {} patches with an unlocked dep on `{}` at {} \
                         with `{}`, \
                         looking at sources",
                        patches.len(),
                        dep.package_name(),
                        dep.source_id(),
                        dep.version_req()
                    );
                }

                // Ensure the requested source_id is loaded
                self.ensure_loaded(dep.source_id(), Kind::Normal)
                    .with_context(|| {
                        format!(
                            "failed to load source for dependency `{}`",
                            dep.package_name()
                        )
                    })?;

                let source = self.sources.get_mut(dep.source_id());
                match (override_summary, source) {
                    (Some(_), None) => anyhow::bail!("override found but no real ones"),
                    (None, None) => return Ok(()),

                    // If we don't have an override then we just ship
                    // everything upstairs after locking the summary
                    (None, Some(source)) => {
                        for patch in patches.iter() {
                            f(patch.clone());
                        }

                        // Our sources shouldn't ever come back to us with two
                        // summaries that have the same version. We could,
                        // however, have an `[patch]` section which is in use
                        // to override a version in the registry. This means
                        // that if our `summary` in this loop has the same
                        // version as something in `patches` that we've
                        // already selected, then we skip this `summary`.
                        let locked = &self.locked;
                        let all_patches = &self.patches_available;
                        let callback = &mut |summary: Summary| {
                            for patch in patches.iter() {
                                let patch = patch.package_id().version();
                                if summary.package_id().version() == patch {
                                    return;
                                }
                            }
                            f(lock(locked, all_patches, summary))
                        };
                        return if fuzzy {
                            source.fuzzy_query(dep, callback)
                        } else {
                            source.query(dep, callback)
                        };
                    }

                    // If we have an override summary then we query the source
                    // to sanity check its results. We don't actually use any of
                    // the summaries it gives us though.
                    (Some(override_summary), Some(source)) => {
                        if !patches.is_empty() {
                            anyhow::bail!("found patches and a path override")
                        }
                        let mut n = 0;
                        let mut to_warn = None;
                        {
                            let callback = &mut |summary| {
                                n += 1;
                                to_warn = Some(summary);
                            };
                            if fuzzy {
                                source.fuzzy_query(dep, callback)?;
                            } else {
                                source.query(dep, callback)?;
                            }
                        }
                        (override_summary, n, to_warn)
                    }
                }
            }
        };

        if n > 1 {
            anyhow::bail!("found an override with a non-locked list");
        } else if let Some(summary) = to_warn {
            self.warn_bad_override(&override_summary, &summary)?;
        }
        f(self.lock(override_summary));
        Ok(())
    }

    fn describe_source(&self, id: SourceId) -> String {
        match self.sources.get(id) {
            Some(src) => src.describe(),
            None => id.to_string(),
        }
    }

    fn is_replaced(&self, id: SourceId) -> bool {
        match self.sources.get(id) {
            Some(src) => src.is_replaced(),
            None => false,
        }
    }
}

fn lock(
    locked: &LockedMap,
    patches: &HashMap<CanonicalUrl, Vec<PackageId>>,
    summary: Summary,
) -> Summary {
    let pair = locked
        .get(&(summary.source_id(), summary.name()))
        .and_then(|vec| vec.iter().find(|&&(id, _)| id == summary.package_id()));

    trace!("locking summary of {}", summary.package_id());

    // Lock the summary's ID if possible
    let summary = match pair {
        Some((precise, _)) => summary.override_id(*precise),
        None => summary,
    };
    summary.map_dependencies(|dep| {
        trace!(
            "\t{}/{}/{}",
            dep.package_name(),
            dep.version_req(),
            dep.source_id()
        );

        // If we've got a known set of overrides for this summary, then
        // one of a few cases can arise:
        //
        // 1. We have a lock entry for this dependency from the same
        //    source as it's listed as coming from. In this case we make
        //    sure to lock to precisely the given package ID.
        //
        // 2. We have a lock entry for this dependency, but it's from a
        //    different source than what's listed, or the version
        //    requirement has changed. In this case we must discard the
        //    locked version because the dependency needs to be
        //    re-resolved.
        //
        // 3. We have a lock entry for this dependency, but it's from a
        //    different source than what's listed. This lock though happens
        //    through `[patch]`, so we want to preserve it.
        //
        // 4. We don't have a lock entry for this dependency, in which
        //    case it was likely an optional dependency which wasn't
        //    included previously so we just pass it through anyway.
        //
        // Cases 1/2 are handled by `matches_id`, case 3 is handled specially,
        // and case 4 is handled by falling through to the logic below.
        if let Some((_, locked_deps)) = pair {
            let locked = locked_deps.iter().find(|&&id| {
                // If the dependency matches the package id exactly then we've
                // found a match, this is the id the dependency was previously
                // locked to.
                if dep.matches_id(id) {
                    return true;
                }

                // If the name/version doesn't match, then we definitely don't
                // have a match whatsoever. Otherwise we need to check
                // `[patch]`...
                if !dep.matches_ignoring_source(id) {
                    return false;
                }

                // ... so here we look up the dependency url in the patches
                // map, and we see if `id` is contained in the list of patches
                // for that url. If it is then this lock is still valid,
                // otherwise the lock is no longer valid.
                match patches.get(dep.source_id().canonical_url()) {
                    Some(list) => list.contains(&id),
                    None => false,
                }
            });

            if let Some(&locked) = locked {
                trace!("\tfirst hit on {}", locked);
                let mut dep = dep;

                // If we found a locked version where the sources match, then
                // we can `lock_to` to get an exact lock on this dependency.
                // Otherwise we got a lock via `[patch]` so we only lock the
                // version requirement, not the source.
                if locked.source_id() == dep.source_id() {
                    dep.lock_to(locked);
                } else {
                    let req = VersionReq::exact(locked.version());
                    dep.set_version_req(req);
                }
                return dep;
            }
        }

        // If this dependency did not have a locked version, then we query
        // all known locked packages to see if they match this dependency.
        // If anything does then we lock it to that and move on.
        let v = locked
            .get(&(dep.source_id(), dep.package_name()))
            .and_then(|vec| vec.iter().find(|&&(id, _)| dep.matches_id(id)));
        if let Some(&(id, _)) = v {
            trace!("\tsecond hit on {}", id);
            let mut dep = dep;
            dep.lock_to(id);
            return dep;
        }

        trace!("\tnope, unlocked");
        dep
    })
}

/// This is a helper for selecting the summary, or generating a helpful error message.
fn summary_for_patch(
    orig_patch: &Dependency,
    locked: &Option<LockedPatchDependency>,
    mut summaries: Vec<Summary>,
    source: &mut dyn Source,
) -> CargoResult<(Summary, Option<PackageId>)> {
    if summaries.len() == 1 {
        return Ok((summaries.pop().unwrap(), None));
    }
    if summaries.len() > 1 {
        // TODO: In the future, it might be nice to add all of these
        // candidates so that version selection would just pick the
        // appropriate one. However, as this is currently structured, if we
        // added these all as patches, the unselected versions would end up in
        // the "unused patch" listing, and trigger a warning. It might take a
        // fair bit of restructuring to make that work cleanly, and there
        // isn't any demand at this time to support that.
        let mut vers: Vec<_> = summaries.iter().map(|summary| summary.version()).collect();
        vers.sort();
        let versions: Vec<_> = vers.into_iter().map(|v| v.to_string()).collect();
        anyhow::bail!(
            "patch for `{}` in `{}` resolved to more than one candidate\n\
            Found versions: {}\n\
            Update the patch definition to select only one package.\n\
            For example, add an `=` version requirement to the patch definition, \
            such as `version = \"={}\"`.",
            orig_patch.package_name(),
            orig_patch.source_id(),
            versions.join(", "),
            versions.last().unwrap()
        );
    }
    assert!(summaries.is_empty());
    // No summaries found, try to help the user figure out what is wrong.
    if let Some(locked) = locked {
        // Since the locked patch did not match anything, try the unlocked one.
        let orig_matches = source.query_vec(orig_patch).unwrap_or_else(|e| {
            log::warn!(
                "could not determine unlocked summaries for dep {:?}: {:?}",
                orig_patch,
                e
            );
            Vec::new()
        });
        let (summary, _) = summary_for_patch(orig_patch, &None, orig_matches, source)?;
        // The unlocked version found a match. This returns a value to
        // indicate that this entry should be unlocked.
        return Ok((summary, Some(locked.package_id)));
    }
    // Try checking if there are *any* packages that match this by name.
    let name_only_dep = Dependency::new_override(orig_patch.package_name(), orig_patch.source_id());
    let name_summaries = source.query_vec(&name_only_dep).unwrap_or_else(|e| {
        log::warn!(
            "failed to do name-only summary query for {:?}: {:?}",
            name_only_dep,
            e
        );
        Vec::new()
    });
    let mut vers = name_summaries
        .iter()
        .map(|summary| summary.version())
        .collect::<Vec<_>>();
    let found = match vers.len() {
        0 => format!(""),
        1 => format!("version `{}`", vers[0]),
        _ => {
            vers.sort();
            let strs: Vec<_> = vers.into_iter().map(|v| v.to_string()).collect();
            format!("versions `{}`", strs.join(", "))
        }
    };
    if found.is_empty() {
        anyhow::bail!(
            "The patch location `{}` does not appear to contain any packages \
            matching the name `{}`.",
            orig_patch.source_id(),
            orig_patch.package_name()
        );
    } else {
        anyhow::bail!(
            "The patch location `{}` contains a `{}` package with {}, but the patch \
            definition requires `{}`.\n\
            Check that the version in the patch location is what you expect, \
            and update the patch definition to match.",
            orig_patch.source_id(),
            orig_patch.package_name(),
            found,
            orig_patch.version_req()
        );
    }
}
