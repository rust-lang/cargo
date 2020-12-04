//! Access to a HTTP-based crate registry.
//!
//! See [`HttpRegistry`] for details.

use crate::core::{PackageId, SourceId};
use crate::ops;
use crate::sources::registry::make_dep_prefix;
use crate::sources::registry::MaybeLock;
use crate::sources::registry::{
    Fetched, RegistryConfig, RegistryData, CRATE_TEMPLATE, LOWER_PREFIX_TEMPLATE, PREFIX_TEMPLATE,
    VERSION_TEMPLATE,
};
use crate::util::errors::{CargoResult, CargoResultExt};
use crate::util::interning::InternedString;
use crate::util::paths;
use crate::util::{self, Config, Filesystem, Progress, ProgressStyle, Sha256};
use bytesize::ByteSize;
use curl::easy::{Easy, HttpVersion, List};
use curl::multi::{EasyHandle, Multi};
use log::{debug, trace};
use std::cell::{Cell, RefCell, RefMut};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as FmtWrite;
use std::fs::{self, File, OpenOptions};
use std::io::prelude::*;
use std::io::SeekFrom;
use std::path::{Path, PathBuf};
use std::str;
use std::time::Duration;
use std::time::Instant;

const ETAG: &'static [u8] = b"ETag";
const LAST_MODIFIED: &'static [u8] = b"Last-Modified";

/// A registry served by the HTTP-based registry API.
///
/// This type is primarily accessed through the [`RegistryData`] trait.
///
/// `HttpRegistry` implements the HTTP-based registry API outlined in [RFC XXX]. Read the RFC for
/// the complete protocol, but _roughly_ the implementation loads each index file (e.g.,
/// config.json or re/ge/regex) from an HTTP service rather than from a locally cloned git
/// repository. The remote service can more or less be a static file server that simply serves the
/// contents of the origin git repository.
///
/// Implemented naively, this leads to a significant amount of network traffic, as a lookup of any
/// index file would need to check with the remote backend if the index file has changed. This
/// cost is somewhat mitigated by the use of HTTP conditional feches (`If-Modified-Since` and
/// `If-None-Match` for `ETag`s) which can be efficiently handled by HTTP/2.
///
/// In order to take advantage of HTTP/2's ability to efficiently send multiple concurrent HTTP
/// requests over a single connection, `HttpRegistry` supports asynchronous prefetching. The caller
/// queues up a number of index files they think it is likely they will want to access, and
/// `HttpRegistry` fires off requests for each one without synchronously waiting for the response.
/// The caller then drives the processing of the responses, which update the index files that are
/// stored on disk, before moving on to the _actual_ dependency resolution. See
/// [`RegistryIndex::prefetch`] for more details.
///
/// [RFC XXX]: https://github.com/rust-lang/rfcs/pull/2789
pub struct HttpRegistry<'cfg> {
    index_path: Filesystem,
    cache_path: Filesystem,
    source_id: SourceId,
    config: &'cfg Config,

    /// Cached HTTP handle for synchronous requests (RegistryData::load).
    http: RefCell<Option<Easy>>,

    /// HTTP multi-handle for asynchronous/parallel requests during prefetching.
    prefetch: Multi,

    /// Has the client requested a cache update?
    ///
    /// Only if they have do we double-check the freshness of each locally-stored index file.
    requested_update: bool,

    /// State for currently pending prefetch downloads.
    downloads: Downloads<'cfg>,

    /// Does the config say that we can use HTTP multiplexing?
    multiplexing: bool,

    /// What paths have we already fetched since the last index update?
    ///
    /// We do not need to double-check any of these index files since we have already done so.
    fresh: HashSet<PathBuf>,

    /// If we are currently prefetching, all calls to RegistryData::load should go to disk.
    is_prefetching: bool,
}

// NOTE: the download bits are lifted from src/cargo/core/package.rs and tweaked

/// Helper for downloading crates.
pub struct Downloads<'cfg> {
    config: &'cfg Config,
    /// When a download is started, it is added to this map. The key is a
    /// "token" (see `Download::token`). It is removed once the download is
    /// finished.
    pending: HashMap<usize, (Download, EasyHandle)>,
    /// Set of paths currently being downloaded, mapped to their tokens.
    /// This should stay in sync with `pending`.
    pending_ids: HashMap<PathBuf, usize>,
    /// The final result of each download. A pair `(token, result)`. This is a
    /// temporary holding area, needed because curl can report multiple
    /// downloads at once, but the main loop (`wait`) is written to only
    /// handle one at a time.
    results: Vec<(usize, Result<(), curl::Error>)>,
    /// Prefetch requests that we already have a response to.
    /// NOTE: Should this maybe be some kind of heap?
    eager: BTreeMap<PathBuf, Fetched>,
    /// The next ID to use for creating a token (see `Download::token`).
    next: usize,
    /// Progress bar.
    progress: RefCell<Option<Progress<'cfg>>>,
    /// Number of downloads that have successfully finished.
    downloads_finished: usize,
    /// Total bytes for all successfully downloaded index files.
    downloaded_bytes: u64,
    /// Time when downloading started.
    start: Instant,
    /// Indicates *all* downloads were successful.
    success: bool,
}

