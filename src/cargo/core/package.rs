use std::cell::OnceCell;
use std::cell::{Cell, Ref, RefCell};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt;
use std::hash;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use anyhow::Context as _;
use cargo_util_schemas::manifest::{Hints, RustVersion};
use futures::FutureExt;
use futures::TryStreamExt;
use futures::stream::FuturesUnordered;
use http::Request;
use semver::Version;
use serde::Serialize;
use tracing::debug;

use crate::core::compiler::{CompileKind, RustcTargetData};
use crate::core::dependency::DepKind;
use crate::core::resolver::features::ForceAllTargets;
use crate::core::resolver::{HasDevUnits, Resolve};
use crate::core::{
    CliUnstable, Dependency, Features, Manifest, PackageId, PackageIdSpec, SerializedDependency,
    SourceId, Target,
};
use crate::core::{Summary, Workspace};
use crate::sources::source::{MaybePackage, SourceMap};
use crate::util::HumanBytes;
use crate::util::cache_lock::{CacheLock, CacheLockMode};
use crate::util::errors::{CargoResult, HttpNotSuccessful};
use crate::util::interning::InternedString;
use crate::util::network::retry::{Retry, RetryResult};
use crate::util::{self, GlobalContext, Progress, ProgressStyle, internal};

/// Information about a package that is available somewhere in the file system.
///
/// A package is a `Cargo.toml` file plus all the files that are part of it.
#[derive(Clone)]
pub struct Package {
    inner: Rc<PackageInner>,
}

#[derive(Clone)]
// TODO: is `manifest_path` a relic?
struct PackageInner {
    /// The package's manifest.
    manifest: Manifest,
    /// The root of the package.
    manifest_path: PathBuf,
}

impl Ord for Package {
    fn cmp(&self, other: &Package) -> Ordering {
        self.package_id().cmp(&other.package_id())
    }
}

impl PartialOrd for Package {
    fn partial_cmp(&self, other: &Package) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// A Package in a form where `Serialize` can be derived.
#[derive(Serialize)]
pub struct SerializedPackage {
    name: InternedString,
    version: Version,
    id: PackageIdSpec,
    license: Option<String>,
    license_file: Option<String>,
    description: Option<String>,
    source: SourceId,
    dependencies: Vec<SerializedDependency>,
    targets: Vec<Target>,
    features: BTreeMap<InternedString, Vec<InternedString>>,
    manifest_path: PathBuf,
    metadata: Option<toml::Value>,
    publish: Option<Vec<String>>,
    authors: Vec<String>,
    categories: Vec<String>,
    keywords: Vec<String>,
    readme: Option<String>,
    repository: Option<String>,
    homepage: Option<String>,
    documentation: Option<String>,
    edition: String,
    links: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metabuild: Option<Vec<String>>,
    default_run: Option<String>,
    rust_version: Option<RustVersion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hints: Option<Hints>,
}

impl Package {
    /// Creates a package from a manifest and its location.
    pub fn new(manifest: Manifest, manifest_path: &Path) -> Package {
        Package {
            inner: Rc::new(PackageInner {
                manifest,
                manifest_path: manifest_path.to_path_buf(),
            }),
        }
    }

    /// Gets the manifest dependencies.
    pub fn dependencies(&self) -> &[Dependency] {
        self.manifest().dependencies()
    }
    /// Gets the manifest.
    pub fn manifest(&self) -> &Manifest {
        &self.inner.manifest
    }
    /// Gets the manifest.
    pub fn manifest_mut(&mut self) -> &mut Manifest {
        &mut Rc::make_mut(&mut self.inner).manifest
    }
    /// Gets the path to the manifest.
    pub fn manifest_path(&self) -> &Path {
        &self.inner.manifest_path
    }
    /// Gets the name of the package.
    pub fn name(&self) -> InternedString {
        self.package_id().name()
    }
    /// Gets the `PackageId` object for the package (fully defines a package).
    pub fn package_id(&self) -> PackageId {
        self.manifest().package_id()
    }
    /// Gets the root folder of the package.
    pub fn root(&self) -> &Path {
        self.manifest_path().parent().unwrap()
    }
    /// Gets the summary for the package.
    pub fn summary(&self) -> &Summary {
        self.manifest().summary()
    }
    /// Gets the targets specified in the manifest.
    pub fn targets(&self) -> &[Target] {
        self.manifest().targets()
    }
    /// Gets the library crate for this package, if it exists.
    pub fn library(&self) -> Option<&Target> {
        self.targets().iter().find(|t| t.is_lib())
    }
    /// Gets the current package version.
    pub fn version(&self) -> &Version {
        self.package_id().version()
    }
    /// Gets the package authors.
    pub fn authors(&self) -> &Vec<String> {
        &self.manifest().metadata().authors
    }

