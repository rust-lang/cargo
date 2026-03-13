//! Access to a HTTP-based crate registry. See [`HttpRegistry`] for details.

use crate::core::global_cache_tracker;
use crate::core::{PackageId, SourceId};
use crate::sources::registry::MaybeLock;
use crate::sources::registry::download;
use crate::sources::registry::{LoadResponse, RegistryConfig, RegistryData};
use crate::util::cache_lock::CacheLockMode;
use crate::util::errors::{CargoResult, HttpNotSuccessful};
use crate::util::interning::InternedString;
use crate::util::network::http_async::ResponsePartsExtensions;
use crate::util::network::retry::{Retry, RetryResult};
use crate::util::{Filesystem, GlobalContext, IntoUrl, Progress, ProgressStyle, auth};
use anyhow::Context as _;
use cargo_credential::Operation;
use cargo_util::paths;
use futures::channel::oneshot;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::str;
use std::time::Duration;
use tracing::{debug, trace};
use url::Url;

// HTTP headers
const ETAG: &'static str = "etag";
const LAST_MODIFIED: &'static str = "last-modified";

const UNKNOWN: &'static str = "Unknown";

/// A registry served by the HTTP-based registry API.
///
/// This type is primarily accessed through the [`RegistryData`] trait.
///
/// `HttpRegistry` implements the HTTP-based registry API outlined in [RFC 2789]. Read the RFC for
/// the complete protocol, but _roughly_ the implementation loads each index file (e.g.,
/// config.json or re/ge/regex) from an HTTP service rather than from a locally cloned git
/// repository. The remote service can more or less be a static file server that simply serves the
/// contents of the origin git repository.
///
/// Implemented naively, this leads to a significant amount of network traffic, as a lookup of any
/// index file would need to check with the remote backend if the index file has changed. This
/// cost is somewhat mitigated by the use of HTTP conditional fetches (`If-Modified-Since` and
/// `If-None-Match` for `ETag`s) which can be efficiently handled by HTTP/2.
///
/// [RFC 2789]: https://github.com/rust-lang/rfcs/pull/2789
pub struct HttpRegistry<'gctx> {
    /// The name of this source, a unique string (across all sources) used as
    /// the directory name where its cached content is stored.
    name: InternedString,
    /// Path to the registry index (`$CARGO_HOME/registry/index/$REG-HASH`).
    ///
    /// To be fair, `HttpRegistry` doesn't store the registry index it
    /// downloads on the file system, but other cached data like registry
    /// configuration could be stored here.
    index_path: Filesystem,
    /// Path to the cache of `.crate` files (`$CARGO_HOME/registry/cache/$REG-HASH`).
    cache_path: Filesystem,
    /// The unique identifier of this registry source.
    source_id: SourceId,
    gctx: &'gctx GlobalContext,

    /// Store the server URL without the protocol prefix (sparse+)
    url: Url,

    /// Has the client requested a cache update?
    ///
    /// Only if they have do we double-check the freshness of each locally-stored index file.
    requested_update: Cell<bool>,

    /// Progress bar for transfers.
    progress: RefCell<Option<Progress<'gctx>>>,

    /// Pending async
    pending: RefCell<HashMap<PathBuf, Vec<oneshot::Sender<CargoResult<LoadResponse>>>>>,

    /// Does the config say that we can use HTTP multiplexing?
    multiplexing: Cell<bool>,

    /// What paths have we already fetched since the last index update?
    ///
    /// We do not need to double-check any of these index files since we have already done so.
    fresh: RefCell<HashSet<PathBuf>>,

    /// Have we started to download any index files?
    fetch_started: Cell<bool>,

    /// Cached registry configuration.
    registry_config: RefCell<Option<RegistryConfig>>,

    /// Should we include the authorization header?
    auth_required: Cell<bool>,

    /// Url to get a token for the registry.
    login_url: RefCell<Option<Url>>,

    /// Headers received with an HTTP 401.
    auth_error_headers: RefCell<Vec<String>>,

    /// Disables status messages.
    quiet: Cell<bool>,
}