struct Download {
    /// The token for this download, used as the key of the `Downloads::pending` map
    /// and stored in `EasyHandle` as well.
    token: usize,

    /// The path of the package that we're downloading.
    path: PathBuf,

    /// The name of the package that we're downloading.
    name: InternedString,

    /// The version requirements for the dependency line that triggered this fetch.
    // NOTE: we can get rid of the HashSet (and other complexity) if we had VersionReq::union
    reqs: HashSet<semver::VersionReq>,

    /// True if this download is of a direct dependency of the root crate.
    is_transitive: bool,

    /// Actual downloaded data, updated throughout the lifetime of this download.
    data: RefCell<Vec<u8>>,

    /// ETag and Last-Modified headers received from the server (if any).
    etag: RefCell<Option<String>>,
    last_modified: RefCell<Option<String>>,

    /// Statistics updated from the progress callback in libcurl.
    total: Cell<u64>,
    current: Cell<u64>,
}

impl<'cfg> HttpRegistry<'cfg> {
    pub fn new(source_id: SourceId, config: &'cfg Config, name: &str) -> HttpRegistry<'cfg> {
        HttpRegistry {
            index_path: config.registry_index_path().join(name),
            cache_path: config.registry_cache_path().join(name),
            source_id,
            config,
            http: RefCell::new(None),
            prefetch: Multi::new(),
            multiplexing: false,
            downloads: Downloads {
                start: Instant::now(),
                config,
                next: 0,
                pending: HashMap::new(),
                pending_ids: HashMap::new(),
                eager: BTreeMap::new(),
                results: Vec::new(),
                progress: RefCell::new(Some(Progress::with_style(
                    "Prefetching",
                    ProgressStyle::Ratio,
                    config,
                ))),
                downloads_finished: 0,
                downloaded_bytes: 0,
                success: false,
            },
            fresh: HashSet::new(),
            requested_update: false,
            is_prefetching: false,
        }
    }

    fn filename(&self, pkg: PackageId) -> String {
        format!("{}-{}.crate", pkg.name(), pkg.version())
    }

    fn http(&self) -> CargoResult<RefMut<'_, Easy>> {
        let handle = if let Ok(h) = self.http.try_borrow_mut() {
            h
        } else {
            anyhow::bail!("concurrent index downloads are not yet supported");
        };

        if handle.is_none() {
            assert!(self.config.offline());
            anyhow::bail!("can't access remote index: you are in offline mode (--offline)");
        } else {
            Ok(RefMut::map(handle, |opt| {
                opt.as_mut().expect("!handle.is_none() implies Some")
            }))
        }
    }

    fn handle_http_header(buf: &[u8]) -> Option<(&[u8], &str)> {
        if buf.is_empty() {
            return None;
        }

        let mut parts = buf.splitn(2, |&c| c == b':');
        let tag = parts.next().expect("first item of split is always Some");
        let rest = parts.next()?;
        let rest = std::str::from_utf8(rest).ok()?;
        let rest = rest.trim();

        // Don't let server sneak extra lines anywhere.
        if rest.contains('\n') {
            return None;
        }

        Some((tag, rest))
    }
}

const LAST_UPDATED_FILE: &str = ".last-updated";

impl<'cfg> RegistryData for HttpRegistry<'cfg> {
    fn prepare(&self) -> CargoResult<()> {
        if !self.config.offline() {
            let mut http = if let Ok(h) = self.http.try_borrow_mut() {
                h
            } else {
                anyhow::bail!("concurrent index downloads are not yet supported");
            };

            if http.is_none() {
                // NOTE: lifted from src/cargo/core/package.rs
                //
                // Ensure that we'll actually be able to acquire an HTTP handle later on
                // once we start trying to download crates. This will weed out any
                // problems with `.cargo/config` configuration related to HTTP.
                //
                // This way if there's a problem the error gets printed before we even
                // hit the index, which may not actually read this configuration.
                let mut handle = ops::http_handle(&self.config)?;
                handle.get(true)?;
                handle.follow_location(true)?;

                // NOTE: lifted from src/cargo/core/package.rs
                //
                // This is an option to `libcurl` which indicates that if there's a
                // bunch of parallel requests to the same host they all wait until the
                // pipelining status of the host is known. This means that we won't
                // initiate dozens of connections to crates.io, but rather only one.
                // Once the main one is opened we realized that pipelining is possible
                // and multiplexing is possible with static.crates.io. All in all this
                // reduces the number of connections done to a more manageable state.
                try_old_curl!(handle.pipewait(true), "pipewait");
                *http = Some(handle);
            }
        }

        Ok(())
    }

