use std::cell::{Ref, RefCell, Cell};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash;
use std::mem;
use std::path::{Path, PathBuf};
use std::time::Duration;

use semver::Version;
use serde::ser;
use toml;
use lazycell::LazyCell;
use curl::easy::{Easy, HttpVersion};
use curl::multi::{Multi, EasyHandle};

use core::{Dependency, Manifest, PackageId, SourceId, Target};
use core::{FeatureMap, SourceMap, Summary};
use core::source::MaybePackage;
use core::interning::InternedString;
use ops;
use util::{internal, lev_distance, Config, Progress, ProgressStyle};
use util::errors::{CargoResult, CargoResultExt, HttpNot200};
use util::network::Retry;

/// Information about a package that is available somewhere in the file system.
///
/// A package is a `Cargo.toml` file plus all the files that are part of it.
// TODO: Is manifest_path a relic?
#[derive(Clone)]
pub struct Package {
    /// The package's manifest
    manifest: Manifest,
    /// The root of the package
    manifest_path: PathBuf,
}

/// A Package in a form where `Serialize` can be derived.
#[derive(Serialize)]
struct SerializedPackage<'a> {
    name: &'a str,
    version: &'a str,
    id: &'a PackageId,
    license: Option<&'a str>,
    license_file: Option<&'a str>,
    description: Option<&'a str>,
    source: &'a SourceId,
    dependencies: &'a [Dependency],
    targets: Vec<&'a Target>,
    features: &'a FeatureMap,
    manifest_path: &'a str,
    metadata: Option<&'a toml::Value>,
    authors: &'a [String],
    categories: &'a [String],
    keywords: &'a [String],
    readme: Option<&'a str>,
    repository: Option<&'a str>,
    edition: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    metabuild: Option<&'a Vec<String>>,
}

impl ser::Serialize for Package {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        let summary = self.manifest.summary();
        let package_id = summary.package_id();
        let manmeta = self.manifest.metadata();
        let license = manmeta.license.as_ref().map(String::as_ref);
        let license_file = manmeta.license_file.as_ref().map(String::as_ref);
        let description = manmeta.description.as_ref().map(String::as_ref);
        let authors = manmeta.authors.as_ref();
        let categories = manmeta.categories.as_ref();
        let keywords = manmeta.keywords.as_ref();
        let readme = manmeta.readme.as_ref().map(String::as_ref);
        let repository = manmeta.repository.as_ref().map(String::as_ref);
        // Filter out metabuild targets. They are an internal implementation
        // detail that is probably not relevant externally. There's also not a
        // real path to show in `src_path`, and this avoids changing the format.
        let targets: Vec<&Target> = self
            .manifest
            .targets()
            .iter()
            .filter(|t| t.src_path().is_path())
            .collect();

        SerializedPackage {
            name: &*package_id.name(),
            version: &package_id.version().to_string(),
            id: package_id,
            license,
            license_file,
            description,
            source: summary.source_id(),
            dependencies: summary.dependencies(),
            targets,
            features: summary.features(),
            manifest_path: &self.manifest_path.display().to_string(),
            metadata: self.manifest.custom_metadata(),
            authors,
            categories,
            keywords,
            readme,
            repository,
            edition: &self.manifest.edition().to_string(),
            metabuild: self.manifest.metabuild(),
        }.serialize(s)
    }
}

impl Package {
    /// Create a package from a manifest and its location
    pub fn new(manifest: Manifest, manifest_path: &Path) -> Package {
        Package {
            manifest,
            manifest_path: manifest_path.to_path_buf(),
        }
    }

