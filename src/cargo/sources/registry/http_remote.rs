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
use crate::util::{Config, Filesystem, Sha256};
use curl::easy::{Easy, HttpVersion, List};
use curl::multi::{EasyHandle, Multi};
use log::{debug, trace, warn};
use std::cell::{Cell, RefCell, RefMut};
use std::collections::{HashMap, HashSet};
use std::fmt::Write as FmtWrite;
use std::fs::{self, File, OpenOptions};
use std::io::prelude::*;
use std::io::SeekFrom;
use std::path::{Path, PathBuf};
use std::str;
use std::time::Duration;

const ETAG: &'static [u8] = b"ETag";
const LAST_MODIFIED: &'static [u8] = b"Last-Modified";

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
/// The last known state of the changelog.
enum ChangelogState {
    /// The server does not host a changelog.
    ///
    /// In this state, we must double-check with the server every time we want to load an index
    /// file in case that file has changed upstream.
    Unsupported,

    /// The server served us a changelog in the past.
    Synchronized {
        /// The last known changelog epoch (see the RFC).
        ///
        /// The epoch allows the server to start the changelog over for garbage-collection purposes
        /// in a way that the client can detect.
        epoch: usize,

        /// The last known length of the changelog (in bytes).
        ///
        /// This is used to efficiently fetch only the suffix of the changelog that has been
        /// appended since we last read it.
        length: usize,
    },
}

impl ChangelogState {
    fn is_synchronized(&self) -> bool {
        matches!(self, ChangelogState::Synchronized { .. })
    }
}

impl Into<(ChangelogState, InternedString)> for ChangelogState {
    fn into(self) -> (ChangelogState, InternedString) {
        let is = InternedString::from(self.to_string());
        (self, is)
    }
}

impl std::str::FromStr for ChangelogState {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "unsupported" {
            return Ok(ChangelogState::Unsupported);
        }

        let mut parts = s.split('.');
        let epoch = parts.next().expect("split always yields one item");
        let epoch = usize::from_str_radix(epoch, 10).map_err(|_| "invalid epoch")?;
        let length = parts.next().ok_or("no changelog offset")?;
        let length = usize::from_str_radix(length, 10).map_err(|_| "invalid changelog offset")?;
        Ok(ChangelogState::Synchronized { epoch, length })
    }
}

impl ToString for ChangelogState {
    fn to_string(&self) -> String {
        match *self {
            ChangelogState::Unsupported => String::from("unsupported"),
            ChangelogState::Synchronized { epoch, length } => format!("{}.{}", epoch, length),
        }
    }
}

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
/// `If-None-Match` for `ETag`s) which can be efficiently handled by HTTP/2, but it's still not
/// ideal. The RFC therefor also introduces the (optional) notion of a _changelog_. The changelog
/// is a dedicated append-only file on the server that lists every crate index change. This allows
/// the client to fetch the changelog, invalidate its locally cached index files for only the
/// changed crates, and then not worry about double-checking with the server for each index file.
///
/// In order to take advantage of HTTP/2's ability to efficiently send multiple concurrent HTTP
/// requests over a single connection, `HttpRegistry` also supports asynchronous prefetching. The
/// caller queues up a number of index files they think it is likely they will want to access, and
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

    /// The current (last known) state of the changelog.
    at: Cell<(ChangelogState, InternedString)>,

    /// Have we loaded self.at from .last-updated (by calling prepare) yet?
    checked_for_at: Cell<bool>,

    /// Cached HTTP handle for synchronous requests (changelog + RegistryData::load).
    http: RefCell<Option<Easy>>,

    /// HTTP multi-handle for asynchronous/parallel requests during prefetching.
    prefetch: Multi,

    /// State for currently pending prefetch downloads.
    downloads: Downloads,

    /// Does the config say that we can use HTTP multiplexing?
    multiplexing: bool,

    /// Has a prefetch phase been run?
    ///
    /// If so, we do not need to double-check any index files -- the prefetch stage already did.
    prefetched: bool,
}