    fn start_prefetch(&mut self) -> CargoResult<bool> {
        // NOTE: lifted from src/cargo/core/package.rs
        //
        // We've enabled the `http2` feature of `curl` in Cargo, so treat
        // failures here as fatal as it would indicate a build-time problem.
        //
        // Note that the multiplexing support is pretty new so we're having it
        // off-by-default temporarily.
        //
        // Also note that pipelining is disabled as curl authors have indicated
        // that it's buggy, and we've empirically seen that it's buggy with HTTP
        // proxies.
        self.multiplexing = self.config.http_config()?.multiplexing.unwrap_or(true);

        self.prefetch
            .pipelining(false, self.multiplexing)
            .chain_err(|| "failed to enable multiplexing/pipelining in curl")?;

        // let's not flood crates.io with connections
        self.prefetch.set_max_host_connections(2)?;

        self.is_prefetching = true;
        Ok(true)
    }

    fn prefetch(
        &mut self,
        root: &Path,
        path: &Path,
        name: InternedString,
        req: Option<&semver::VersionReq>,
        is_transitive: bool,
    ) -> CargoResult<()> {
        // A quick overview of what goes on below:
        //
        // We first check if we have a local copy of the given index file.
        //
        // If we don't have a local copy of the index file, we obviously need to fetch it from the
        // server.
        //
        // If we do, we may need to check with the server if the index file has changed upstream.
        // This happens if cargo has explicitly requested that we fetch the _latest_ versions of
        // dependencies. We do this using a conditional HTTP request using the `Last-Modified` and
        // `ETag` headers we got when we fetched the currently cached index file (those headers are
        // stored in the first two lines of each index file). That way, if nothing has changed
        // (likely the common case), the server doesn't have to send us any data, just a 304 Not
        // Modified.

        let pkg = root.join(path);
        let bytes;
        // TODO: Can we avoid this file-system interaction if we're already downloading?
        let was = if pkg.exists() {
            if !self.requested_update || self.fresh.contains(path) {
                let req = if let Some(req) = req {
                    req
                } else {
                    // We don't need to fetch this file, and the caller does not care about it,
                    // so we can just return.
                    return Ok(());
                };

                trace!("not prefetching fresh {}", name);

                // We already have this file locally, and we don't need to double-check it with
                // upstream because the client hasn't requested an index update. So there's really
                // nothing to prefetch. We do keep track of the request though so that we will
                // eventually yield this back to the caller who may then want to prefetch other
                // transitive dependencies.
                use std::collections::btree_map::Entry;
                match self.downloads.eager.entry(path.to_path_buf()) {
                    Entry::Occupied(mut o) => {
                        o.get_mut().reqs.insert(req.clone());
                        // We trust a signal that something is _not_ transitive
                        // more than a signal that it is transitive.
                        o.get_mut().is_transitive &= is_transitive;
                    }
                    Entry::Vacant(v) => {
                        if self.fresh.contains(path) {
                            debug!("yielding already-prefetched {}", name);
                        }
                        let mut reqs = HashSet::new();
                        reqs.insert(req.clone());
                        v.insert(Fetched {
                            path: path.to_path_buf(),
                            name,
                            reqs,
                            is_transitive,
                        });
                    }
                }
                return Ok(());
            }

            // We have a local copy that we need to double-check the contents of.
            // First, extract the `Last-Modified` and `Etag` headers.
            trace!("prefetch load {} from disk", path.display());
            bytes = paths::read_bytes(&pkg)?;
            let mut lines = bytes.splitn(3, |&c| c == b'\n');
            let etag = lines.next().expect("splitn always returns >=1 item");
            let last_modified = if let Some(lm) = lines.next() {
                lm
            } else {
                anyhow::bail!("index file is missing HTTP header header");
            };
            let rest = if let Some(rest) = lines.next() {
                rest
            } else {
                anyhow::bail!("index file is missing HTTP header header");
            };

            assert!(!self.config.offline());

            let etag = std::str::from_utf8(etag)?;
            let last_modified = std::str::from_utf8(last_modified)?;
            Some((etag, last_modified, rest))
        } else {
            None
        };

        // If the path is already being fetched, don't fetch it again.
        // Just note down the version requirement and move on.
        if let Some(token) = self.downloads.pending_ids.get(path) {
            let (dl, _) = self
                .downloads
                .pending
                .get_mut(token)
                .expect("invalid token");

            trace!("amending dependency that we're already fetching: {}", name);
            if let Some(req) = req {
                trace!("adding req {}", req);
                dl.reqs.insert(req.clone());
            }
            dl.is_transitive &= is_transitive;

            return Ok(());
        } else if self.fresh.contains(path) {
            // This must have been a 404 when we initially prefetched it.
            return Ok(());
        } else if let Some(f) = self.downloads.eager.get_mut(path) {
            // We can't hit this case.
            // The index file must exist for the path to be in `eager`,
            // but since that's the case, we should have caught this
            // in the eager check _in_ the pkg.exists() path.
            unreachable!(
                "index file `{}` is in eager, but file doesn't exist",
                f.path.display()
            );
        }

        if was.is_some() {
            debug!("double-checking freshness of {}", path.display());
        }

        // Looks like we're going to have to bite the bullet and do a network request.
        let url = self.source_id.url();
        self.prepare()?;

        let mut handle = ops::http_handle(self.config)?;
        debug!("prefetch {}{}", url, path.display());
        handle.get(true)?;
        handle.url(&format!("{}{}", url, path.display()))?;
        handle.follow_location(true)?;

        // Enable HTTP/2 if possible.
        if self.multiplexing {
            try_old_curl!(handle.http_version(HttpVersion::V2), "HTTP2");
        } else {
            handle.http_version(HttpVersion::V11)?;
        }

        // This is an option to `libcurl` which indicates that if there's a
        // bunch of parallel requests to the same host they all wait until the
        // pipelining status of the host is known. This means that we won't
        // initiate dozens of connections to crates.io, but rather only one.
        // Once the main one is opened we realized that pipelining is possible
        // and multiplexing is possible with static.crates.io. All in all this
        // reduces the number of connections done to a more manageable state.
        try_old_curl!(handle.pipewait(true), "pipewait");

        // Make sure we don't send data back if it's the same as we have in the index.
        if let Some((ref etag, ref last_modified, _)) = was {
            let mut list = List::new();
            list.append(&format!("If-None-Match: {}", etag))?;
            list.append(&format!("If-Modified-Since: {}", last_modified))?;
            handle.http_headers(list)?;
        }

        // We're going to have a bunch of downloads all happening "at the same time".
        // So, we need some way to track what headers/data/responses are for which request.
        // We do that through this token. Each request (and associated response) gets one.
        let token = self.downloads.next;
        self.downloads.next += 1;
        debug!("downloading {} as {}", path.display(), token);
        assert_eq!(
            self.downloads.pending_ids.insert(path.to_path_buf(), token),
            None,
            "path queued for download more than once"
        );
        let mut reqs = HashSet::new();
        if let Some(req) = req {
            reqs.insert(req.clone());
        }

        // Each write should go to self.downloads.pending[&token].data.
        // Since the write function must be 'static, we access downloads through a thread-local.
        // That thread-local is set up in `next_prefetched` when it calls self.prefetch.perform,
        // which is what ultimately calls this method.
        handle.write_function(move |buf| {
            // trace!("{} - {} bytes of data", token, buf.len());
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

        // Same goes for the progress function -- it goes through thread-local storage.
        handle.progress(true)?;
        handle.progress_function(move |dl_total, dl_cur, _, _| {
            tls::with(|downloads| match downloads {
                Some(d) => d.progress(token, dl_total as u64, dl_cur as u64),
                None => false,
            })
        })?;

        // And ditto for the header function.
        handle.header_function(move |buf| {
            if let Some((tag, value)) = Self::handle_http_header(buf) {
                let is_etag = buf.eq_ignore_ascii_case(ETAG);
                let is_lm = buf.eq_ignore_ascii_case(LAST_MODIFIED);
                if is_etag || is_lm {
                    debug!(
                        "{} - got header {}: {}",
                        token,
                        std::str::from_utf8(tag)
                            .expect("both ETAG and LAST_MODIFIED are valid strs"),
                        value
                    );

                    // Append a new line to each so we can easily prepend to the index file.
                    let mut s = String::with_capacity(value.len() + 1);
                    s.push_str(value);
                    s.push('\n');
                    tls::with(|downloads| {
                        if let Some(downloads) = downloads {
                            let into = if is_etag {
                                &downloads.pending[&token].0.etag
                            } else {
                                &downloads.pending[&token].0.last_modified
                            };
                            *into.borrow_mut() = Some(s);
                        }
                    })
                }
            }

            true
        })?;

        // If the progress bar isn't enabled then it may be awhile before the
        // first index file finishes downloading so we inform immediately that
        // we're prefetching here.
        if self.downloads.downloads_finished == 0
            && self.downloads.pending.is_empty()
            && !self
                .downloads
                .progress
                .borrow()
                .as_ref()
                .unwrap()
                .is_enabled()
        {
            self.downloads
                .config
                .shell()
                .status("Prefetching", "index files ...")?;
        }

        let dl = Download {
            token,
            data: RefCell::new(Vec::new()),
            path: path.to_path_buf(),
            name,
            reqs,
            is_transitive,
            etag: RefCell::new(None),
            last_modified: RefCell::new(None),
            total: Cell::new(0),
            current: Cell::new(0),
        };

        // Finally add the request we've lined up to the pool of requests that cURL manages.
        let mut handle = self.prefetch.add(handle)?;
        handle.set_token(token)?;
        self.downloads.pending.insert(dl.token, (dl, handle));
        self.downloads.tick(WhyTick::DownloadStarted)?;

        Ok(())
    }

    fn next_prefetched(&mut self) -> CargoResult<Option<Fetched>> {
        while !self.downloads.pending.is_empty() || !self.downloads.eager.is_empty() {
            // We may already have packages that are ready to go. This takes care of grabbing the
            // next of those, while ensuring that we yield every distinct version requirement for
            // each package.
            //
            // TODO: Use the nightly BTreeMap::pop_first when stable.
            if let Some(path) = self.downloads.eager.keys().next().cloned() {
                let fetched = self.downloads.eager.remove(&path).unwrap();

                if fetched.reqs.is_empty() {
                    // This index file was proactively fetched even though it did not appear as a
                    // dependency, so we should not yield it back for future exploration.
                    trace!(
                        "not yielding fetch result for {} with no requirements",
                        fetched.name
                    );
                    continue;
                }
                trace!("yielding fetch result for {}", fetched.name);
                return Ok(Some(fetched));
            }

            // We don't have any fetched results immediately ready to be yielded,
            // so we need to check if curl has made any progress.
            assert_eq!(
                self.downloads.pending.len(),
                self.downloads.pending_ids.len()
            );
            // Note the `tls::set` here which sets up the thread-local storage needed to access
            // self.downloads from `write_function` and `header_function` above.
            let _remaining_in_multi = tls::set(&self.downloads, || {
                self.prefetch
                    .perform()
                    .chain_err(|| "failed to perform http requests")
            })?;
            // trace!("handles remaining: {}", _remaining_in_multi);

            // Walk all the messages cURL came across in case anything completed.
            let results = &mut self.downloads.results;
            let pending = &self.downloads.pending;
            self.prefetch.messages(|msg| {
                let token = msg.token().expect("failed to read token");
                let handle = &pending[&token].1;
                if let Some(result) = msg.result_for(handle) {
                    results.push((token, result));
                } else {
                    debug!("message without a result (?)");
                }
            });

            // Walk all the requests that completed and handle their responses.
            //
            // This will ultimately add more replies to self.downloads.eager, which we'll
            while let Some((token, result)) = self.downloads.results.pop() {
                trace!("{} finished with {:?}", token, result);

                let (dl, handle) = self
                    .downloads
                    .pending
                    .remove(&token)
                    .expect("got a token for a non-in-progress transfer");

                let data = dl.data.into_inner();
                let mut handle = self.prefetch.remove(handle)?;
                self.downloads.pending_ids.remove(&dl.path);

                let fetched = Fetched {
                    path: dl.path,
                    name: dl.name,
                    reqs: dl.reqs,
                    is_transitive: dl.is_transitive,
                };
                assert!(
                    self.fresh.insert(fetched.path.clone()),
                    "downloaded the index file `{}` twice during prefetching",
                    fetched.path.display(),
                );

                let code = handle.response_code()?;
                debug!(
                    "index file for {} downloaded with status code {}",
                    fetched.name,
                    handle.response_code()?
                );

                // This gets really noisy very quickly:
                // self.config.shell().status("Prefetched", &fetched.name)?;

                self.downloads.downloads_finished += 1;
                self.downloads.downloaded_bytes += dl.total.get();
                self.downloads.tick(WhyTick::DownloadFinished)?;

                match code {
                    200 => {
                        // We got data back, hooray!
                        // Let's update the index file.
                        let path = self.config.assert_package_cache_locked(&self.index_path);
                        let pkg = path.join(&fetched.path);
                        paths::create_dir_all(pkg.parent().expect("pkg is a file"))?;
                        let mut file = paths::create(pkg)?;
                        file.write_all(dl.etag.into_inner().as_deref().unwrap_or("\n").as_bytes())?;
                        file.write_all(
                            dl.last_modified
                                .into_inner()
                                .as_deref()
                                .unwrap_or("\n")
                                .as_bytes(),
                        )?;
                        file.write_all(&data)?;
                        file.flush()?;

                        assert!(
                            self.downloads
                                .eager
                                .insert(fetched.path.clone(), fetched)
                                .is_none(),
                            "download finished for already-finished path"
                        );
                    }
                    304 => {
                        // Not Modified response.
                        // There's nothing for us to do -- the index file is up to date.
                        // The only thing that matters is telling the caller about this package.
                        assert!(
                            self.downloads
                                .eager
                                .insert(fetched.path.clone(), fetched)
                                .is_none(),
                            "download finished for already-finished path"
                        );
                    }
                    403 | 404 => {
                        // Not Found response.
                        // We treat Forbidden as just being another expression for 404
                        // from a server that does not want to reveal file names.
                        // The crate doesn't exist, so we simply do not yield it.
                        // Errors will eventually be yielded by load().
                    }
                    410 | 451 => {
                        // The crate was deleted from the registry.
                        // Errors will eventually be yielded by load().
                        todo!("we should delete the local index file here if it exists");
                    }
                    code => {
                        anyhow::bail!(
                            "prefetch: server returned unexpected HTTP status code {} for {}{}",
                            code,
                            self.source_id.url(),
                            fetched.path.display()
                        );
                    }
                }
            }

            if !self.downloads.eager.is_empty() {
                continue;
            }

            if self.downloads.pending.is_empty() {
                // We're all done!
                break;
            }

            // We have no more replies to provide the caller with,
            // so we need to wait until cURL has something new for us.
            let timeout = self
                .prefetch
                .get_timeout()?
                .unwrap_or_else(|| Duration::new(5, 0));
            self.prefetch
                .wait(&mut [], timeout)
                .chain_err(|| "failed to wait on curl `Multi`")?;
        }

        debug!("prefetched all transitive dependencies");
        self.is_prefetching = false;
        Ok(None)
    }

    fn index_path(&self) -> &Filesystem {
        // NOTE: I'm pretty sure this method is unnecessary.
        // The only place it is used is to set `.path` in `RegistryIndex`,
        // which only uses it to call `assert_index_locked below`...
        &self.index_path
    }

    fn assert_index_locked<'a>(&self, path: &'a Filesystem) -> &'a Path {
        self.config.assert_package_cache_locked(path)
    }