    /// Returns `None` if the package is set to publish.
    /// Returns `Some(allowed_registries)` if publishing is limited to specified
    /// registries or if package is set to not publish.
    pub fn publish(&self) -> &Option<Vec<String>> {
        self.manifest().publish()
    }
    /// Returns `true` if this package is a proc-macro.
    pub fn proc_macro(&self) -> bool {
        self.targets().iter().any(|target| target.proc_macro())
    }
    /// Gets the package's minimum Rust version.
    pub fn rust_version(&self) -> Option<&RustVersion> {
        self.manifest().rust_version()
    }

    /// Gets the package's hints.
    pub fn hints(&self) -> Option<&Hints> {
        self.manifest().hints()
    }

    /// Returns `true` if the package uses a custom build script for any target.
    pub fn has_custom_build(&self) -> bool {
        self.targets().iter().any(|t| t.is_custom_build())
    }

    pub fn map_source(self, to_replace: SourceId, replace_with: SourceId) -> Package {
        Package {
            inner: Rc::new(PackageInner {
                manifest: self.manifest().clone().map_source(to_replace, replace_with),
                manifest_path: self.manifest_path().to_owned(),
            }),
        }
    }

    pub fn serialized(
        &self,
        unstable_flags: &CliUnstable,
        cargo_features: &Features,
    ) -> SerializedPackage {
        let summary = self.manifest().summary();
        let package_id = summary.package_id();
        let manmeta = self.manifest().metadata();
        // Filter out metabuild targets. They are an internal implementation
        // detail that is probably not relevant externally. There's also not a
        // real path to show in `src_path`, and this avoids changing the format.
        let targets: Vec<Target> = self
            .manifest()
            .targets()
            .iter()
            .filter(|t| t.src_path().is_path())
            .cloned()
            .collect();
        // Convert Vec<FeatureValue> to Vec<InternedString>
        let crate_features = summary
            .features()
            .iter()
            .map(|(k, v)| (*k, v.iter().map(|fv| fv.to_string().into()).collect()))
            .collect();

        SerializedPackage {
            name: package_id.name(),
            version: package_id.version().clone(),
            id: package_id.to_spec(),
            license: manmeta.license.clone(),
            license_file: manmeta.license_file.clone(),
            description: manmeta.description.clone(),
            source: summary.source_id(),
            dependencies: summary
                .dependencies()
                .iter()
                .map(|dep| dep.serialized(unstable_flags, cargo_features))
                .collect(),
            targets,
            features: crate_features,
            manifest_path: self.manifest_path().to_path_buf(),
            metadata: self.manifest().custom_metadata().cloned(),
            authors: manmeta.authors.clone(),
            categories: manmeta.categories.clone(),
            keywords: manmeta.keywords.clone(),
            readme: manmeta.readme.clone(),
            repository: manmeta.repository.clone(),
            homepage: manmeta.homepage.clone(),
            documentation: manmeta.documentation.clone(),
            edition: self.manifest().edition().to_string(),
            links: self.manifest().links().map(|s| s.to_owned()),
            metabuild: self.manifest().metabuild().cloned(),
            publish: self.publish().as_ref().cloned(),
            default_run: self.manifest().default_run().map(|s| s.to_owned()),
            rust_version: self.rust_version().cloned(),
            hints: self.hints().cloned(),
        }
    }
}

impl fmt::Display for Package {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.summary().package_id())
    }
}

impl fmt::Debug for Package {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Package")
            .field("id", &self.summary().package_id())
            .field("..", &"..")
            .finish()
    }
}

impl PartialEq for Package {
    fn eq(&self, other: &Package) -> bool {
        self.package_id() == other.package_id()
    }
}

impl Eq for Package {}

impl hash::Hash for Package {
    fn hash<H: hash::Hasher>(&self, into: &mut H) {
        self.package_id().hash(into)
    }
}