impl<'cfg> HttpRegistry<'cfg> {
    pub fn new(source_id: SourceId, config: &'cfg Config, name: &str) -> HttpRegistry<'cfg> {
        HttpRegistry {
            index_path: config.registry_index_path().join(name),
            cache_path: config.registry_cache_path().join(name),
            source_id,
            config,
            at: Cell::new(ChangelogState::Unsupported.into()),
            checked_for_at: Cell::new(false),
            http: RefCell::new(None),
            prefetch: Multi::new(),
            multiplexing: false,
            prefetched: false,
            downloads: Downloads::default(),
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
        // Load last known changelog state from LAST_UPDATED_FILE.
        if !self.checked_for_at.get() {
            self.checked_for_at.set(true);
            let path = self.config.assert_package_cache_locked(&self.index_path);
            if path.exists() {
                let cl_state = paths::read(&path.join(LAST_UPDATED_FILE))?;
                let cl_state: ChangelogState = cl_state
                    .parse()
                    .map_err(|e| anyhow::anyhow!("{}", e))
                    .chain_err(|| {
                        format!("failed to parse last changelog state: '{}'", cl_state)
                    })?;
                self.at.set(cl_state.into());
            }
        }

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

                // TODO: explicitly enable HTTP2?
                // https://github.com/rust-lang/cargo/blob/905134577c1955ad7865bcf4b31440d4bc882cde/src/cargo/core/package.rs#L651-L703

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
        //
        // TODO: Is that still the case? We probably want pipelining here if possible.
        self.multiplexing = self.config.http_config()?.multiplexing.unwrap_or(true);

        self.prefetch
            .pipelining(false, self.multiplexing)
            .chain_err(|| "failed to enable multiplexing/pipelining in curl")?;

        // let's not flood crates.io with connections
        self.prefetch.set_max_host_connections(2)?;

        self.prefetched = true;

        Ok(true)
    }

    fn prefetch(
        &mut self,
        root: &Path,
        path: &Path,
        name: InternedString,
        req: &semver::VersionReq,
    ) -> CargoResult<()> {
        // A quick overview of what goes on below:
        //
        // We first check if we have a local copy of the given index file.
        //
        // If we do, and the server has a changelog, then we know that the index file is up to
        // date (as of when we last checked the changelog), so there's no need to double-check with
        // the server that the file isn't stale. We can just tell the next call to
        // `next_prefetched` to go ahead with this path immediately. If we _need_ a newer version
        // of it, `update_index` will be called and then `prefetch` will be called again.
        //
        // If we do, but the server does not have a changelog, we need to check with the server if
        // the index file has changed upstream. We do this using a conditional HTTP request using
        // the `Last-Modified` and `ETag` headers we got when we fetched the currently cached index
        // file (those headers are stored in the first two lines of each index file). That way, if
        // nothing has changed (likely the common case), the server doesn't have to send us
        // any data, just a 304 Not Modified.
        //
        // If we don't have a local copy of the index file, we need to fetch it from the server.
        let pkg = root.join(path);
        let bytes;
        let was = if pkg.exists() {
            if self.at.get().0.is_synchronized() {
                // We already have this file locally, and we don't need to double-check it with
                // upstream because we have a changelog, so there's really nothing to prefetch.
                // We do keep track of the request though so that we will eventually yield this
                // back to the caller who may then want to prefetch other transitive dependencies.
                if let Some(f) = self
                    .downloads
                    .eager
                    .iter_mut()
                    .find(|f| f.primary.path == path)
                {
                    if &f.primary.req != req {
                        f.others.insert(req.clone());
                    }
                } else {
                    self.downloads.eager.push(MultiVersionFetched {
                        primary: Fetched {
                            path: path.to_path_buf(),
                            name,
                            req: req.clone(),
                        },
                        others: HashSet::new(),
                    });
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
            debug!("double-checking freshness of {}", path.display());

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
            if &dl.req != req {
                dl.additional_reqs.insert(req.clone());
            }
            return Ok(());
        } else if let Some(f) = self
            .downloads
            .eager
            .iter_mut()
            .find(|f| f.primary.path == path)
        {
            if &f.primary.req != req {
                f.others.insert(req.clone());
            }
            return Ok(());
        }

        // Looks like we're going to have to bite the bullet and do a network request.
        let url = self.source_id.url();
        self.prepare()?;

        let mut handle = ops::http_handle(self.config)?;
        debug!("fetch {}{}", url, path.display());
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
        let dl = Download {
            token,
            data: RefCell::new(Vec::new()),
            path: path.to_path_buf(),
            name,
            req: req.clone(),
            additional_reqs: HashSet::new(),
            etag: RefCell::new(None),
            last_modified: RefCell::new(None),
        };

        // Each write should go to self.downloads.pending[&token].data.
        // Since the write function must be 'static, we access downloads through a thread-local.
        // That thread-local is set up in `next_prefetched` when it calls self.prefetch.perform,
        // which is what ultimately calls this method.
        handle.write_function(move |buf| {
            debug!("{} - {} bytes of data", token, buf.len());
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

        // Same goes for the header function -- it goes through thread-local storage.
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

        // TODO: Track and display download progress (see `Downloads` in `core/pacakge.rs`).

        // Finally add the request we've lined up to the pool of requests that cURL manages.
        let mut handle = self.prefetch.add(handle)?;
        handle.set_token(token)?;
        self.downloads.pending.insert(dl.token, (dl, handle));

        Ok(())
    }

    fn next_prefetched(&mut self) -> CargoResult<Option<Fetched>> {
        while !self.downloads.pending.is_empty() && !self.downloads.eager.is_empty() {
            // We may already have packages that are ready to go. This takes care of grabbing the
            // next of those, while ensuring that we yield every distinct version requirement for
            // each package.
            if let Some(mut fetched) = self.downloads.eager.pop() {
                return if let Some(req) = fetched.others.iter().next().cloned() {
                    fetched.others.remove(&req);
                    let ret = Ok(Some(Fetched {
                        path: fetched.primary.path.clone(),
                        name: fetched.primary.name,
                        req,
                    }));
                    self.downloads.eager.push(fetched);
                    ret
                } else {
                    Ok(Some(fetched.primary))
                };
            }

            // We don't have any fetched results immediately ready to be yielded,
            // so we need to check if curl has made any progress.
            assert_eq!(
                self.downloads.pending.len(),
                self.downloads.pending_ids.len()
            );
            // Note the `tls::set` here which sets up the thread-local storage needed to access
            // self.downloads from `write_function` and `header_function` above.
            let remaining_in_multi = tls::set(&self.downloads, || {
                self.prefetch
                    .perform()
                    .chain_err(|| "failed to perform http requests")
            })?;
            debug!("handles remaining: {}", remaining_in_multi);

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
            while let Some((token, result)) = results.pop() {
                debug!("{} finished with {:?}", token, result);

                let (dl, handle) = self
                    .downloads
                    .pending
                    .remove(&token)
                    .expect("got a token for a non-in-progress transfer");

                let data = dl.data.into_inner();
                let mut handle = self.prefetch.remove(handle)?;
                self.downloads.pending_ids.remove(&dl.path);

                let fetched = MultiVersionFetched {
                    primary: Fetched {
                        path: dl.path,
                        name: dl.name,
                        req: dl.req,
                    },
                    others: dl.additional_reqs,
                };

                let code = handle.response_code()?;
                debug!(
                    "index file downloaded with status code {}",
                    handle.response_code()?
                );
                // TODO: How do we ensure that the next call to load doesn't _also_ send an HTTP
                // request? Do we need to keep track of each fetched prefetched path or something?
                match code {
                    200 => {
                        // We got data back, hooray!
                        // Let's update the index file.
                        let path = self.config.assert_package_cache_locked(&self.index_path);
                        let pkg = path.join(&fetched.primary.path);
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

                        self.downloads.eager.push(fetched);
                    }
                    304 => {
                        // Not Modified response.
                        // There's nothing for us to do -- the index file is up to date.
                        // The only thing that matters is telling the caller about this package.
                        self.downloads.eager.push(fetched);
                    }
                    404 => {
                        // Not Found response.
                        // The crate doesn't exist, so we simply do not yield it.
                    }
                    410 | 451 => {
                        // The crate was deleted from the registry.
                        todo!();
                    }
                    code => {
                        anyhow::bail!("server returned unexpected HTTP status code {}", code);
                    }
                }
            }

            if !self.downloads.eager.is_empty() {
                continue;
            }

            if self.downloads.pending.is_empty() {
                // We're all done!
                return Ok(None);
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
        let cl_state = self.at.get();
        if cl_state.0.is_synchronized() {
            Some(cl_state.1)
        } else {
            None
        }
    }

    fn load(
        &self,
        root: &Path,
        path: &Path,
        data: &mut dyn FnMut(&[u8]) -> CargoResult<()>,
    ) -> CargoResult<()> {
        // NOTE: This is pretty much a synchronous version of the prefetch() + next_prefetched()
        // dance. Much of the code is sort-of duplicated, which isn't great, but it works.

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

            // NOTE: We should always double-check for changes to config.json.
            let double_check = !self.at.get().0.is_synchronized() || path.ends_with("config.json");

            if double_check {
                if self.prefetched {
                    trace!(
                        "not double-checking freshness of {} after prefetch",
                        path.display()
                    );
                } else if self.config.offline() {
                    debug!(
                        "not double-checking freshness of {} due to offline",
                        path.display()
                    );
                } else {
                    debug!("double-checking freshness of {}", path.display());
                }
            } else {
                debug!(
                    "using {} from cache as changelog is synchronized",
                    path.display()
                );
            }

            // NOTE: If we're in offline mode, we don't double-check with the server.
            if !double_check || self.config.offline() {
                return data(rest);
            } else {
                // We cannot trust the index files and need to double-check with server.
                let etag = std::str::from_utf8(etag)?;
                let last_modified = std::str::from_utf8(last_modified)?;
                Some((etag, last_modified, rest))
            }
        } else {
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

        // TODO: Should we display transfer status here somehow?

        transfer
            .perform()
            .chain_err(|| format!("failed to fetch index file `{}`", path.display()))?;
        drop(transfer);

        // Avoid the same conditional headers being sent in future re-uses of the `Easy` client.
        let mut list = List::new();
        list.append("If-Modified-Since:")?;
        list.append("If-None-Match:")?;
        handle.http_headers(list)?;

        debug!(
            "index file downloaded with status code {}",
            handle.response_code()?
        );
        match handle.response_code()? {
            200 => {}
            304 => {
                // Not Modified response.
                let (_, _, bytes) =
                    was.expect("conditional request response implies we have local index file");
                return data(bytes);
            }
            404 | 410 | 451 => {
                // The crate was deleted from the registry.
                if was.is_some() {
                    // Make sure we delete the local index file.
                    debug!("crate {} was deleted from the registry", path.display());
                    paths::remove_file(&pkg)?;
                }
                anyhow::bail!("crate has been deleted from the registry");
            }
            code => {
                anyhow::bail!("server returned unexpected HTTP status code {}", code);
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
        self.config.assert_package_cache_locked(&self.index_path);
        let mut config = None;
        self.load(Path::new(""), Path::new("config.json"), &mut |json| {
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
        // Make sure the index is only updated once per session since it is an
        // expensive operation. This generally only happens when the resolver
        // is run multiple times, such as during `cargo publish`.
        if self.config.updated_sources().contains(&self.source_id) {
            return Ok(());
        }

        // NOTE: We check for the changelog even if the server did not previously have a changelog
        // in case it has wisened up since then.

        debug!("updating the index");

        // Make sure that subsequent loads double-check with the server again.
        self.prefetched = false;

        self.prepare()?;
        let path = self.config.assert_package_cache_locked(&self.index_path);
        self.config
            .shell()
            .status("Updating", self.source_id.display_index())?;

        let url = self.source_id.url();
        let mut handle = self.http()?;
        handle.url(&format!("{}/changelog", url))?;

        // TODO: Retry logic using network::with_retry?

        /// How are we attempting to fetch the changelog?
        #[derive(Debug, Copy, Clone)]
        enum ChangelogStrategy {
            /// We are fetching the changelog with no historical context.
            FirstFetch { full: bool },
            /// We are trying to follow the changelog to update our view of the index.
            Follow { epoch: usize, length: usize },
        }
        let mut plan = if let ChangelogState::Synchronized { epoch, length } = self.at.get().0 {
            ChangelogStrategy::Follow { epoch, length }
        } else {
            ChangelogStrategy::FirstFetch { full: false }
        };

        // NOTE: Loop in case of rollover, in which case we need to fetch it starting at byte 0.
        let was = self.at.get();
        'changelog: loop {
            // Reset in case we looped.
            handle.range("")?;
            handle.resume_from(0)?;

            match plan {
                ChangelogStrategy::Follow { length, .. } => {
                    handle.resume_from(length as u64)?;
                }
                ChangelogStrategy::FirstFetch { full: false } => {
                    // We really just need the epoch number and file size,
                    // which we can get at by fetching just the first line.
                    // "1 2019-10-18 23:51:23 ".len() == 22
                    handle.range("0-22")?;
                }
                ChangelogStrategy::FirstFetch { full: _ } => {}
            }

            let mut contents = Vec::new();
            let mut total_bytes = None;
            let mut transfer = handle.transfer();
            transfer.write_function(|buf| {
                contents.extend_from_slice(buf);
                Ok(buf.len())
            })?;

            // Extract `Content-Range` header to learn the total size of the changelog.
            //
            // We need the total size from `Content-Range` since we only fetch a very small subset
            // of the changelog when we first access the server (just enought to get the epoch).
            transfer.header_function(|buf| {
                const CONTENT_RANGE: &'static [u8] = b"Content-Range:";
                if buf.len() > CONTENT_RANGE.len()
                    && buf[..CONTENT_RANGE.len()].eq_ignore_ascii_case(CONTENT_RANGE)
                {
                    let mut buf = &buf[CONTENT_RANGE.len()..];

                    // Trim leading whitespace.
                    while !buf.is_empty() && buf[0] == b' ' {
                        buf = &buf[1..];
                    }

                    // Check that the Content-Range unit is indeed bytes.
                    const BYTES_UNIT: &'static [u8] = b"bytes ";
                    if !buf.starts_with(BYTES_UNIT) {
                        return true;
                    }
                    buf = &buf[BYTES_UNIT.len()..];

                    // Extract out the total length.
                    let rest = buf.splitn(2, |&c| c == b'/');
                    if let Some(complete_length) = rest.skip(1 /* byte-range */).next() {
                        if complete_length.starts_with(b"*") {
                            // The server does not know the total size of the changelog.
                            // This seems weird, but not much we can do about it.
                            // We'll end up falling back to a full fetch.
                            return true;
                        }
                        let complete_length = complete_length
                            .splitn(2, |&c| c == b' ')
                            .next()
                            .expect("split always yields >= 1 element");
                        if complete_length.into_iter().all(|c| c.is_ascii_digit()) {
                            let complete_length =
                                std::str::from_utf8(complete_length).expect("only ascii digits");
                            total_bytes = Some(
                                usize::from_str_radix(complete_length, 10)
                                    .expect("ascii digits make for valid numbers"),
                            );
                        }
                    }
                }
                true
            })?;

            // TODO: Should we show progress here somehow?

            transfer
                .perform()
                .chain_err(|| format!("failed to fetch index changelog from `{}`", url))?;
            drop(transfer);

            let mut contents = &contents[..];
            let total_bytes = match handle.response_code()? {
                200 => {
                    // The server does not support Range: requests,
                    // so we need to manually slice the bytes we got back.
                    //
                    // TODO: This is a really bad operating state! We're fetching the _entire_
                    // changelog each time we update the changelog. Not clear if that's better than
                    // just validating each index lookup?
                    let total_bytes = contents.len();
                    if let ChangelogStrategy::Follow { length, .. } = plan {
                        if contents.len() < length || contents.len() == 0 {
                            // The changelog must have rolled over.
                            // Luckily, since the server sent the whole response,
                            // we can just continue as if that was our plan all along.
                            plan = ChangelogStrategy::FirstFetch { full: true };
                        } else {
                            contents = &contents[length..];
                        }
                    }
                    total_bytes
                }
                206 => {
                    // 206 Partial Content -- this is what we expect to get.
                    match total_bytes {
                        None => {
                            // The server sent us back only the byte range we asked for,
                            // but it did not inform us of the total size of the changelog.
                            // This is fine if we're just following the changelog, since we can
                            // compute the total size (old size + size of content), but if we're
                            // trying to _start_ following the changelog, we need to know its
                            // current size to know where to fetch from next time!
                            match plan {
                                ChangelogStrategy::FirstFetch { full } => {
                                    assert!(!full, "got partial response without Range:");

                                    // Our only recourse is to fetch the full changelog.
                                    plan = ChangelogStrategy::FirstFetch { full: true };
                                    continue;
                                }
                                ChangelogStrategy::Follow { length, .. } => length + contents.len(),
                            }
                        }
                        Some(b) => b,
                    }
                }
                404 => {
                    // The server does not have a changelog.
                    if self.at.get().0.is_synchronized() {
                        // We used to have a changelog, but now we don't. It's important that we
                        // record that fact so that later calls to load() will all double-check
                        // with the server.
                        self.at.set(ChangelogState::Unsupported.into());
                    }
                    break 'changelog;
                }
                416 => {
                    // 416 Range Not Satisfiable
                    //
                    // This can mean one of two things:
                    //
                    //  1. The changelog has rolled over, so we requested too much data.
                    //  2. There are no new entries (our request goes beyond the end of the
                    //     changelog).
                    //
                    // If we hit case 1, we need to fetch the start of the new changelog instead.
                    // If we hit case 2, what we'd like to do is, well, nothing.
                    match (plan, total_bytes) {
                        (ChangelogStrategy::Follow { length, .. }, Some(total_bytes))
                            if length == total_bytes =>
                        {
                            contents = &[];
                            total_bytes
                        }
                        // We must assume we're in case 1.
                        (ChangelogStrategy::FirstFetch { full }, _) => {
                            // Our request for just the start of the changelog (Range: 0-22) failed.
                            // This probably means that the changelog is empty, but we do a full fetch
                            // to make sure.
                            assert!(!full);
                            plan = ChangelogStrategy::FirstFetch { full: true };
                            continue;
                        }
                        (ChangelogStrategy::Follow { .. }, _) => {
                            // We requested a byte range past the end of the changelog, which
                            // implies that it must have rolled over (and shrunk).
                            plan = ChangelogStrategy::FirstFetch { full: false };
                            continue;
                        }
                    }
                }
                code => {
                    anyhow::bail!("server returned unexpected HTTP status code {}", code);
                }
            };

            if contents.len() == 0 {
                if total_bytes == 0 {
                    // We can't use the changelog, since we don't know its epoch.
                    self.at.set(ChangelogState::Unsupported.into());
                } else {
                    // There are no changes in changelog, so there's supposedly nothing to update.
                    //
                    // TODO: This isn't fool-proof. It _could_ be that the changelog rolled over,
                    // and just so happens to be exactly the same length as the old changelog was
                    // last time we checked it. This is quite unlikely, but not impossible. To fix
                    // this, we should keep track of ETag + Last-Modified, and check that here. If
                    // they do not match, then fall back to a ::FirstFetch.
                }
                break;
            }

            enum WhatLine {
                First,
                Second { first_failed: bool },
                Later,
            }
            let mut at = WhatLine::First;

            let mut line = String::new();
            let mut new_changelog = false;
            let mut fetched_epoch = None;
            while contents.read_line(&mut line)? != 0 {
                // First, make sure that the line is a _complete_ line.
                // It's possible that the changelog rolled over, _but_ our old range was still
                // valid. In that case, the returned content may not start at a line bounary, and
                // parsing will fail in weird ways. Or worse yet, succeed but with an incorrect
                // epoch number! Should that happen, we need to detect it.
                //
                // Lines _should_ look like this:
                // 1 2019-10-18 23:52:00 anyhow
                //
                // That is: epoch date time crate.
                let mut parts = line.trim().split_whitespace();
                let epoch = parts.next().expect("split always has one element");
                let krate = parts.skip(2).next();

                if epoch.is_empty() {
                    // Skip empty lines.

                    // We _have_ observed a line change though,
                    // so the next epoch read is guaranteed to read a complete epoch.
                    if let WhatLine::First = at {
                        at = WhatLine::Second {
                            first_failed: false,
                        };
                    }
                    continue;
                }

                let epoch = if let Ok(epoch) = epoch.parse::<usize>() {
                    fetched_epoch = Some(epoch);
                    epoch
                } else if let WhatLine::First = at {
                    // The line is clearly not valid.
                    //
                    // This means the changelog rolled over. Unfortunately, the byte range we
                    // requested does not contain the epoch, so we don't have enough information to
                    // move forwards. We need to parse one more line.

                    // If we got here during a first fetch (which fetches starting at byte 0), the
                    // server's changelog is entirely bad.
                    if let ChangelogStrategy::FirstFetch { .. } = plan {
                        warn!("server changelog does not begin with an epoch");
                        // Ensure that all future index fetches check with server
                        self.at.set(ChangelogState::Unsupported.into());
                        break 'changelog;
                    }

                    debug!(
                        "index {} changelog has invalid first line; assuming rollover",
                        url
                    );
                    at = WhatLine::Second { first_failed: true };
                    continue;
                } else {
                    warn!("index {} changelog has invalid lines", url);
                    // Ensure that all future index fetches check with server
                    self.at.set(ChangelogState::Unsupported.into());
                    break 'changelog;
                };

                match plan {
                    ChangelogStrategy::FirstFetch { .. } => {
                        // This requested bytes starting at 0, so the epoch we parsed out is valid.

                        // We don't actually care about the remainder of the changelog,
                        // since we've completely purged our local index.
                        new_changelog = true;
                        at = WhatLine::Later;
                        break;
                    }
                    ChangelogStrategy::Follow {
                        epoch: last_epoch, ..
                    } if last_epoch != epoch => {
                        // There has clearly been a rollover, though we have to be a little
                        // careful. Since we requested a particular byte offset, the parsed epoch
                        // may not actually have been the "true" epoch. Imagine that we fetched:
                        //
                        // 1 2019-10-18 23:52:00 anyhow
                        //
                        // it _could_ be that that's just an unfortunate slice of this line:
                        //
                        // 21 2019-10-18 23:52:00 anyhow
                        //
                        // So, we need to parse a second line to ensure we have the _true_ line.
                        if let WhatLine::First = at {
                            at = WhatLine::Second { first_failed: true };
                            continue;
                        }

                        debug!("index {} changelog has rolled over", url);

                        // TODO: Try previous changelog if available?
                        // https://github.com/rust-lang/rfcs/pull/2789#issuecomment-730024821

                        // We're starting over with this new, rolled-over changelog, so we don't
                        // care about its contents.
                        new_changelog = true;
                        at = WhatLine::Later;
                        break;
                    }
                    ChangelogStrategy::Follow { .. } => {}
                }

                at = match at {
                    WhatLine::First => WhatLine::Second {
                        first_failed: false,
                    },
                    WhatLine::Second { first_failed: true } => {
                        // If the first line failed to parse, that must mean there was a rollover.
                        // If we get here, that means that we're in ::Follow mode, but that the
                        // next line had an epoch that _did_ match our own epoch, which would imply
                        // there _wasn't_ a rollover. Something is _very_ wrong.
                        unreachable!("server response byte offset mismatch");
                    }
                    WhatLine::Second { first_failed: _ } | WhatLine::Later => WhatLine::Later,
                };

                let krate = if let Some(krate) = krate {
                    krate
                } else {
                    warn!("index {} changelog has an invalid line: {}", url, line);

                    // We could error out here, but it's always safe for us to ignore the changelog
                    // and just double-check all index file loads instead, so we prefer that.
                    self.at.set(ChangelogState::Unsupported.into());
                    break 'changelog;
                };

                if krate.is_empty() {
                    warn!("index {} changelog has an invalid line: {}", url, line);

                    // Same as above -- prefer working to failing.
                    self.at.set(ChangelogState::Unsupported.into());
                    break 'changelog;
                }

                // Remove the outdated index file -- we'll have to re-fetch it
                let path = path.join(&Path::new(&make_dep_prefix(krate))).join(krate);
                if path.exists() {
                    paths::remove_file(path)?;
                }
            }

            if let WhatLine::Second { first_failed } = at {
                let (epoch, length) = if let ChangelogStrategy::Follow { epoch, length } = plan {
                    (epoch, length)
                } else {
                    unreachable!("::FirstFetch always breaks on the first line");
                };

                if first_failed {
                    // The changelog must have rolled over. This means that whatever we got in
                    // `fetched_epoch` may not be valid due to weird byte offsets. Unfortunately,
                    // we never got a second line to ensure we parsed a complete epoch either! Our
                    // only option here is to do another request to the server for the start of the
                    // changelog.
                    plan = ChangelogStrategy::FirstFetch { full: false };
                    continue;
                }

                // There is a _slight_ chance that there was a rollover, and that the
                // byte offset we provided happened to be valid, and happened to perfectly
                // align so that the string starts with a number that just so happens to be
                // the same as the old epoch. That's... weird, but possible.
                //
                // Basically, imagine that the previous epoch we knew about was 3, and the first
                // (and only) line we got in the changelog diff we requested was:
                //
                // 3 2019-10-18 23:52:00 anyhow
                //
                // All good, right? Well, not _quite_.
                // What if that is just a weird slicing of this line:
                //
                // 13 2019-10-18 23:52:00 anyhow
                //
                // And since there was no second line, we never saw epoch 13, and just kept going
                // as if everything is fine. To make absolutely sure, we do another fetch of the
                // changelog that includes some earlier data as well. That fetch should get more
                // than one line, and so detect any such epoch shenanigans.
                plan = ChangelogStrategy::Follow {
                    epoch,
                    // How far back we go here isn't super important. We just have to make sure we
                    // go at least one line back, so that the response will include at least two
                    // lines. The longer back we go, the more index entries we will unnecessarily
                    // invalidate. If we don't go far enough, we'll just end up in this clause
                    // again and do another round trip to go further back.
                    length: length.saturating_sub(16),
                };
                continue;
            }

            let epoch =
                fetched_epoch.expect("changelog was non-empty, and epoch parsing didn't fail");

            if new_changelog {
                debug!(
                    "index {} is at epoch {} (offset: {})",
                    url, epoch, total_bytes
                );

                // We don't know which index entries are now invalid and which are not,
                // so we have to purge them all.
                //
                // TODO: Will this cause issues with directory locking?
                if path.exists() {
                    paths::remove_dir_all(&path)?;
                    paths::create_dir_all(&path)?;
                }

                // From this point forward, we're synchronized with the changelog!
                self.at.set(
                    ChangelogState::Synchronized {
                        epoch,
                        length: total_bytes,
                    }
                    .into(),
                );
            } else {
                // Keep track of our new byte offset into the changelog.
                self.at.set(
                    ChangelogState::Synchronized {
                        epoch,
                        length: total_bytes,
                    }
                    .into(),
                );
            }
            break;
        }

        // Reset the http handle for later requests that re-use the Easy.
        handle.range("")?;
        handle.resume_from(0)?;

        self.config.updated_sources().insert(self.source_id);

        // Record the latest known state of the index if it changed.
        let lu_file = path.join(LAST_UPDATED_FILE);
        if !lu_file.exists() || was != self.at.get() {
            if !path.exists() {
                paths::create_dir_all(&path)?;
            }
            let mut file = paths::create(&lu_file)?;
            file.write_all(self.at.get().1.as_bytes())?;
            file.flush()?;
        }

        Ok(())
    }

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

struct MultiVersionFetched {
    primary: Fetched,
    others: HashSet<semver::VersionReq>,
}

// NOTE: what follows is lifted from src/cargo/core/package.rs and tweaked

/// Helper for downloading crates.
#[derive(Default)]
pub struct Downloads {
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
    eager: Vec<MultiVersionFetched>,
    /// The next ID to use for creating a token (see `Download::token`).
    next: usize,
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
    req: semver::VersionReq,

    /// Additional version requirements for same package.
    additional_reqs: HashSet<semver::VersionReq>,

    /// Actual downloaded data, updated throughout the lifetime of this download.
    data: RefCell<Vec<u8>>,

    /// ETag and Last-Modified headers received from the server (if any).
    etag: RefCell<Option<String>>,
    last_modified: RefCell<Option<String>>,
}

mod tls {
    use std::cell::Cell;

    use super::Downloads;

    thread_local!(static PTR: Cell<usize> = Cell::new(0));

    pub(crate) fn with<R>(f: impl FnOnce(Option<&Downloads>) -> R) -> R {
        let ptr = PTR.with(|p| p.get());
        if ptr == 0 {
            f(None)
        } else {
            unsafe { f(Some(&*(ptr as *const Downloads))) }
        }
    }

    pub(crate) fn set<R>(dl: &Downloads, f: impl FnOnce() -> R) -> R {
        struct Reset<'a, T: Copy>(&'a Cell<T>, T);

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