    fn current_version(&self) -> Option<InternedString> {
        // TODO: Can we use the time of the last call to update_index here?
        None
    }

    fn update_index_file(&mut self, root: &Path, path: &Path) -> CargoResult<bool> {
        let pkg = root.join(path);
        if pkg.exists() {
            paths::remove_file(&pkg)?;
        }
        // Also reset self.fresh so we don't hit an assertion failure if we re-download.
        self.fresh.remove(path);
        Ok(true)
    }

    fn load(
        &mut self,
        root: &Path,
        path: &Path,
        data: &mut dyn FnMut(&[u8]) -> CargoResult<()>,
    ) -> CargoResult<()> {
        // NOTE: This is pretty much a synchronous version of the prefetch() + next_prefetched()
        // dance. Much of the code is sort-of duplicated, which isn't great, but it's moderalyte
        // straightforward and works. When the real resolver supports a load returning "not yet",
        // load and prefetch can be merged.

        let pkg = root.join(path);
        let bytes;
        let was = if pkg.exists() {
            // We have a local copy -- extract the `Last-Modified` and `Etag` headers.
            trace!("load {} from disk", path.display());

            bytes = paths::read_bytes(&pkg)?;
            let mut lines = bytes.splitn(3, |&c| c == b'\n');
            let etag = lines.next().expect("splitn always returns >=1 item");
            let last_modified = if let Some(lm) = lines.next() {
                lm
            } else {
                anyhow::bail!("index file is missing HTTP header header");
            };
            let rest = if let Some(rest) = lines.next() {
                rest
            } else {
                anyhow::bail!("index file is missing HTTP header header");
            };

            let is_fresh = if !self.requested_update {
                trace!(
                    "using local {} as user did not request update",
                    path.display()
                );
                true
            } else if self.config.offline() {
                trace!("using local {} in offline mode", path.display());
                true
            } else if self.is_prefetching {
                trace!("using local {} in load while prefetching", path.display());
                true
            } else if self.fresh.contains(path) {
                trace!(
                    "using local {} as it was already prefetched",
                    path.display()
                );
                true
            } else {
                debug!("double-checking freshness of {}", path.display());
                false
            };

            if is_fresh {
                return data(rest);
            } else {
                // We cannot trust the index files and need to double-check with server.
                let etag = std::str::from_utf8(etag)?;
                let last_modified = std::str::from_utf8(last_modified)?;
                Some((etag, last_modified, rest))
            }
        } else if self.fresh.contains(path) {
            // This must have been a 404.
            anyhow::bail!("crate does not exist in the registry");
        } else {
            assert!(!self.is_prefetching);
            None
        };

        let url = self.source_id.url();
        if self.config.offline() {
            anyhow::bail!(
                "can't download index file from '{}': you are in offline mode (--offline)",
                url
            );
        }

        self.prepare()?;
        let mut handle = self.http()?;
        debug!("fetch {}{}", url, path.display());
        handle.url(&format!("{}{}", url, path.display()))?;

        if let Some((ref etag, ref last_modified, _)) = was {
            let mut list = List::new();
            list.append(&format!("If-None-Match: {}", etag))?;
            list.append(&format!("If-Modified-Since: {}", last_modified))?;
            handle.http_headers(list)?;
        }

        let mut contents = Vec::new();
        let mut etag = None;
        let mut last_modified = None;
        let mut transfer = handle.transfer();
        transfer.write_function(|buf| {
            contents.extend_from_slice(buf);
            Ok(buf.len())
        })?;

        // Capture ETag and Last-Modified.
        transfer.header_function(|buf| {
            if let Some((tag, value)) = Self::handle_http_header(buf) {
                let is_etag = tag.eq_ignore_ascii_case(ETAG);
                let is_lm = tag.eq_ignore_ascii_case(LAST_MODIFIED);
                if is_etag || is_lm {
                    // Append a new line to each so we can easily prepend to the index file.
                    let mut s = String::with_capacity(value.len() + 1);
                    s.push_str(value);
                    s.push('\n');
                    if is_etag {
                        etag = Some(s);
                    } else if is_lm {
                        last_modified = Some(s);
                    }
                }
            }

            true
        })?;

        transfer
            .perform()
            .chain_err(|| format!("failed to fetch index file `{}`", path.display()))?;
        drop(transfer);

        // Avoid the same conditional headers being sent in future re-uses of the `Easy` client.
        let mut list = List::new();
        list.append("If-Modified-Since:")?;
        list.append("If-None-Match:")?;
        handle.http_headers(list)?;
        let response_code = handle.response_code()?;
        drop(handle);

        debug!("index file downloaded with status code {}", response_code,);

        // Make sure we don't double-check the file again if it's loaded again.
        assert!(
            self.fresh.insert(path.to_path_buf()),
            "downloaded the index file `{}` twice",
            path.display(),
        );

        match response_code {
            200 => {}
            304 => {
                // Not Modified response.
                let (_, _, bytes) =
                    was.expect("conditional request response implies we have local index file");
                return data(bytes);
            }
            403 | 404 | 410 | 451 => {
                // The crate was deleted from the registry.
                if was.is_some() {
                    // Make sure we delete the local index file.
                    debug!("crate {} was deleted from the registry", path.display());
                    paths::remove_file(&pkg)?;
                }
                anyhow::bail!("crate has been deleted from the registry");
            }
            code => {
                anyhow::bail!(
                    "load: server returned unexpected HTTP status code {} for {}{}",
                    code,
                    self.source_id.url(),
                    path.display()
                );
            }
        }

        paths::create_dir_all(pkg.parent().expect("pkg is a file"))?;
        let mut file = paths::create(&root.join(path))?;
        file.write_all(etag.as_deref().unwrap_or("\n").as_bytes())?;
        file.write_all(last_modified.as_deref().unwrap_or("\n").as_bytes())?;
        file.write_all(&contents)?;
        file.flush()?;
        data(&contents)
    }