impl<'gctx> HttpRegistry<'gctx> {
    /// Creates a HTTP-rebased remote registry for `source_id`.
    ///
    /// * `name` --- Name of a path segment where `.crate` tarballs and the
    ///   registry index are stored. Expect to be unique.
    pub fn new(
        source_id: SourceId,
        gctx: &'gctx GlobalContext,
        name: &str,
    ) -> CargoResult<HttpRegistry<'gctx>> {
        let url = source_id.url().as_str();
        // Ensure the url ends with a slash so we can concatenate paths.
        if !url.ends_with('/') {
            anyhow::bail!("sparse registry url must end in a slash `/`: {url}")
        }
        assert!(source_id.is_sparse());
        let url = url
            .strip_prefix("sparse+")
            .expect("sparse registry needs sparse+ prefix")
            .into_url()
            .expect("a url with the sparse+ stripped should still be valid");

        Ok(HttpRegistry {
            name: name.into(),
            index_path: gctx.registry_index_path().join(name),
            cache_path: gctx.registry_cache_path().join(name),
            source_id,
            gctx,
            url,
            multiplexing: Cell::new(false),
            progress: RefCell::new(Some(Progress::with_style(
                "Fetch",
                ProgressStyle::Indeterminate,
                gctx,
            ))),
            fresh: RefCell::new(HashSet::new()),
            requested_update: Cell::new(false),
            fetch_started: Cell::new(false),
            registry_config: RefCell::new(None),
            auth_required: Cell::new(false),
            login_url: RefCell::new(None),
            auth_error_headers: RefCell::new(vec![]),
            quiet: Cell::new(false),
            pending: RefCell::new(HashMap::new()),
        })
    }

    /// Setup the necessary works before the first fetch gets started.
    ///
    /// This is a no-op if called more than one time.
    fn start_fetch(&self) -> CargoResult<()> {
        if self.fetch_started.get() {
            // We only need to run the setup code once.
            return Ok(());
        }
        self.fetch_started.set(true);

        // We've enabled the `http2` feature of `curl` in Cargo, so treat
        // failures here as fatal as it would indicate a build-time problem.
        self.multiplexing
            .set(self.gctx.http_config()?.multiplexing.unwrap_or(true));

        if !self.quiet.get() {
            self.gctx
                .shell()
                .status("Updating", self.source_id.display_index())?;
        }

        Ok(())
    }

    /// Constructs the full URL to download a index file.
    fn full_url(&self, path: &Path) -> String {
        // self.url always ends with a slash.
        format!("{}{}", self.url, path.display())
    }

    /// Check if an index file of `path` is up-to-date.
    ///
    /// The `path` argument is the same as in [`RegistryData::load`].
    fn is_fresh(&self, path: &Path) -> bool {
        if !self.requested_update.get() {
            trace!(
                "using local {} as user did not request update",
                path.display()
            );
            true
        } else if self.gctx.cli_unstable().no_index_update {
            trace!("using local {} in no_index_update mode", path.display());
            true
        } else if !self.gctx.network_allowed() {
            trace!("using local {} in offline mode", path.display());
            true
        } else if self.fresh.borrow().contains(path) {
            trace!("using local {} as it was already fetched", path.display());
            true
        } else {
            debug!("checking freshness of {}", path.display());
            false
        }
    }

    /// Get the cached registry configuration, if it exists.
    fn config_cached(&self) -> Option<RegistryConfig> {
        if let Some(cfg) = self.registry_config.borrow().as_ref() {
            return Some(cfg.clone());
        }
        let config_json_path = self
            .assert_index_locked(&self.index_path)
            .join(RegistryConfig::NAME);
        match fs::read(&config_json_path) {
            Ok(raw_data) => match serde_json::from_slice::<RegistryConfig>(&raw_data) {
                Ok(json) => {
                    self.registry_config.replace(Some(json));
                }
                Err(e) => tracing::debug!("failed to decode cached config.json: {}", e),
            },
            Err(e) => {
                if e.kind() != ErrorKind::NotFound {
                    tracing::debug!("failed to read config.json cache: {}", e)
                }
            }
        }
        self.registry_config.borrow().clone()
    }

    /// Get the registry configuration from either cache or remote.
    async fn config(&self) -> CargoResult<RegistryConfig> {
        let Some(config) = self.config_opt().await? else {
            return Err(anyhow::anyhow!("config.json not found"));
        };
        Ok(config)
    }

    /// Get the registry configuration from either cache or remote.
    /// Returns None if the config is not available.
    async fn config_opt(&self) -> CargoResult<Option<RegistryConfig>> {
        debug!("loading config");
        let index_path = self.assert_index_locked(&self.index_path);
        let config_json_path = index_path.join(RegistryConfig::NAME);
        if self.is_fresh(Path::new(RegistryConfig::NAME))
            && let Some(config) = self.config_cached()
        {
            return Ok(Some(config.clone()));
        }

        let response = self.fetch(Path::new(RegistryConfig::NAME), None).await;
        let response = match response {
            Err(e)
                if !self.auth_required.get()
                    && e.downcast_ref::<HttpNotSuccessful>()
                        .map(|e| e.code == 401)
                        .unwrap_or_default() =>
            {
                self.auth_required.set(true);
                self.fresh
                    .borrow_mut()
                    .remove(Path::new(RegistryConfig::NAME));
                self.fetch(Path::new(RegistryConfig::NAME), None).await
            }
            resp => resp,
        }?;

        match response {
            LoadResponse::Data {
                raw_data,
                index_version: _,
            } => {
                trace!("config loaded");
                self.registry_config
                    .replace(Some(serde_json::from_slice(&raw_data)?));
                if paths::create_dir_all(&config_json_path.parent().unwrap()).is_ok() {
                    if let Err(e) = fs::write(&config_json_path, &raw_data) {
                        tracing::debug!("failed to write config.json cache: {}", e);
                    }
                }
                Ok(Some(self.registry_config.borrow().clone().unwrap()))
            }
            LoadResponse::NotFound => Ok(None),
            LoadResponse::CacheValid => Err(crate::util::internal(
                "config.json is never stored in the index cache",
            )),
        }
    }

    async fn fetch(&self, path: &Path, index_version: Option<&str>) -> CargoResult<LoadResponse> {
        if let Some(index_version) = index_version {
            trace!(
                "local cache of {} is available at version `{}`",
                path.display(),
                index_version
            );
            if self.is_fresh(path) {
                return Ok(LoadResponse::CacheValid);
            }
        } else if self.fresh.borrow().contains(path) {
            // We have no cached copy of this file, and we already downloaded it.
            debug!(
                "cache did not contain previously downloaded file {}",
                path.display()
            );
            return Ok(LoadResponse::NotFound);
        }

        if !self.gctx.network_allowed() || self.gctx.cli_unstable().no_index_update {
            // Return NotFound in offline mode when the file doesn't exist in the cache.
            // If this results in resolution failure, the resolver will suggest
            // removing the --offline flag.
            return Ok(LoadResponse::NotFound);
        }

        // Check if this request has already started. If so, return a oneshot that hands out the same data.
        let rx = {
            let mut pending = self.pending.borrow_mut();
            if let Some(waiters) = pending.get_mut(path) {
                let (tx, rx) = oneshot::channel::<CargoResult<LoadResponse>>();
                waiters.push(tx);
                Some(rx)
            } else {
                pending.insert(path.to_path_buf(), Vec::new());
                None
            }
        };
        if let Some(rx) = rx {
            // This is probably really only needed for the config.
            return rx.await?;
        }

        // Check if there's a cached config that says auth is required.
        // This allows avoiding the initial unauthenticated request to probe.
        if !self.auth_required.get()
            && let Some(c) = self.config_cached()
        {
            self.auth_required.set(c.auth_required);
        }

        let mut r = Retry::new(self.gctx)?;
        let response = loop {
            let response = self.fetch_uncached_no_retry(path, index_version).await;
            match r.r#try(|| response) {
                RetryResult::Success(result) => break Ok(result),
                RetryResult::Err(error) => break Err(error),
                RetryResult::Retry(delay_ms) => {
                    futures_timer::Delay::new(Duration::from_millis(delay_ms)).await;
                    self.fresh.borrow_mut().remove(path);
                }
            }
        };
        for entry in self.pending.borrow_mut().remove(path).unwrap() {
            let response = match &response {
                Ok(response) => Ok(response.clone()),
                Err(_) => Err(anyhow::anyhow!("TODO: can't clone errors")),
            };
            let _ = entry.send(response);
        }

        response
    }

    async fn fetch_uncached_no_retry(
        &self,
        path: &Path,
        index_version: Option<&str>,
    ) -> CargoResult<LoadResponse> {
        trace!("load: {}", path.display());
        self.start_fetch()?;
        let full_url = self.full_url(path);
        let mut request = http::Request::get(&full_url);

        // Include a header to identify the protocol. This allows the server to
        // know that Cargo is attempting to use the sparse protocol.
        request = request.header("cargo-protocol", "version=1");
        request = request.header(http::header::ACCEPT, "text/plain");

        // If we have a cached copy of the file, include IF_NONE_MATCH or IF_MODIFIED_SINCE header.
        if let Some(index_version) = index_version {
            if let Some((key, value)) = index_version.split_once(':') {
                match key {
                    ETAG => request = request.header(http::header::IF_NONE_MATCH, value.trim()),
                    LAST_MODIFIED => {
                        request = request.header(http::header::IF_MODIFIED_SINCE, value.trim())
                    }
                    _ => debug!("unexpected index version: {}", index_version),
                }
            }
        }

        if self.auth_required.get() {
            let authorization = auth::auth_token(
                self.gctx,
                &self.source_id,
                self.login_url.borrow().clone().as_ref(),
                Operation::Read,
                self.auth_error_headers.borrow().clone(),
                true,
            )?;
            request = request.header(http::header::AUTHORIZATION, authorization);
            trace!(target: "network", "including authorization for {}", full_url);
        }

        let response = self
            .gctx
            .http_async(request.body(Vec::new())?)
            .await
            .with_context(|| format!("download of {} failed", path.display()))?;

        self.tick()?;

        let is_new = self.fresh.borrow_mut().insert(path.to_path_buf());
        assert!(
            is_new,
            "downloaded the index file `{}` twice",
            path.display()
        );
        let (response, body) = response.into_parts();

        match response.status {
            http::StatusCode::OK => {
                let response_index_version =
                    if let Some(etag) = response.headers.get(http::header::ETAG) {
                        format!("{}: {}", ETAG, etag.to_str().unwrap())
                    } else if let Some(lm) = response.headers.get(http::header::LAST_MODIFIED) {
                        format!("{}: {}", LAST_MODIFIED, lm.to_str().unwrap())
                    } else {
                        UNKNOWN.to_string()
                    };
                trace!("index file version: {}", response_index_version);
                Ok(LoadResponse::Data {
                    raw_data: body,
                    index_version: Some(response_index_version),
                })
            }
            http::StatusCode::NOT_MODIFIED => {
                // Not Modified: the data in the cache is still the latest.
                if index_version.is_none() {
                    return Err(anyhow::anyhow!(
                        "server said not modified (HTTP 304) when no local cache exists"
                    ));
                }
                Ok(LoadResponse::CacheValid)
            }
            http::StatusCode::NOT_FOUND => {
                // The crate was not found or deleted from the registry.
                return Ok(LoadResponse::NotFound);
            }
            http::StatusCode::UNAUTHORIZED => {
                // Store the headers for later error reporting if needed.
                self.auth_error_headers.replace(
                    response
                        .headers
                        .iter()
                        .map(|(name, value)| {
                            format!("{}: {}", name.as_str(), value.to_str().unwrap_or_default())
                        })
                        .collect(),
                );

                // Look for a `www-authenticate` header with the `Cargo` scheme.
                for value in &response.headers.get_all(http::header::WWW_AUTHENTICATE) {
                    for challenge in
                        http_auth::ChallengeParser::new(value.to_str().unwrap_or_default())
                    {
                        match challenge {
                            Ok(challenge) if challenge.scheme.eq_ignore_ascii_case("Cargo") => {
                                // Look for the `login_url` parameter.
                                for (param, value) in challenge.params {
                                    if param.eq_ignore_ascii_case("login_url") {
                                        self.login_url
                                            .replace(Some(value.to_unescaped().into_url()?));
                                    }
                                }
                            }
                            Ok(challenge) => {
                                debug!(target: "network", "ignoring non-Cargo challenge: {}", challenge.scheme)
                            }
                            Err(e) => {
                                debug!(target: "network", "failed to parse challenge: {}", e)
                            }
                        }
                    }
                }

                let mut err = Err(HttpNotSuccessful {
                    code: 401,
                    body: body,
                    url: self.full_url(path),
                    ip: None,
                    headers: response
                        .headers
                        .iter()
                        .map(|(k, v)| format!("{}: {}", k, v.to_str().unwrap_or_default()))
                        .collect(),
                }
                .into());
                if self.auth_required.get() {
                    let auth_error = auth::AuthorizationError::new(
                        self.gctx,
                        self.source_id,
                        self.login_url.borrow().clone(),
                        auth::AuthorizationErrorReason::TokenRejected,
                    )?;
                    err = err.context(auth_error)
                }
                err
            }
            code => Err(HttpNotSuccessful {
                code: code.as_u16() as u32,
                body: body,
                url: self.full_url(path),
                ip: response.client_ip().map(str::to_owned),
                headers: response
                    .headers
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, v.to_str().unwrap_or_default()))
                    .collect(),
            }
            .into()),
        }
    }

    /// Updates the state of the progress bar for downloads.
    fn tick(&self) -> CargoResult<()> {
        let mut progress = self.progress.borrow_mut();
        let Some(progress) = progress.as_mut() else {
            return Ok(());
        };

        // Since the sparse protocol discovers dependencies as it goes,
        // it's not possible to get an accurate progress indication.
        //
        // As an approximation, we assume that the depth of the dependency graph
        // is fixed, and base the progress on how many times the caller has asked
        // for blocking. If there are actually additional dependencies, the progress
        // bar will get stuck. If there are fewer dependencies, it will disappear
        // early. It will never go backwards.
        //
        // The status text also contains the number of completed & pending requests, which
        // gives an better indication of forward progress.
        let approximate_tree_depth = 10;

        progress.tick(
            0.min(approximate_tree_depth),
            approximate_tree_depth + 1,
            &format!(
                " {} complete; {} pending",
                self.fresh.borrow().len(),
                self.pending.borrow().len(),
            ),
        )
    }
}

