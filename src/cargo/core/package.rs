use std::cell::{Cell, Ref, RefCell, RefMut};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt;
use std::hash;
use std::mem;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Context as _;
use bytesize::ByteSize;
use cargo_util_schemas::manifest::RustVersion;
use curl::easy::Easy;
use curl::multi::{EasyHandle, Multi};
use lazycell::LazyCell;
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
use crate::util::cache_lock::{CacheLock, CacheLockMode};
use crate::util::errors::{CargoResult, HttpNotSuccessful};
use crate::util::interning::InternedString;
use crate::util::network::http::http_handle_and_timeout;
use crate::util::network::http::HttpTimeout;
use crate::util::network::retry::{Retry, RetryResult};
use crate::util::network::sleep::SleepTracker;
use crate::util::{self, internal, GlobalContext, Progress, ProgressStyle};

/// Information about a package that is available somewhere in the file system.
///
/// A package is a `Cargo.toml` file plus all the files that are part of it.
#[derive(Clone)]
pub struct Package {
    inner: Arc<PackageInner>,
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
}

impl Package {
    /// Creates a package from a manifest and its location.
    pub fn new(manifest: Manifest, manifest_path: &Path) -> Package {
        Package {
            inner: Arc::new(PackageInner {
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
        &mut Arc::make_mut(&mut self.inner).manifest
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

    /// Returns `true` if the package uses a custom build script for any target.
    pub fn has_custom_build(&self) -> bool {
        self.targets().iter().any(|t| t.is_custom_build())
    }

    pub fn map_source(self, to_replace: SourceId, replace_with: SourceId) -> Package {
        Package {
            inner: Arc::new(PackageInner {
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
            .map(|(k, v)| {
                (
                    *k,
                    v.iter()
                        .map(|fv| InternedString::new(&fv.to_string()))
                        .collect(),
                )
            })
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
    packages: HashMap<PackageId, LazyCell<Package>>,
    sources: RefCell<SourceMap<'gctx>>,
    gctx: &'gctx GlobalContext,
    multi: Multi,
    /// Used to prevent reusing the `PackageSet` to download twice.
    downloading: Cell<bool>,
    /// Whether or not to use curl HTTP/2 multiplexing.
    multiplexing: bool,
}

/// Helper for downloading crates.
pub struct Downloads<'a, 'gctx> {
    set: &'a PackageSet<'gctx>,
    /// When a download is started, it is added to this map. The key is a
    /// "token" (see `Download::token`). It is removed once the download is
    /// finished.
    pending: HashMap<usize, (Download<'gctx>, EasyHandle)>,
    /// Set of packages currently being downloaded. This should stay in sync
    /// with `pending`.
    pending_ids: HashSet<PackageId>,
    /// Downloads that have failed and are waiting to retry again later.
    sleeping: SleepTracker<(Download<'gctx>, Easy)>,
    /// The final result of each download. A pair `(token, result)`. This is a
    /// temporary holding area, needed because curl can report multiple
    /// downloads at once, but the main loop (`wait`) is written to only
    /// handle one at a time.
    results: Vec<(usize, Result<(), curl::Error>)>,
    /// The next ID to use for creating a token (see `Download::token`).
    next: usize,
    /// Progress bar.
    progress: RefCell<Option<Progress<'gctx>>>,
    /// Number of downloads that have successfully finished.
    downloads_finished: usize,
    /// Total bytes for all successfully downloaded packages.
    downloaded_bytes: u64,
    /// Size (in bytes) and package name of the largest downloaded package.
    largest: (u64, InternedString),
    /// Time when downloading started.
    start: Instant,
    /// Indicates *all* downloads were successful.
    success: bool,

    /// Timeout management, both of timeout thresholds as well as whether or not
    /// our connection has timed out (and accompanying message if it has).
    ///
    /// Note that timeout management is done manually here instead of in libcurl
    /// because we want to apply timeouts to an entire batch of operations, not
    /// any one particular single operation.
    timeout: HttpTimeout,
    /// Last time bytes were received.
    updated_at: Cell<Instant>,
    /// This is a slow-speed check. It is reset to `now + timeout_duration`
    /// every time at least `threshold` bytes are received. If the current
    /// time ever exceeds `next_speed_check`, then give up and report a
    /// timeout error.
    next_speed_check: Cell<Instant>,
    /// This is the slow-speed threshold byte count. It starts at the
    /// configured threshold value (default 10), and is decremented by the
    /// number of bytes received in each chunk. If it is <= zero, the
    /// threshold has been met and data is being received fast enough not to
    /// trigger a timeout; reset `next_speed_check` and set this back to the
    /// configured threshold.
    next_speed_check_bytes_threshold: Cell<u64>,
    /// Global filesystem lock to ensure only one Cargo is downloading at a
    /// time.
    _lock: CacheLock<'gctx>,
}

struct Download<'gctx> {
    /// The token for this download, used as the key of the `Downloads::pending` map
    /// and stored in `EasyHandle` as well.
    token: usize,

    /// The package that we're downloading.
    id: PackageId,

    /// Actual downloaded data, updated throughout the lifetime of this download.
    data: RefCell<Vec<u8>>,

    /// HTTP headers for debugging.
    headers: RefCell<Vec<String>>,

    /// The URL that we're downloading from, cached here for error messages and
    /// reenqueuing.
    url: String,

    /// A descriptive string to print when we've finished downloading this crate.
    descriptor: String,

    /// Statistics updated from the progress callback in libcurl.
    total: Cell<u64>,
    current: Cell<u64>,

    /// The moment we started this transfer at.
    start: Instant,
    timed_out: Cell<Option<String>>,

    /// Logic used to track retrying this download if it's a spurious failure.
    retry: Retry<'gctx>,
}

impl<'gctx> PackageSet<'gctx> {
    pub fn new(
        package_ids: &[PackageId],
        sources: SourceMap<'gctx>,
        gctx: &'gctx GlobalContext,
    ) -> CargoResult<PackageSet<'gctx>> {
        // We've enabled the `http2` feature of `curl` in Cargo, so treat
        // failures here as fatal as it would indicate a build-time problem.
        let mut multi = Multi::new();
        let multiplexing = gctx.http_config()?.multiplexing.unwrap_or(true);
        multi
            .pipelining(false, multiplexing)
            .context("failed to enable multiplexing/pipelining in curl")?;

        // let's not flood crates.io with connections
        multi.set_max_host_connections(2)?;

        Ok(PackageSet {
            packages: package_ids
                .iter()
                .map(|&id| (id, LazyCell::new()))
                .collect(),
            sources: RefCell::new(sources),
            gctx,
            multi,
            downloading: Cell::new(false),
            multiplexing,
        })
    }

    pub fn package_ids(&self) -> impl Iterator<Item = PackageId> + '_ {
        self.packages.keys().cloned()
    }

    pub fn packages(&self) -> impl Iterator<Item = &Package> {
        self.packages.values().filter_map(|p| p.borrow())
    }

    pub fn enable_download<'a>(&'a self) -> CargoResult<Downloads<'a, 'gctx>> {
        assert!(!self.downloading.replace(true));
        let timeout = HttpTimeout::new(self.gctx)?;
        Ok(Downloads {
            start: Instant::now(),
            set: self,
            next: 0,
            pending: HashMap::new(),
            pending_ids: HashSet::new(),
            sleeping: SleepTracker::new(),
            results: Vec::new(),
            progress: RefCell::new(Some(Progress::with_style(
                "Downloading",
                ProgressStyle::Ratio,
                self.gctx,
            ))),
            downloads_finished: 0,
            downloaded_bytes: 0,
            largest: (0, InternedString::new("")),
            success: false,
            updated_at: Cell::new(Instant::now()),
            timeout,
            next_speed_check: Cell::new(Instant::now()),
            next_speed_check_bytes_threshold: Cell::new(0),
            _lock: self
                .gctx
                .acquire_package_cache_lock(CacheLockMode::DownloadExclusive)?,
        })
    }

    pub fn get_one(&self, id: PackageId) -> CargoResult<&Package> {
        if let Some(pkg) = self.packages.get(&id).and_then(|slot| slot.borrow()) {
            return Ok(pkg);
        }
        Ok(self.get_many(Some(id))?.remove(0))
    }

    pub fn get_many(&self, ids: impl IntoIterator<Item = PackageId>) -> CargoResult<Vec<&Package>> {
        let mut pkgs = Vec::new();
        let _lock = self
            .gctx
            .acquire_package_cache_lock(CacheLockMode::DownloadExclusive)?;
        let mut downloads = self.enable_download()?;
        for id in ids {
            pkgs.extend(downloads.start(id)?);
        }
        while downloads.remaining() > 0 {
            pkgs.push(downloads.wait()?);
        }
        downloads.success = true;
        drop(downloads);

        let mut deferred = self.gctx.deferred_global_last_use()?;
        deferred.save_no_error(self.gctx);
        Ok(pkgs)
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

    fn filter_deps<'a>(
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

    pub fn sources_mut(&self) -> RefMut<'_, SourceMap<'gctx>> {
        self.sources.borrow_mut()
    }

    /// Merge the given set into self.
    pub fn add_set(&mut self, set: PackageSet<'gctx>) {
        assert!(!self.downloading.get());
        assert!(!set.downloading.get());
        for (pkg_id, p_cell) in set.packages {
            self.packages.entry(pkg_id).or_insert(p_cell);
        }
        let mut sources = self.sources.borrow_mut();
        let other_sources = set.sources.into_inner();
        sources.add_source_map(other_sources);
    }
}

impl<'a, 'gctx> Downloads<'a, 'gctx> {
    /// Starts to download the package for the `id` specified.
    ///
    /// Returns `None` if the package is queued up for download and will
    /// eventually be returned from `wait_for_download`. Returns `Some(pkg)` if
    /// the package is ready and doesn't need to be downloaded.
    #[tracing::instrument(skip_all)]
    pub fn start(&mut self, id: PackageId) -> CargoResult<Option<&'a Package>> {
        self.start_inner(id)
            .with_context(|| format!("failed to download `{}`", id))
    }

    fn start_inner(&mut self, id: PackageId) -> CargoResult<Option<&'a Package>> {
        // First up see if we've already cached this package, in which case
        // there's nothing to do.
        let slot = self
            .set
            .packages
            .get(&id)
            .ok_or_else(|| internal(format!("couldn't find `{}` in package set", id)))?;
        if let Some(pkg) = slot.borrow() {
            return Ok(Some(pkg));
        }

        // Ask the original source for this `PackageId` for the corresponding
        // package. That may immediately come back and tell us that the package
        // is ready, or it could tell us that it needs to be downloaded.
        let mut sources = self.set.sources.borrow_mut();
        let source = sources
            .get_mut(id.source_id())
            .ok_or_else(|| internal(format!("couldn't find source for `{}`", id)))?;
        let pkg = source
            .download(id)
            .context("unable to get packages from source")?;
        let (url, descriptor, authorization) = match pkg {
            MaybePackage::Ready(pkg) => {
                debug!("{} doesn't need a download", id);
                assert!(slot.fill(pkg).is_ok());
                return Ok(Some(slot.borrow().unwrap()));
            }
            MaybePackage::Download {
                url,
                descriptor,
                authorization,
            } => (url, descriptor, authorization),
        };

        // Ok we're going to download this crate, so let's set up all our
        // internal state and hand off an `Easy` handle to our libcurl `Multi`
        // handle. This won't actually start the transfer, but later it'll
        // happen during `wait_for_download`
        let token = self.next;
        self.next += 1;
        debug!(target: "network", "downloading {} as {}", id, token);
        assert!(self.pending_ids.insert(id));

        let (mut handle, _timeout) = http_handle_and_timeout(self.set.gctx)?;
        handle.get(true)?;
        handle.url(&url)?;
        handle.follow_location(true)?; // follow redirects

        // Add authorization header.
        if let Some(authorization) = authorization {
            let mut headers = curl::easy::List::new();
            headers.append(&format!("Authorization: {}", authorization))?;
            handle.http_headers(headers)?;
        }

        // Enable HTTP/2 if possible.
        crate::try_old_curl_http2_pipewait!(self.set.multiplexing, handle);

        handle.write_function(move |buf| {
            debug!(target: "network", "{} - {} bytes of data", token, buf.len());
            tls::with(|downloads| {
                if let Some(downloads) = downloads {
                    downloads.pending[&token]
                        .0
                        .data
                        .borrow_mut()
                        .extend_from_slice(buf);
                }
            });
            Ok(buf.len())
        })?;
        handle.header_function(move |data| {
            tls::with(|downloads| {
                if let Some(downloads) = downloads {
                    // Headers contain trailing \r\n, trim them to make it easier
                    // to work with.
                    let h = String::from_utf8_lossy(data).trim().to_string();
                    downloads.pending[&token].0.headers.borrow_mut().push(h);
                }
            });
            true
        })?;

        handle.progress(true)?;
        handle.progress_function(move |dl_total, dl_cur, _, _| {
            tls::with(|downloads| match downloads {
                Some(d) => d.progress(token, dl_total as u64, dl_cur as u64),
                None => false,
            })
        })?;

        // If the progress bar isn't enabled then it may be awhile before the
        // first crate finishes downloading so we inform immediately that we're
        // downloading crates here.
        if self.downloads_finished == 0
            && self.pending.is_empty()
            && !self.progress.borrow().as_ref().unwrap().is_enabled()
        {
            self.set.gctx.shell().status("Downloading", "crates ...")?;
        }

        let dl = Download {
            token,
            data: RefCell::new(Vec::new()),
            headers: RefCell::new(Vec::new()),
            id,
            url,
            descriptor,
            total: Cell::new(0),
            current: Cell::new(0),
            start: Instant::now(),
            timed_out: Cell::new(None),
            retry: Retry::new(self.set.gctx)?,
        };
        self.enqueue(dl, handle)?;
        self.tick(WhyTick::DownloadStarted)?;

        Ok(None)
    }

    /// Returns the number of crates that are still downloading.
    pub fn remaining(&self) -> usize {
        self.pending.len() + self.sleeping.len()
    }

    /// Blocks the current thread waiting for a package to finish downloading.
    ///
    /// This method will wait for a previously enqueued package to finish
    /// downloading and return a reference to it after it's done downloading.
    ///
    /// # Panics
    ///
    /// This function will panic if there are no remaining downloads.
    #[tracing::instrument(skip_all)]
    pub fn wait(&mut self) -> CargoResult<&'a Package> {
        let (dl, data) = loop {
            assert_eq!(self.pending.len(), self.pending_ids.len());
            let (token, result) = self.wait_for_curl()?;
            debug!(target: "network", "{} finished with {:?}", token, result);

            let (mut dl, handle) = self
                .pending
                .remove(&token)
                .expect("got a token for a non-in-progress transfer");
            let data = mem::take(&mut *dl.data.borrow_mut());
            let headers = mem::take(&mut *dl.headers.borrow_mut());
            let mut handle = self.set.multi.remove(handle)?;
            self.pending_ids.remove(&dl.id);

            // Check if this was a spurious error. If it was a spurious error
            // then we want to re-enqueue our request for another attempt and
            // then we wait for another request to finish.
            let ret = {
                let timed_out = &dl.timed_out;
                let url = &dl.url;
                dl.retry.r#try(|| {
                    if let Err(e) = result {
                        // If this error is "aborted by callback" then that's
                        // probably because our progress callback aborted due to
                        // a timeout. We'll find out by looking at the
                        // `timed_out` field, looking for a descriptive message.
                        // If one is found we switch the error code (to ensure
                        // it's flagged as spurious) and then attach our extra
                        // information to the error.
                        if !e.is_aborted_by_callback() {
                            return Err(e.into());
                        }

                        return Err(match timed_out.replace(None) {
                            Some(msg) => {
                                let code = curl_sys::CURLE_OPERATION_TIMEDOUT;
                                let mut err = curl::Error::new(code);
                                err.set_extra(msg);
                                err
                            }
                            None => e,
                        }
                        .into());
                    }

                    let code = handle.response_code()?;
                    if code != 200 && code != 0 {
                        return Err(HttpNotSuccessful::new_from_handle(
                            &mut handle,
                            &url,
                            data,
                            headers,
                        )
                        .into());
                    }
                    Ok(data)
                })
            };
            match ret {
                RetryResult::Success(data) => break (dl, data),
                RetryResult::Err(e) => {
                    return Err(e.context(format!("failed to download from `{}`", dl.url)))
                }
                RetryResult::Retry(sleep) => {
                    debug!(target: "network", "download retry {} for {sleep}ms", dl.url);
                    self.sleeping.push(sleep, (dl, handle));
                }
            }
        };

        // If the progress bar isn't enabled then we still want to provide some
        // semblance of progress of how we're downloading crates, and if the
        // progress bar is enabled this provides a good log of what's happening.
        self.progress.borrow_mut().as_mut().unwrap().clear();
        self.set.gctx.shell().status("Downloaded", &dl.descriptor)?;

        self.downloads_finished += 1;
        self.downloaded_bytes += dl.total.get();
        if dl.total.get() > self.largest.0 {
            self.largest = (dl.total.get(), dl.id.name());
        }

        // We're about to synchronously extract the crate below. While we're
        // doing that our download progress won't actually be updated, nor do we
        // have a great view into the progress of the extraction. Let's prepare
        // the user for this CPU-heavy step if it looks like it'll take some
        // time to do so.
        if dl.total.get() < ByteSize::kb(400).0 {
            self.tick(WhyTick::DownloadFinished)?;
        } else {
            self.tick(WhyTick::Extracting(&dl.id.name()))?;
        }

        // Inform the original source that the download is finished which
        // should allow us to actually get the package and fill it in now.
        let mut sources = self.set.sources.borrow_mut();
        let source = sources
            .get_mut(dl.id.source_id())
            .ok_or_else(|| internal(format!("couldn't find source for `{}`", dl.id)))?;
        let start = Instant::now();
        let pkg = source.finish_download(dl.id, data)?;

        // Assume that no time has passed while we were calling
        // `finish_download`, update all speed checks and timeout limits of all
        // active downloads to make sure they don't fire because of a slowly
        // extracted tarball.
        let finish_dur = start.elapsed();
        self.updated_at.set(self.updated_at.get() + finish_dur);
        self.next_speed_check
            .set(self.next_speed_check.get() + finish_dur);

        let slot = &self.set.packages[&dl.id];
        assert!(slot.fill(pkg).is_ok());
        Ok(slot.borrow().unwrap())
    }

    fn enqueue(&mut self, dl: Download<'gctx>, handle: Easy) -> CargoResult<()> {
        let mut handle = self.set.multi.add(handle)?;
        let now = Instant::now();
        handle.set_token(dl.token)?;
        self.updated_at.set(now);
        self.next_speed_check.set(now + self.timeout.dur);
        self.next_speed_check_bytes_threshold
            .set(u64::from(self.timeout.low_speed_limit));
        dl.timed_out.set(None);
        dl.current.set(0);
        dl.total.set(0);
        self.pending.insert(dl.token, (dl, handle));
        Ok(())
    }

    /// Block, waiting for curl. Returns a token and a `Result` for that token
    /// (`Ok` means the download successfully finished).
    fn wait_for_curl(&mut self) -> CargoResult<(usize, Result<(), curl::Error>)> {
        // This is the main workhorse loop. We use libcurl's portable `wait`
        // method to actually perform blocking. This isn't necessarily too
        // efficient in terms of fd management, but we should only be juggling
        // a few anyway.
        //
        // Here we start off by asking the `multi` handle to do some work via
        // the `perform` method. This will actually do I/O work (non-blocking)
        // and attempt to make progress. Afterwards we ask about the `messages`
        // contained in the handle which will inform us if anything has finished
        // transferring.
        //
        // If we've got a finished transfer after all that work we break out
        // and process the finished transfer at the end. Otherwise we need to
        // actually block waiting for I/O to happen, which we achieve with the
        // `wait` method on `multi`.
        loop {
            self.add_sleepers()?;
            let n = tls::set(self, || {
                self.set
                    .multi
                    .perform()
                    .context("failed to perform http requests")
            })?;
            debug!(target: "network", "handles remaining: {}", n);
            let results = &mut self.results;
            let pending = &self.pending;
            self.set.multi.messages(|msg| {
                let token = msg.token().expect("failed to read token");
                let handle = &pending[&token].1;
                if let Some(result) = msg.result_for(handle) {
                    results.push((token, result));
                } else {
                    debug!(target: "network", "message without a result (?)");
                }
            });

            if let Some(pair) = results.pop() {
                break Ok(pair);
            }
            assert_ne!(self.remaining(), 0);
            if self.pending.is_empty() {
                let delay = self.sleeping.time_to_next().unwrap();
                debug!(target: "network", "sleeping main thread for {delay:?}");
                std::thread::sleep(delay);
            } else {
                let min_timeout = Duration::new(1, 0);
                let timeout = self.set.multi.get_timeout()?.unwrap_or(min_timeout);
                let timeout = timeout.min(min_timeout);
                self.set
                    .multi
                    .wait(&mut [], timeout)
                    .context("failed to wait on curl `Multi`")?;
            }
        }
    }

    fn add_sleepers(&mut self) -> CargoResult<()> {
        for (dl, handle) in self.sleeping.to_retry() {
            self.pending_ids.insert(dl.id);
            self.enqueue(dl, handle)?;
        }
        Ok(())
    }

    fn progress(&self, token: usize, total: u64, cur: u64) -> bool {
        let dl = &self.pending[&token].0;
        dl.total.set(total);
        let now = Instant::now();
        if cur > dl.current.get() {
            let delta = cur - dl.current.get();
            let threshold = self.next_speed_check_bytes_threshold.get();

            dl.current.set(cur);
            self.updated_at.set(now);

            if delta >= threshold {
                self.next_speed_check.set(now + self.timeout.dur);
                self.next_speed_check_bytes_threshold
                    .set(u64::from(self.timeout.low_speed_limit));
            } else {
                self.next_speed_check_bytes_threshold.set(threshold - delta);
            }
        }
        if self.tick(WhyTick::DownloadUpdate).is_err() {
            return false;
        }

        // If we've spent too long not actually receiving any data we time out.
        if now > self.updated_at.get() + self.timeout.dur {
            self.updated_at.set(now);
            let msg = format!(
                "failed to download any data for `{}` within {}s",
                dl.id,
                self.timeout.dur.as_secs()
            );
            dl.timed_out.set(Some(msg));
            return false;
        }

        // If we reached the point in time that we need to check our speed
        // limit, see if we've transferred enough data during this threshold. If
        // it fails this check then we fail because the download is going too
        // slowly.
        if now >= self.next_speed_check.get() {
            self.next_speed_check.set(now + self.timeout.dur);
            assert!(self.next_speed_check_bytes_threshold.get() > 0);
            let msg = format!(
                "download of `{}` failed to transfer more \
                 than {} bytes in {}s",
                dl.id,
                self.timeout.low_speed_limit,
                self.timeout.dur.as_secs()
            );
            dl.timed_out.set(Some(msg));
            return false;
        }

        true
    }

    fn tick(&self, why: WhyTick<'_>) -> CargoResult<()> {
        let mut progress = self.progress.borrow_mut();
        let progress = progress.as_mut().unwrap();

        if let WhyTick::DownloadUpdate = why {
            if !progress.update_allowed() {
                return Ok(());
            }
        }
        let pending = self.remaining();
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
                let mut dur = Duration::new(0, 0);
                let mut remaining = 0;
                for (dl, _) in self.pending.values() {
                    dur += dl.start.elapsed();
                    // If the total/current look weird just throw out the data
                    // point, sounds like curl has more to learn before we have
                    // the true information.
                    if dl.total.get() >= dl.current.get() {
                        remaining += dl.total.get() - dl.current.get();
                    }
                }
                if remaining > 0 && dur > Duration::from_millis(500) {
                    msg.push_str(&format!(", remaining bytes: {}", ByteSize(remaining)));
                }
            }
        }
        progress.print_now(&msg)
    }
}