    fn config(&mut self) -> CargoResult<Option<RegistryConfig>> {
        debug!("loading config");
        self.prepare()?;
        let path = self
            .config
            .assert_package_cache_locked(&self.index_path)
            .to_path_buf();
        let mut config = None;
        self.load(&path, Path::new("config.json"), &mut |json| {
            config = Some(serde_json::from_slice(json)?);
            Ok(())
        })?;
        trace!("config loaded");
        Ok(config)
    }

    fn update_index(&mut self) -> CargoResult<()> {
        if self.config.offline() {
            return Ok(());
        }
        if self.config.cli_unstable().no_index_update {
            return Ok(());
        }
        if self.config.frozen() {
            anyhow::bail!("attempting to update a http repository, but --frozen was specified")
        }
        if !self.config.network_allowed() {
            anyhow::bail!("can't update a http repository in offline mode")
        }
        // Make sure the index is only updated once per session since it is an
        // expensive operation. This generally only happens when the resolver
        // is run multiple times, such as during `cargo publish`.
        if self.config.updated_sources().contains(&self.source_id) {
            return Ok(());
        }

        let path = self.config.assert_package_cache_locked(&self.index_path);
        self.config
            .shell()
            .status("Updating", self.source_id.display_index())?;

        // Actually updating the index is more or less a no-op for this implementation.
        // All it does is ensure that a subsequent load/prefetch will double-check files with the
        // server rather than rely on a locally cached copy of the index files.

        debug!("updating the index");
        self.requested_update = true;
        self.fresh.clear();
        self.config.updated_sources().insert(self.source_id);

        // Create a dummy file to record the mtime for when we updated the
        // index.
        if !path.exists() {
            paths::create_dir_all(&path)?;
        }
        paths::create(&path.join(LAST_UPDATED_FILE))?;

        Ok(())
    }