/// A set of packages, with the intent to download.
///
/// This is primarily used to convert a set of `PackageId`s to `Package`s. It
/// will download as needed, or used the cached download if available.
pub struct PackageSet<'gctx> {
    packages: HashMap<PackageId, OnceCell<Package>>,
    sources: RefCell<SourceMap<'gctx>>,
    gctx: &'gctx GlobalContext,
}

/// Helper for downloading crates.
pub struct Downloads<'a, 'gctx> {
    set: &'a PackageSet<'gctx>,
    /// Progress bar.
    progress: RefCell<Progress<'gctx>>,
    /// Flag for keeping track of whether we've printed the Downloading message.
    first: Cell<bool>,
    /// Size (in bytes) and package name of the largest downloaded package.
    largest: Cell<Option<(u64, InternedString)>>,
    /// Number of downloads that have successfully finished.
    downloads_finished: Cell<u64>,
    /// Total bytes for all successfully downloaded packages.
    downloaded_bytes: Cell<u64>,
    /// Number of currently pending downloads.
    pending: Cell<u64>,
    /// Time when downloading started.
    start: Instant,
    /// Global filesystem lock to ensure only one Cargo is downloading one at a time.
    _lock: CacheLock<'gctx>,
}

impl<'a, 'gctx> Downloads<'a, 'gctx> {
    pub async fn download(
        set: &'a PackageSet<'gctx>,
        ids: impl IntoIterator<Item = PackageId>,
    ) -> CargoResult<Vec<&'a Package>> {
        let progress = RefCell::new(Progress::with_style(
            "Downloading",
            ProgressStyle::Ratio,
            set.gctx,
        ));
        let dl = Downloads {
            set,
            progress,
            first: Cell::new(true),
            largest: Cell::new(None),
            downloads_finished: Cell::new(0),
            downloaded_bytes: Cell::new(0),
            pending: Cell::new(0),
            start: Instant::now(),
            _lock: set
                .gctx
                .acquire_package_cache_lock(CacheLockMode::DownloadExclusive)?,
        };
        dl.run(ids).await
    }

    async fn run(&self, ids: impl IntoIterator<Item = PackageId>) -> CargoResult<Vec<&'a Package>> {
        let mut futures: FuturesUnordered<_> =
            ids.into_iter().map(|id| self.get_package(id)).collect();

        // Wait for downloads to complete, or the timer to expire.
        // This ensure that we call the tick function at a fast
        // enough rate to give the user progress updates.
        let mut out = Vec::new();
        loop {
            futures::select! {
                pkg = futures.try_next() => {
                    match pkg? {
                        Some(pkg) => out.push(pkg),
                        None => break,
                    }
                },
                _ = futures_timer::Delay::new(Duration::from_millis(200)).fuse() => {
                    self.tick(WhyTick::DownloadUpdate)?;
                },
            }
        }
        self.print_summary()?;
        self.set
            .gctx
            .deferred_global_last_use()?
            .save_no_error(self.set.gctx);
        Ok(out)
    }

    /// Get the existing package, or find the URL to download the .crate
    /// file and start the download.
    async fn get_package(&self, id: PackageId) -> CargoResult<&'a Package> {
        let slot = self
            .set
            .packages
            .get(&id)
            .ok_or_else(|| internal(format!("couldn't find `{}` in package set", id)))?;
        if let Some(pkg) = slot.get() {
            return CargoResult::Ok(pkg);
        }
        let source = self
            .set
            .sources
            .borrow()
            .get(id.source_id())
            .ok_or_else(|| internal(format!("couldn't find source for `{}`", id)))?
            .clone();
        let pkg = match source
            .download(id)
            .await
            .context("unable to get packages from source")
            .with_context(|| format!("failed to download `{}`", id))?
        {
            MaybePackage::Ready(package) => CargoResult::Ok(package),
            MaybePackage::Download {
                url,
                descriptor,
                authorization,
            } => {
                let mut r = Retry::new(self.set.gctx)?;
                let contents = loop {
                    self.tick(WhyTick::DownloadStarted)?;
                    self.pending.update(|v| v + 1);
                    let response = self
                        .fetch(&url, authorization.as_deref(), &descriptor, &id)
                        .await;
                    self.pending.update(|v| v - 1);
                    match r.r#try(|| response) {
                        RetryResult::Success(result) => break result,
                        RetryResult::Err(error) => {
                            debug!(target: "network", "final failure for {url}");
                            return Err(error);
                        }
                        RetryResult::Retry(delay_ms) => {
                            debug!(target: "network", "download retry {url} for {delay_ms}ms");
                            futures_timer::Delay::new(Duration::from_millis(delay_ms)).await;
                        }
                    }
                };
                self.downloads_finished.update(|v| v + 1);
                self.downloaded_bytes.update(|v| v + contents.len() as u64);

                // We're about to synchronously extract the crate below. While we're
                // doing that our download progress won't actually be updated, nor do we
                // have a great view into the progress of the extraction. Let's prepare
                // the user for this CPU-heavy step if it looks like it'll take some
                // time to do so.
                let kib_400 = 1024 * 400;
                if contents.len() < kib_400 {
                    self.tick(WhyTick::DownloadFinished)?;
                } else {
                    self.tick(WhyTick::Extracting(&id.name()))?;
                }

                Ok(source.finish_download(id, contents).await?)
            }
        }?;

        assert!(slot.set(pkg).is_ok());
        Ok(slot.get().unwrap())
    }

    /// Perform the request to download the .crate file.
    async fn fetch(
        &self,
        url: &str,
        authorization: Option<&str>,
        descriptor: &str,
        id: &PackageId,
    ) -> CargoResult<Vec<u8>> {
        // http::Uri doesn't support file urls without an authority, even though it's optional.
        // so we insert localhost here to make it work.
        let mut request = if let Some(file_url) = url.strip_prefix("file:///") {
            Request::get(format!("file://localhost/{file_url}"))
        } else {
            Request::get(url)
        };
        if let Some(authorization) = authorization {
            request = request.header(http::header::AUTHORIZATION, authorization);
        }
        let client = self
            .set
            .gctx
            .http_async()
            .with_context(|| format!("failed to download `{}`", id))?;

        // If the progress bar isn't enabled then it may be awhile before the
        // first crate finishes downloading so we inform immediately that we're
        // downloading crates here.
        if self.first.get() && !self.progress.borrow().is_enabled() {
            self.first.set(false);
            self.set.gctx.shell().status("Downloading", "crates ...")?;
        }

        let response = client
            .request(request.body(Vec::new())?)
            .await
            .with_context(|| format!("failed to download from `{}`", url))?;

        let previous_largest = self.largest.get().map(|(v, _)| v).unwrap_or_default();
        let len = response.body().len() as u64;
        if len > previous_largest {
            self.largest.set(Some((len, id.name())));
        }

        if response.status() != http::StatusCode::OK {
            return Err(HttpNotSuccessful::new_from_response(response, &url))
                .with_context(|| format!("failed to download from `{}`", url))?;
        }
        // If the progress bar isn't enabled then we still want to provide some
        // semblance of progress of how we're downloading crates, and if the
        // progress bar is enabled this provides a good log of what's happening.
        // progress.clear();
        self.set.gctx.shell().status("Downloaded", descriptor)?;

        Ok(response.into_body())
    }

    fn tick(&self, why: WhyTick<'_>) -> CargoResult<()> {
        let mut progress = self.progress.borrow_mut();

        if let WhyTick::DownloadUpdate = why {
            if !progress.update_allowed() {
                return Ok(());
            }
        }

        let pending = self.pending.get();
        let mut msg = if pending == 1 {
            format!("{} crate", pending)
        } else {
            format!("{} crates", pending)
        };
        match why {
            WhyTick::Extracting(krate) => {
                msg.push_str(&format!(", extracting {} ...", krate));
            }
            _ => {
                let remaining = self
                    .set
                    .gctx
                    .http_async()
                    .map(|c| c.bytes_pending())
                    .unwrap_or_default();
                if remaining > 0 {
                    msg.push_str(&format!(
                        ", remaining bytes: {:.1}",
                        HumanBytes(remaining as u64)
                    ));
                }
            }
        }
        progress.print_now(&msg)
    }

    fn print_summary(&self) -> CargoResult<()> {
        // Don't print a download summary if we're not using a progress bar,
        // we've already printed lots of `Downloading...` items.
        if !self.progress.borrow().is_enabled() {
            return Ok(());
        }
        let downloads_finished = self.downloads_finished.get();

        // If we didn't download anything, no need for a summary.
        if downloads_finished == 0 {
            return Ok(());
        }

        // pick the correct plural of crate(s)
        let crate_string = if downloads_finished == 1 {
            "crate"
        } else {
            "crates"
        };
        let mut status = format!(
            "{downloads_finished} {crate_string} ({:.1}) in {}",
            HumanBytes(self.downloaded_bytes.get()),
            util::elapsed(self.start.elapsed())
        );
        // print the size of largest crate if it was >1mb
        // however don't print if only a single crate was downloaded
        // because it is obvious that it will be the largest then
        if let Some(largest) = self.largest.get() {
            let mib_1 = 1024 * 1024;
            if largest.0 > mib_1 && downloads_finished > 1 {
                status.push_str(&format!(
                    " (largest was `{}` at {:.1})",
                    largest.1,
                    HumanBytes(largest.0),
                ));
            }
        }

        // Clear progress before displaying final summary.
        self.progress.borrow_mut().clear();
        self.set.gctx.shell().status("Downloaded", status)?;
        Ok(())
    }
}