impl<'gctx> RegistryData for HttpRegistry<'gctx> {
    fn prepare(&self) -> CargoResult<()> {
        self.gctx
            .deferred_global_last_use()?
            .mark_registry_index_used(global_cache_tracker::RegistryIndex {
                encoded_registry_name: self.name,
            });
        Ok(())
    }

    fn index_path(&self) -> &Filesystem {
        &self.index_path
    }

    fn cache_path(&self) -> &Filesystem {
        &self.cache_path
    }

    fn assert_index_locked<'a>(&self, path: &'a Filesystem) -> &'a Path {
        self.gctx
            .assert_package_cache_locked(CacheLockMode::DownloadExclusive, path)
    }

    fn is_updated(&self) -> bool {
        self.requested_update.get()
    }

    async fn load(
        &self,
        _root: &Path,
        path: &Path,
        index_version: Option<&str>,
    ) -> CargoResult<LoadResponse> {
        let Some(config) = self.config_opt().await? else {
            return Ok(LoadResponse::NotFound);
        };
        self.auth_required
            .set(self.auth_required.get() || config.auth_required);
        self.fetch(path, index_version).await
    }

    async fn config(&self) -> CargoResult<Option<RegistryConfig>> {
        Ok(Some(self.config().await?))
    }

    fn invalidate_cache(&self) {
        // Actually updating the index is more or less a no-op for this implementation.
        // All it does is ensure that a subsequent load will double-check files with the
        // server rather than rely on a locally cached copy of the index files.
        debug!("invalidated index cache");
        self.fresh.borrow_mut().clear();
        self.requested_update.set(true);
    }

    fn set_quiet(&mut self, quiet: bool) {
        self.quiet.set(quiet);
        self.progress.replace(None);
    }

    fn download(&self, pkg: PackageId, checksum: &str) -> CargoResult<MaybeLock> {
        let registry_config = futures::executor::block_on(self.config())?;
        download::download(
            &self.cache_path,
            &self.gctx,
            self.name.clone(),
            pkg,
            checksum,
            registry_config,
        )
    }

    fn finish_download(&self, pkg: PackageId, checksum: &str, data: &[u8]) -> CargoResult<File> {
        download::finish_download(
            &self.cache_path,
            &self.gctx,
            self.name.clone(),
            pkg,
            checksum,
            data,
        )
    }

    fn is_crate_downloaded(&self, pkg: PackageId) -> bool {
        download::is_crate_downloaded(&self.cache_path, &self.gctx, pkg)
    }
}