    // NOTE: What follows is identical to remote.rs

    fn download(&mut self, pkg: PackageId, _checksum: &str) -> CargoResult<MaybeLock> {
        let filename = self.filename(pkg);

        // Attempt to open an read-only copy first to avoid an exclusive write
        // lock and also work with read-only filesystems. Note that we check the
        // length of the file like below to handle interrupted downloads.
        //
        // If this fails then we fall through to the exclusive path where we may
        // have to redownload the file.
        let path = self.cache_path.join(&filename);
        let path = self.config.assert_package_cache_locked(&path);
        if let Ok(dst) = File::open(&path) {
            let meta = dst.metadata()?;
            if meta.len() > 0 {
                return Ok(MaybeLock::Ready(dst));
            }
        }

        let config = self.config()?.unwrap();
        let mut url = config.dl;
        if !url.contains(CRATE_TEMPLATE)
            && !url.contains(VERSION_TEMPLATE)
            && !url.contains(PREFIX_TEMPLATE)
            && !url.contains(LOWER_PREFIX_TEMPLATE)
        {
            write!(url, "/{}/{}/download", CRATE_TEMPLATE, VERSION_TEMPLATE).unwrap();
        }
        let prefix = make_dep_prefix(&*pkg.name());
        let url = url
            .replace(CRATE_TEMPLATE, &*pkg.name())
            .replace(VERSION_TEMPLATE, &pkg.version().to_string())
            .replace(PREFIX_TEMPLATE, &prefix)
            .replace(LOWER_PREFIX_TEMPLATE, &prefix.to_lowercase());

        Ok(MaybeLock::Download {
            url,
            descriptor: pkg.to_string(),
        })
    }