    /// Get the manifest dependencies
    pub fn dependencies(&self) -> &[Dependency] {
        self.manifest.dependencies()
    }
    /// Get the manifest
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    /// Get the path to the manifest
    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }
    /// Get the name of the package
    pub fn name(&self) -> InternedString {
        self.package_id().name()
    }
    /// Get the PackageId object for the package (fully defines a package)
    pub fn package_id(&self) -> &PackageId {
        self.manifest.package_id()
    }
    /// Get the root folder of the package
    pub fn root(&self) -> &Path {
        self.manifest_path.parent().unwrap()
    }
    /// Get the summary for the package
    pub fn summary(&self) -> &Summary {
        self.manifest.summary()
    }
    /// Get the targets specified in the manifest
    pub fn targets(&self) -> &[Target] {
        self.manifest.targets()
    }
    /// Get the current package version
    pub fn version(&self) -> &Version {
        self.package_id().version()
    }
    /// Get the package authors
    pub fn authors(&self) -> &Vec<String> {
        &self.manifest.metadata().authors
    }
    /// Whether the package is set to publish
    pub fn publish(&self) -> &Option<Vec<String>> {
        self.manifest.publish()
    }

    /// Whether the package uses a custom build script for any target
    pub fn has_custom_build(&self) -> bool {
        self.targets().iter().any(|t| t.is_custom_build())
    }

    pub fn find_closest_target(
        &self,
        target: &str,
        is_expected_kind: fn(&Target) -> bool,
    ) -> Option<&Target> {
        let targets = self.targets();

        let matches = targets
            .iter()
            .filter(|t| is_expected_kind(t))
            .map(|t| (lev_distance(target, t.name()), t))
            .filter(|&(d, _)| d < 4);
        matches.min_by_key(|t| t.0).map(|t| t.1)
    }

    pub fn map_source(self, to_replace: &SourceId, replace_with: &SourceId) -> Package {
        Package {
            manifest: self.manifest.map_source(to_replace, replace_with),
            manifest_path: self.manifest_path,
        }
    }

    pub fn to_registry_toml(&self, config: &Config) -> CargoResult<String> {
        let manifest = self.manifest().original().prepare_for_publish(config)?;
        let toml = toml::to_string(&manifest)?;
        Ok(format!(
            "\
             # THIS FILE IS AUTOMATICALLY GENERATED BY CARGO\n\
             #\n\
             # When uploading crates to the registry Cargo will automatically\n\
             # \"normalize\" Cargo.toml files for maximal compatibility\n\
             # with all versions of Cargo and also rewrite `path` dependencies\n\
             # to registry (e.g. crates.io) dependencies\n\
             #\n\
             # If you believe there's an error in this file please file an\n\
             # issue against the rust-lang/cargo repository. If you're\n\
             # editing this file be aware that the upstream Cargo.toml\n\
             # will likely look very different (and much more reasonable)\n\
             \n\
             {}\
             ",
            toml
        ))
    }
}

impl fmt::Display for Package {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.summary().package_id())
    }
}

impl fmt::Debug for Package {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Package")
            .field("id", self.summary().package_id())
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

pub struct PackageSet<'cfg> {
    packages: HashMap<PackageId, LazyCell<Package>>,
    sources: RefCell<SourceMap<'cfg>>,
    config: &'cfg Config,
    multi: Multi,
    downloading: Cell<bool>,
}

pub struct Downloads<'a, 'cfg: 'a> {
    set: &'a PackageSet<'cfg>,
    pending: HashMap<usize, (Download, EasyHandle)>,
    pending_ids: HashSet<PackageId>,
    results: Vec<(usize, CargoResult<()>)>,
    next: usize,
    retry: Retry<'cfg>,
    progress: RefCell<Progress<'cfg>>,
    downloads_finished: usize,
    downloaded_bytes: usize,
}

struct Download {
    token: usize,
    id: PackageId,
    data: RefCell<Vec<u8>>,
    url: String,
    descriptor: String,
    total: Cell<usize>,
    current: Cell<usize>,
}

impl<'cfg> PackageSet<'cfg> {
    pub fn new(
        package_ids: &[PackageId],
        sources: SourceMap<'cfg>,
        config: &'cfg Config,
    ) -> CargoResult<PackageSet<'cfg>> {
        // We've enabled the `http2` feature of `curl` in Cargo, so treat
        // failures here as fatal as it would indicate a build-time problem.
        let mut multi = Multi::new();
        multi.pipelining(true, true)
            .chain_err(|| "failed to enable multiplexing/pipelining in curl")?;

        Ok(PackageSet {
            packages: package_ids
                .iter()
                .map(|id| (id.clone(), LazyCell::new()))
                .collect(),
            sources: RefCell::new(sources),
            config,
            multi,
            downloading: Cell::new(false),
        })
    }

    pub fn package_ids<'a>(&'a self) -> Box<Iterator<Item = &'a PackageId> + 'a> {
        Box::new(self.packages.keys())
    }

    pub fn enable_download<'a>(&'a self) -> CargoResult<Downloads<'a, 'cfg>> {
        assert!(!self.downloading.replace(true));
        Ok(Downloads {
            set: self,
            next: 0,
            pending: HashMap::new(),
            pending_ids: HashSet::new(),
            results: Vec::new(),
            retry: Retry::new(self.config)?,
            progress: RefCell::new(Progress::with_style(
                "Downloading",
                ProgressStyle::Ratio,
                self.config,
            )),
            downloads_finished: 0,
            downloaded_bytes: 0,
        })
    }
    pub fn get_one(&self, id: &PackageId) -> CargoResult<&Package> {
        let mut e = self.enable_download()?;
        match e.start(id)? {
            Some(s) => Ok(s),
            None => e.wait(),
        }
    }

    pub fn sources(&self) -> Ref<SourceMap<'cfg>> {
        self.sources.borrow()
    }
}