impl<'gctx> PackageSet<'gctx> {
    pub fn new(
        package_ids: &[PackageId],
        sources: SourceMap<'gctx>,
        gctx: &'gctx GlobalContext,
    ) -> CargoResult<PackageSet<'gctx>> {
        gctx.http_config()?;

        Ok(PackageSet {
            packages: package_ids
                .iter()
                .map(|&id| (id, OnceCell::new()))
                .collect(),
            sources: RefCell::new(sources),
            gctx,
        })
    }

    pub fn package_ids(&self) -> impl Iterator<Item = PackageId> + '_ {
        self.packages.keys().cloned()
    }

    pub fn packages(&self) -> impl Iterator<Item = &Package> {
        self.packages.values().filter_map(|p| p.get())
    }

    pub fn get_one(&self, id: PackageId) -> CargoResult<&Package> {
        if let Some(pkg) = self.packages.get(&id).and_then(|slot| slot.get()) {
            return Ok(pkg);
        }
        Ok(self.get_many(Some(id))?.remove(0))
    }

    pub fn get_many(&self, ids: impl IntoIterator<Item = PackageId>) -> CargoResult<Vec<&Package>> {
        return crate::util::block_on(Downloads::download(self, ids));
    }

    /// Downloads any packages accessible from the give root ids.
    #[tracing::instrument(skip_all)]
    pub fn download_accessible(
        &self,
        resolve: &Resolve,
        root_ids: &[PackageId],
        has_dev_units: HasDevUnits,
        requested_kinds: &[CompileKind],
        target_data: &RustcTargetData<'gctx>,
        force_all_targets: ForceAllTargets,
    ) -> CargoResult<()> {
        fn collect_used_deps(
            used: &mut BTreeSet<(PackageId, CompileKind)>,
            resolve: &Resolve,
            pkg_id: PackageId,
            has_dev_units: HasDevUnits,
            requested_kind: CompileKind,
            target_data: &RustcTargetData<'_>,
            force_all_targets: ForceAllTargets,
        ) -> CargoResult<()> {
            if !used.insert((pkg_id, requested_kind)) {
                return Ok(());
            }
            let requested_kinds = &[requested_kind];
            let filtered_deps = PackageSet::filter_deps(
                pkg_id,
                resolve,
                has_dev_units,
                requested_kinds,
                target_data,
                force_all_targets,
            );
            for (pkg_id, deps) in filtered_deps {
                collect_used_deps(
                    used,
                    resolve,
                    pkg_id,
                    has_dev_units,
                    requested_kind,
                    target_data,
                    force_all_targets,
                )?;
                let artifact_kinds = deps.iter().filter_map(|dep| {
                    Some(
                        dep.artifact()?
                            .target()?
                            .to_resolved_compile_kind(*requested_kinds.iter().next().unwrap()),
                    )
                });
                for artifact_kind in artifact_kinds {
                    collect_used_deps(
                        used,
                        resolve,
                        pkg_id,
                        has_dev_units,
                        artifact_kind,
                        target_data,
                        force_all_targets,
                    )?;
                }
            }
            Ok(())
        }

        // This is sorted by PackageId to get consistent behavior and error
        // messages for Cargo's testsuite. Perhaps there is a better ordering
        // that optimizes download time?
        let mut to_download = BTreeSet::new();

        for id in root_ids {
            for requested_kind in requested_kinds {
                collect_used_deps(
                    &mut to_download,
                    resolve,
                    *id,
                    has_dev_units,
                    *requested_kind,
                    target_data,
                    force_all_targets,
                )?;
            }
        }
        let to_download = to_download
            .into_iter()
            .map(|(p, _)| p)
            .collect::<BTreeSet<_>>();
        self.get_many(to_download.into_iter())?;
        Ok(())
    }

    /// Check if there are any dependency packages that violate artifact constraints
    /// to instantly abort, or that do not have any libs which results in warnings.
    pub(crate) fn warn_no_lib_packages_and_artifact_libs_overlapping_deps(
        &self,
        ws: &Workspace<'gctx>,
        resolve: &Resolve,
        root_ids: &[PackageId],
        has_dev_units: HasDevUnits,
        requested_kinds: &[CompileKind],
        target_data: &RustcTargetData<'_>,
        force_all_targets: ForceAllTargets,
    ) -> CargoResult<()> {
        let no_lib_pkgs: BTreeMap<PackageId, Vec<(&Package, &HashSet<Dependency>)>> = root_ids
            .iter()
            .map(|&root_id| {
                let dep_pkgs_to_deps: Vec<_> = PackageSet::filter_deps(
                    root_id,
                    resolve,
                    has_dev_units,
                    requested_kinds,
                    target_data,
                    force_all_targets,
                )
                .collect();

                let dep_pkgs_and_deps = dep_pkgs_to_deps
                    .into_iter()
                    .filter(|(_id, deps)| deps.iter().any(|dep| dep.maybe_lib()))
                    .filter_map(|(dep_package_id, deps)| {
                        self.get_one(dep_package_id).ok().and_then(|dep_pkg| {
                            (!dep_pkg.targets().iter().any(|t| t.is_lib())).then(|| (dep_pkg, deps))
                        })
                    })
                    .collect();
                (root_id, dep_pkgs_and_deps)
            })
            .collect();

        for (pkg_id, dep_pkgs) in no_lib_pkgs {
            for (_dep_pkg_without_lib_target, deps) in dep_pkgs {
                for dep in deps.iter().filter(|dep| {
                    dep.artifact()
                        .map(|artifact| artifact.is_lib())
                        .unwrap_or(true)
                }) {
                    ws.gctx().shell().warn(&format!(
                        "{} ignoring invalid dependency `{}` which is missing a lib target",
                        pkg_id,
                        dep.name_in_toml(),
                    ))?;
                }
            }
        }
        Ok(())
    }

    pub fn filter_deps<'a>(
        pkg_id: PackageId,
        resolve: &'a Resolve,
        has_dev_units: HasDevUnits,
        requested_kinds: &'a [CompileKind],
        target_data: &'a RustcTargetData<'_>,
        force_all_targets: ForceAllTargets,
    ) -> impl Iterator<Item = (PackageId, &'a HashSet<Dependency>)> + 'a {
        resolve
            .deps(pkg_id)
            .filter(move |&(_id, deps)| {
                deps.iter().any(|dep| {
                    if dep.kind() == DepKind::Development && has_dev_units == HasDevUnits::No {
                        return false;
                    }
                    if force_all_targets == ForceAllTargets::No {
                        let activated = requested_kinds
                            .iter()
                            .chain(Some(&CompileKind::Host))
                            .any(|kind| target_data.dep_platform_activated(dep, *kind));
                        if !activated {
                            return false;
                        }
                    }
                    true
                })
            })
            .into_iter()
    }

    pub fn sources(&self) -> Ref<'_, SourceMap<'gctx>> {
        self.sources.borrow()
    }

    /// Merge the given set into self.
    pub fn add_set(&mut self, set: PackageSet<'gctx>) {
        for (pkg_id, p_cell) in set.packages {
            self.packages.entry(pkg_id).or_insert(p_cell);
        }
        let mut sources = self.sources.borrow_mut();
        let other_sources = set.sources.into_inner();
        sources.add_source_map(other_sources);
    }
}

#[derive(Copy, Clone)]
enum WhyTick<'a> {
    DownloadStarted,
    DownloadUpdate,
    DownloadFinished,
    Extracting(&'a str),
}