    fn finish_download(
        &mut self,
        pkg: PackageId,
        checksum: &str,
        data: &[u8],
    ) -> CargoResult<File> {
        // Verify what we just downloaded
        let actual = Sha256::new().update(data).finish_hex();
        if actual != checksum {
            anyhow::bail!("failed to verify the checksum of `{}`", pkg)
        }

        let filename = self.filename(pkg);
        self.cache_path.create_dir()?;
        let path = self.cache_path.join(&filename);
        let path = self.config.assert_package_cache_locked(&path);
        let mut dst = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&path)
            .chain_err(|| format!("failed to open `{}`", path.display()))?;
        let meta = dst.metadata()?;
        if meta.len() > 0 {
            return Ok(dst);
        }

        dst.write_all(data)?;
        dst.seek(SeekFrom::Start(0))?;
        Ok(dst)
    }

    fn is_crate_downloaded(&self, pkg: PackageId) -> bool {
        let filename = format!("{}-{}.crate", pkg.name(), pkg.version());
        let path = Path::new(&filename);

        let path = self.cache_path.join(path);
        let path = self.config.assert_package_cache_locked(&path);
        if let Ok(meta) = fs::metadata(path) {
            return meta.len() > 0;
        }
        false
    }
}

impl<'cfg> Downloads<'cfg> {
    fn progress(&self, token: usize, total: u64, cur: u64) -> bool {
        let dl = &self.pending[&token].0;
        dl.total.set(total);
        dl.current.set(cur);
        if self.tick(WhyTick::DownloadUpdate).is_err() {
            return false;
        }

        true
    }