#[derive(Copy, Clone)]
enum WhyTick<'a> {
    DownloadStarted,
    DownloadUpdate,
    DownloadFinished,
    Extracting(&'a str),
}

impl<'a, 'gctx> Drop for Downloads<'a, 'gctx> {
    fn drop(&mut self) {
        self.set.downloading.set(false);
        let progress = self.progress.get_mut().take().unwrap();
        // Don't print a download summary if we're not using a progress bar,
        // we've already printed lots of `Downloading...` items.
        if !progress.is_enabled() {
            return;
        }
        // If we didn't download anything, no need for a summary.
        if self.downloads_finished == 0 {
            return;
        }
        // If an error happened, let's not clutter up the output.
        if !self.success {
            return;
        }
        // pick the correct plural of crate(s)
        let crate_string = if self.downloads_finished == 1 {
            "crate"
        } else {
            "crates"
        };
        let mut status = format!(
            "{} {} ({}) in {}",
            self.downloads_finished,
            crate_string,
            ByteSize(self.downloaded_bytes),
            util::elapsed(self.start.elapsed())
        );
        // print the size of largest crate if it was >1mb
        // however don't print if only a single crate was downloaded
        // because it is obvious that it will be the largest then
        if self.largest.0 > ByteSize::mb(1).0 && self.downloads_finished > 1 {
            status.push_str(&format!(
                " (largest was `{}` at {})",
                self.largest.1,
                ByteSize(self.largest.0),
            ));
        }
        // Clear progress before displaying final summary.
        drop(progress);
        drop(self.set.gctx.shell().status("Downloaded", status));
    }
}

mod tls {
    use std::cell::Cell;

    use super::Downloads;

    thread_local!(static PTR: Cell<usize> = Cell::new(0));

    pub(crate) fn with<R>(f: impl FnOnce(Option<&Downloads<'_, '_>>) -> R) -> R {
        let ptr = PTR.with(|p| p.get());
        if ptr == 0 {
            f(None)
        } else {
            unsafe { f(Some(&*(ptr as *const Downloads<'_, '_>))) }
        }
    }

    pub(crate) fn set<R>(dl: &Downloads<'_, '_>, f: impl FnOnce() -> R) -> R {
        struct Reset<'a, T: Copy>(&'a Cell<T>, T);

        impl<'a, T: Copy> Drop for Reset<'a, T> {
            fn drop(&mut self) {
                self.0.set(self.1);
            }
        }

        PTR.with(|p| {
            let _reset = Reset(p, p.get());
            p.set(dl as *const Downloads<'_, '_> as usize);
            f()
        })
    }
}