impl<'a, 'cfg> Downloads<'a, 'cfg> {
    /// Starts to download the package for the `id` specified.
    ///
    /// Returns `None` if the package is queued up for download and will
    /// eventually be returned from `wait_for_download`. Returns `Some(pkg)` if
    /// the package is ready and doesn't need to be downloaded.
    pub fn start(&mut self, id: &PackageId) -> CargoResult<Option<&'a Package>> {
        // First up see if we've already cached this package, in which case
        // there's nothing to do.
        let slot = self.set.packages
            .get(id)
            .ok_or_else(|| internal(format!("couldn't find `{}` in package set", id)))?;
        if let Some(pkg) = slot.borrow() {
            return Ok(Some(pkg));
        }

        // Ask the original source fo this `PackageId` for the corresponding
        // package. That may immediately come back and tell us that the package
        // is ready, or it could tell us that it needs to be downloaded.
        let mut sources = self.set.sources.borrow_mut();
        let source = sources
            .get_mut(id.source_id())
            .ok_or_else(|| internal(format!("couldn't find source for `{}`", id)))?;
        let pkg = source
            .download(id)
            .chain_err(|| format_err!("unable to get packages from source"))?;
        let (url, descriptor) = match pkg {
            MaybePackage::Ready(pkg) => {
                debug!("{} doesn't need a download", id);
                assert!(slot.fill(pkg).is_ok());
                return Ok(Some(slot.borrow().unwrap()))
            }
            MaybePackage::Download { url, descriptor } => (url, descriptor),
        };

        // Ok we're going to download this crate, so let's set up all our
        // internal state and hand off an `Easy` handle to our libcurl `Multi`
        // handle. This won't actually start the transfer, but later it'll
        // hapen during `wait_for_download`
        let token = self.next;
        self.next += 1;
        debug!("downloading {} as {}", id, token);
        assert!(self.pending_ids.insert(id.clone()));

        let mut handle = ops::http_handle(self.set.config)?;
        handle.get(true)?;
        handle.url(&url)?;
        handle.follow_location(true)?; // follow redirects

        // Enable HTTP/2 to be used as it'll allow true multiplexing which makes
        // downloads much faster. Currently Cargo requests the `http2` feature
        // of the `curl` crate which means it should always be built in, so
        // treat it as a fatal error of http/2 support isn't found.
        handle.http_version(HttpVersion::V2)
            .chain_err(|| "failed to enable HTTP2, is curl not built right?")?;

        // This is an option to `libcurl` which indicates that if there's a
        // bunch of parallel requests to the same host they all wait until the
        // pipelining status of the host is known. This means that we won't
        // initiate dozens of connections to crates.io, but rather only one.
        // Once the main one is opened we realized that pipelining is possible
        // and multiplexing is possible with static.crates.io. All in all this
        // reduces the number of connections done to a more manageable state.
        handle.pipewait(true)?;

        handle.write_function(move |buf| {
            debug!("{} - {} bytes of data", token, buf.len());
            tls::with(|downloads| {
                downloads.pending[&token].0.data.borrow_mut().extend_from_slice(buf);
            });
            Ok(buf.len())
        })?;

        handle.progress(true)?;
        handle.progress_function(move |dl_total, dl_cur, _, _| {
            tls::with(|downloads| {
                let dl = &downloads.pending[&token].0;
                dl.total.set(dl_total as usize);
                dl.current.set(dl_cur as usize);
                downloads.tick(false).is_ok()
            })
        })?;

        let dl = Download {
            token,
            data: RefCell::new(Vec::new()),
            id: id.clone(),
            url,
            descriptor,
            total: Cell::new(0),
            current: Cell::new(0),
        };
        self.enqueue(dl, handle)?;