    fn tick(&self, why: WhyTick) -> CargoResult<()> {
        if let WhyTick::DownloadUpdate = why {
            // We don't show progress for individual downloads.
            return Ok(());
        }

        let mut progress = self.progress.borrow_mut();
        let progress = progress.as_mut().unwrap();

        // NOTE: should we show something about self.eager?
        progress.tick(
            self.downloads_finished,
            self.downloads_finished + self.pending.len(),
        )
    }
}

#[derive(Copy, Clone)]
enum WhyTick {
    DownloadStarted,
    DownloadUpdate,
    DownloadFinished,
}

impl<'cfg> Drop for Downloads<'cfg> {
    fn drop(&mut self) {
        let progress = self.progress.get_mut().take().unwrap();
        // Don't print a download summary if we're not using a progress bar,
        // we've already printed lots of `Prefetching...` items.
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
        let index_files = if self.downloads_finished == 1 {
            "index file"
        } else {
            "index files"
        };
        let status = format!(
            "{} {} ({}) in {}",
            self.downloads_finished,
            index_files,
            ByteSize(self.downloaded_bytes),
            util::elapsed(self.start.elapsed())
        );
        // Clear progress before displaying final summary.
        drop(progress);
        drop(self.config.shell().status("Prefetched", status));
    }
}

mod tls {
    use std::cell::Cell;

    use super::Downloads;

    thread_local!(static PTR: Cell<usize> = Cell::new(0));

    pub(crate) fn with<R>(f: impl FnOnce(Option<&Downloads<'_>>) -> R) -> R {
        let ptr = PTR.with(|p| p.get());
        if ptr == 0 {
            f(None)
        } else {
            unsafe { f(Some(&*(ptr as *const Downloads<'_>))) }
        }
    }

    pub(crate) fn set<R>(dl: &Downloads<'_>, f: impl FnOnce() -> R) -> R {
        struct Reset<'a, T: Copy>(&'a Cell<T>, T);

        impl<'a, T: Copy> Drop for Reset<'a, T> {
            fn drop(&mut self) {
                self.0.set(self.1);
            }
        }

        PTR.with(|p| {
            let _reset = Reset(p, p.get());
            p.set(dl as *const Downloads<'_> as usize);
            f()
        })
    }
}