        Ok(None)
    }

    /// Returns the number of crates that are still downloading
    pub fn remaining(&self) -> usize {
        self.pending.len()
    }

    /// Blocks the current thread waiting for a package to finish downloading.
    ///
    /// This method will wait for a previously enqueued package to finish
    /// downloading and return a reference to it after it's done downloading.
    ///
    /// # Panics
    ///
    /// This function will panic if there are no remaining downloads.
    pub fn wait(&mut self) -> CargoResult<&'a Package> {
        self.tick(true)?;

        let (dl, data) = loop {
            assert_eq!(self.pending.len(), self.pending_ids.len());
            let (token, result) = self.wait_for_curl()?;
            debug!("{} finished with {:?}", token, result);

            let (mut dl, handle) = self.pending.remove(&token)
                .expect("got a token for a non-in-progress transfer");
            let data = mem::replace(&mut *dl.data.borrow_mut(), Vec::new());
            let mut handle = self.set.multi.remove(handle)?;

            // Check if this was a spurious error. If it was a spurious error
            // then we want to re-enqueue our request for another attempt and
            // then we wait for another request to finish.
            let ret = {
                self.retry.try(|| {
                    result.chain_err(|| {
                        format!("failed to download from `{}`", dl.url)
                    })?;

                    let code = handle.response_code()?;
                    if code != 200 && code != 0 {
                        let url = handle.effective_url()?.unwrap_or(&dl.url);
                        return Err(HttpNot200 {
                            code,
                            url: url.to_string(),
                        }.into())
                    }
                    Ok(())
                })?
            };
            if ret.is_some() {
                break (dl, data)
            }
            self.enqueue(dl, handle)?;
        };
        self.pending_ids.remove(&dl.id);
        if !self.progress.borrow().is_enabled() {
            self.set.config.shell().status("Downloading", &dl.descriptor)?;
        }
        self.downloads_finished += 1;
        self.downloaded_bytes += dl.total.get();
        self.tick(true)?;

        // Inform the original source that the download is finished which
        // should allow us to actually get the package and fill it in now.
        let mut sources = self.set.sources.borrow_mut();
        let source = sources
            .get_mut(dl.id.source_id())
            .ok_or_else(|| internal(format!("couldn't find source for `{}`", dl.id)))?;
        let pkg = source.finish_download(&dl.id, data)?;
        let slot = &self.set.packages[&dl.id];
        assert!(slot.fill(pkg).is_ok());
        Ok(slot.borrow().unwrap())
    }

    fn enqueue(&mut self, dl: Download, handle: Easy) -> CargoResult<()> {
        let mut handle = self.set.multi.add(handle)?;
        handle.set_token(dl.token)?;
        self.pending.insert(dl.token, (dl, handle));
        Ok(())
    }

    fn wait_for_curl(&mut self) -> CargoResult<(usize, CargoResult<()>)> {
        // This is the main workhorse loop. We use libcurl's portable `wait`
        // method to actually perform blocking. This isn't necessarily too
        // efficient in terms of fd management, but we should only be juggling
        // a few anyway.
        //
        // Here we start off by asking the `multi` handle to do some work via
        // the `perform` method. This will actually do I/O work (nonblocking)
        // and attempt to make progress. Afterwards we ask about the `messages`
        // contained in the handle which will inform us if anything has finished
        // transferring.
        //
        // If we've got a finished transfer after all that work we break out
        // and process the finished transfer at the end. Otherwise we need to
        // actually block waiting for I/O to happen, which we achieve with the
        // `wait` method on `multi`.
        loop {
            let n = tls::set(self, || {
                self.set.multi.perform()
                    .chain_err(|| "failed to perform http requests")
            })?;
            debug!("handles remaining: {}", n);
            let results = &mut self.results;
            self.set.multi.messages(|msg| {
                let token = msg.token().expect("failed to read token");
                if let Some(result) = msg.result() {
                    results.push((token, result.map_err(|e| e.into())));
                } else {
                    debug!("message without a result (?)");
                }
            });

            if let Some(pair) = results.pop() {
                break Ok(pair)
            }
            assert!(self.pending.len() > 0);
            self.set.multi.wait(&mut [], Duration::new(60, 0))
                .chain_err(|| "failed to wait on curl `Multi`")?;
        }
    }

    fn tick(&self, now: bool) -> CargoResult<()> {
        self.progress.borrow_mut().tick(
            self.downloads_finished,
            self.downloads_finished + self.pending.len(),
        )
    }
}

impl<'a, 'cfg> Drop for Downloads<'a, 'cfg> {
    fn drop(&mut self) {
        self.set.downloading.set(false);
    }
}

mod tls {
    use std::cell::Cell;

    use super::Downloads;

    thread_local!(static PTR: Cell<usize> = Cell::new(0));

    pub(crate) fn with<R>(f: impl FnOnce(&Downloads) -> R) -> R {
        let ptr = PTR.with(|p| p.get());
        assert!(ptr != 0);
        unsafe {
            f(&*(ptr as *const Downloads))
        }
    }

    pub(crate) fn set<R>(dl: &Downloads, f: impl FnOnce() -> R) -> R {
        struct Reset<'a, T: Copy + 'a>(&'a Cell<T>, T);

        impl<'a, T: Copy> Drop for Reset<'a, T> {
            fn drop(&mut self) {
                self.0.set(self.1);
            }
        }

        PTR.with(|p| {
            let _reset = Reset(p, p.get());
            p.set(dl as *const Downloads as usize);
            f()
        })
    }
}
