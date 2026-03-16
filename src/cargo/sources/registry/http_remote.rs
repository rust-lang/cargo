//! Access to a HTTP-based crate registry. See [`HttpRegistry`] for details.

use crate::core::PackageId;
use crate::core::SourceId;
use crate::core::global_cache_tracker;
use crate::sources::registry::LoadResponse;
use crate::sources::registry::MaybeLock;
use crate::sources::registry::RegistryConfig;
use crate::sources::registry::RegistryData;
use crate::sources::registry::download;
use crate::util::Filesystem;
use crate::util::GlobalContext;
use crate::util::IntoUrl;
use crate::util::Progress;
use crate::util::ProgressStyle;
use crate::util::auth;
use crate::util::cache_lock::CacheLockMode;
use crate::util::errors::CargoResult;
use crate::util::errors::HttpNotSuccessful;
use crate::util::interning::InternedString;
use crate::util::network::http_async::ResponsePartsExtensions;
use crate::util::network::retry::Retry;
use crate::util::network::retry::RetryResult;
use anyhow::Context as _;
use cargo_credential::Operation;
use cargo_util::paths;
use futures::lock::Mutex;
use http::HeaderName;
use http::HeaderValue;
use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashSet;
use std::fs;
use std::fs::File;
use std::io::ErrorKind;
use std::path::Path;
use std::str;
use std::time::Duration;
use tracing::debug;
use tracing::trace;
use tracing::warn;
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

    /// Cached registry configuration.
    registry_config: Mutex<Option<RegistryConfig>>,

    /// Backend used for making network requests.
    inner: HttpBackend<'gctx>,
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
        Ok(HttpRegistry {
            name: name.into(),
            registry_config: Mutex::new(None),
            inner: HttpBackend::new(source_id, gctx, name)?,
        })
    }

    fn inner(&self) -> &HttpBackend<'gctx> {
        &self.inner
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
        let mut config = self.registry_config.lock().await;
        if let Some(config) = &*config
            && self.inner().is_fresh(RegistryConfig::NAME)
        {
            Ok(Some(config.clone()))
        } else {
            let result = self.config_opt_inner().await?;
            *config = result.clone();
            Ok(result)
        }
    }

    async fn config_opt_inner(&self) -> CargoResult<Option<RegistryConfig>> {
        debug!("loading config");
        let index_path = self.assert_index_locked(&self.inner().index_cache_path);
        let config_json_path = index_path.join(RegistryConfig::NAME);
        if self.inner().is_fresh(RegistryConfig::NAME)
            && let Some(config) = self.config_from_filesystem()
        {
            return Ok(Some(config.clone()));
        }

        // Check if there's a cached config that says auth is required.
        // This allows avoiding the initial unauthenticated request to probe.
        if let Some(c) = self.config_from_filesystem() {
            self.inner().auth_required.update(|v| v || c.auth_required);
        }

        let response = self
            .inner()
            .fetch_uncached(RegistryConfig::NAME, None)
            .await;
        let response = match response {
            Err(e)
                if !self.inner().auth_required.get()
                    && e.downcast_ref::<HttpNotSuccessful>()
                        .map(|e| e.code == 401)
                        .unwrap_or_default() =>
            {
                self.inner().auth_required.set(true);
                debug!(target: "network", "re-attempting request for config.json with authorization included.");
                self.inner()
                    .fetch_uncached(RegistryConfig::NAME, None)
                    .await
            }
            resp => resp,
        }?;

        match response {
            LoadResponse::Data {
                raw_data,
                index_version: _,
            } => {
                trace!("config loaded");
                let config = Some(serde_json::from_slice(&raw_data)?);
                if paths::create_dir_all(&config_json_path.parent().unwrap()).is_ok() {
                    if let Err(e) = fs::write(&config_json_path, &raw_data) {
                        tracing::debug!("failed to write config.json cache: {}", e);
                    }
                }
                Ok(config)
            }
            LoadResponse::NotFound => Ok(None),
            LoadResponse::CacheValid => Err(crate::util::internal(
                "config.json is never stored in the index cache",
            )),
        }
    }

    /// Get the cached registry configuration from the filesystem, if it exists.
    fn config_from_filesystem(&self) -> Option<RegistryConfig> {
        let config_json_path = self
            .assert_index_locked(&self.inner().index_cache_path)
            .join(RegistryConfig::NAME);
        match fs::read(&config_json_path) {
            Ok(raw_data) => match serde_json::from_slice(&raw_data) {
                Ok(json) => return Some(json),
                Err(e) => tracing::debug!("failed to decode cached config.json: {}", e),
            },
            Err(e) => {
                if e.kind() != ErrorKind::NotFound {
                    tracing::debug!("failed to read config.json cache: {}", e)
                }
            }
        }
        None
    }

    async fn sparse_fetch(
        &self,
        path: &str,
        index_version: Option<&str>,
    ) -> CargoResult<LoadResponse> {
        if let Some(index_version) = index_version {
            trace!("local cache of {path} is available at version `{index_version}`",);
            if self.inner().is_fresh(&path) {
                return Ok(LoadResponse::CacheValid);
            }
        } else if self.inner().fresh.borrow().contains(path) {
            // We have no cached copy of this file, and we already downloaded it.
            debug!("cache did not contain previously downloaded file {path}",);
            return Ok(LoadResponse::NotFound);
        }

        // If we have a cached copy of the file, include IF_NONE_MATCH or IF_MODIFIED_SINCE header.
        let index_version =
            index_version
                .and_then(|v| v.split_once(':'))
                .and_then(|(key, value)| match key {
                    ETAG => Some((
                        http::header::IF_NONE_MATCH,
                        HeaderValue::from_str(value.trim()).ok()?,
                    )),
                    LAST_MODIFIED => Some((
                        http::header::IF_MODIFIED_SINCE,
                        HeaderValue::from_str(value.trim()).ok()?,
                    )),
                    _ => {
                        debug!("unexpected index version: {}", index_version.unwrap());
                        None
                    }
                });
        let index_version = index_version.as_ref().map(|(k, v)| (k, v));
        self.inner().fetch_uncached(&path, index_version).await
    }
}

#[async_trait::async_trait(?Send)]
impl<'gctx> RegistryData for HttpRegistry<'gctx> {
    fn prepare(&self) -> CargoResult<()> {
        self.inner()
            .gctx
            .deferred_global_last_use()?
            .mark_registry_index_used(global_cache_tracker::RegistryIndex {
                encoded_registry_name: self.name,
            });
        Ok(())
    }

    fn index_path(&self) -> &Filesystem {
        &self.inner().index_cache_path
    }

    fn cache_path(&self) -> &Filesystem {
        &self.inner().crate_cache_path
    }

    fn assert_index_locked<'a>(&self, path: &'a Filesystem) -> &'a Path {
        self.inner()
            .gctx
            .assert_package_cache_locked(CacheLockMode::DownloadExclusive, path)
    }

    fn is_updated(&self) -> bool {
        self.inner().requested_update.get()
    }

    async fn load(
        &self,
        _root: &Path,
        path: &Path,
        index_version: Option<&str>,
    ) -> CargoResult<LoadResponse> {
        // Ensure the config is loaded.
        let Some(config) = self.config_opt().await? else {
            return Ok(LoadResponse::NotFound);
        };
        self.inner()
            .auth_required
            .update(|v| v || config.auth_required);

        let path = path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("non UTF8 path: {}", path.display()))?;
        self.sparse_fetch(path, index_version).await
    }

    async fn config(&self) -> CargoResult<Option<RegistryConfig>> {
        Ok(Some(self.config().await?))
    }

    fn invalidate_cache(&self) {
        // Actually updating the index is more or less a no-op for this implementation.
        // All it does is ensure that a subsequent load will double-check files with the
        // server rather than rely on a locally cached copy of the index files.
        debug!("invalidated index cache");
        self.inner().fresh.borrow_mut().clear();
        self.inner().requested_update.set(true);
    }

    fn set_quiet(&mut self, quiet: bool) {
        self.inner().quiet.set(quiet);
        self.inner().progress.replace(None);
    }

    fn download(&self, pkg: PackageId, checksum: &str) -> CargoResult<MaybeLock> {
        let registry_config = crate::util::block_on(self.config())?;
        download::download(
            &self.inner().crate_cache_path,
            &self.inner().gctx,
            self.name.clone(),
            pkg,
            checksum,
            registry_config,
        )
    }

    fn finish_download(&self, pkg: PackageId, checksum: &str, data: &[u8]) -> CargoResult<File> {
        download::finish_download(
            &self.inner().crate_cache_path,
            &self.inner().gctx,
            self.name.clone(),
            pkg,
            checksum,
            data,
        )
    }

    fn is_crate_downloaded(&self, pkg: PackageId) -> bool {
        download::is_crate_downloaded(&self.inner().crate_cache_path, &self.inner().gctx, pkg)
    }
}

struct HttpBackend<'gctx> {
    /// Path to the registry index (`$CARGO_HOME/registry/index/$REG-HASH`).
    index_cache_path: Filesystem,

    /// Path to the cache of `.crate` files (`$CARGO_HOME/registry/cache/$REG-HASH`).
    crate_cache_path: Filesystem,

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

    /// Number of in-flight requests.
    pending: Cell<usize>,

    /// What paths have we already fetched since the last index update?
    ///
    /// We do not need to double-check any of these index files since we have already done so.
    fresh: RefCell<HashSet<String>>,

    /// Have we started to download any index files?
    fetch_started: Cell<bool>,

    /// Should we include the authorization header?
    auth_required: Cell<bool>,

    /// Url to get a token for the registry.
    login_url: RefCell<Option<Url>>,

    /// Headers received with an HTTP 401.
    auth_error_headers: RefCell<Vec<String>>,

    /// Disables status messages.
    quiet: Cell<bool>,
}

impl<'gctx> HttpBackend<'gctx> {
    pub fn new(
        source_id: SourceId,
        gctx: &'gctx GlobalContext,
        name: &str,
    ) -> CargoResult<HttpBackend<'gctx>> {
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

        let index_cache_path = gctx.registry_index_path().join(name);
        Ok(HttpBackend {
            index_cache_path: index_cache_path.clone(),
            crate_cache_path: gctx.registry_cache_path().join(name),
            source_id,
            gctx,
            url,
            progress: RefCell::new(Some(Progress::with_style(
                "Fetch",
                ProgressStyle::Indeterminate,
                gctx,
            ))),
            fresh: RefCell::new(HashSet::new()),
            requested_update: Cell::new(false),
            fetch_started: Cell::new(false),
            auth_required: Cell::new(false),
            login_url: RefCell::new(None),
            auth_error_headers: RefCell::new(vec![]),
            quiet: Cell::new(false),
            pending: Cell::new(0),
        })
    }

    /// Constructs the full URL to download a index file.
    fn full_url(&self, path: &str) -> String {
        // self.url always ends with a slash.
        format!("{}{}", self.url, path)
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

        if !self.quiet.get() {
            self.gctx
                .shell()
                .status("Updating", self.source_id.display_index())?;
        }

        Ok(())
    }

    /// Are we in offline mode?
    ///
    /// Return NotFound in offline mode when the file doesn't exist in the cache.
    /// If this results in resolution failure, the resolver will suggest
    /// removing the --offline flag.
    fn offline(&self) -> bool {
        !self.gctx.network_allowed() || self.gctx.cli_unstable().no_index_update
    }

    /// Check if an index file of `path` is up-to-date.
    fn is_fresh(&self, path: &str) -> bool {
        if !self.requested_update.get() {
            trace!("using local {path} as user did not request update",);
            true
        } else if self.offline() {
            trace!("using local {path} in offline mode");
            true
        } else if self.fresh.borrow().contains(path) {
            trace!("using local {path} as it was already fetched");
            true
        } else {
            debug!("checking freshness of {path}");
            false
        }
    }

    async fn fetch_uncached(
        &self,
        path: &str,
        extra_header: Option<(&HeaderName, &HeaderValue)>,
    ) -> CargoResult<LoadResponse> {
        if self.offline() {
            return Ok(LoadResponse::NotFound);
        }

        if !self.fresh.borrow_mut().insert(path.to_string()) {
            warn!("downloaded the index file `{path}` twice");
        }

        let mut r = Retry::new(self.gctx)?;
        self.pending.update(|v| v + 1);
        let response = loop {
            let response = self.fetch_uncached_no_retry(path, extra_header).await;
            match r.r#try(|| response) {
                RetryResult::Success(result) => break Ok(result),
                RetryResult::Err(error) => break Err(error),
                RetryResult::Retry(delay_ms) => {
                    futures_timer::Delay::new(Duration::from_millis(delay_ms)).await;
                }
            }
        };
        self.pending.update(|v| v - 1);
        response
    }

    async fn fetch_uncached_no_retry(
        &self,
        path: &str,
        extra_header: Option<(&HeaderName, &HeaderValue)>,
    ) -> CargoResult<LoadResponse> {
        trace!("load: {path}");
        self.start_fetch()?;
        let full_url = self.full_url(path);
        let mut request = http::Request::get(&full_url);

        // Include a header to identify the protocol. This allows the server to
        // know that Cargo is attempting to use the sparse protocol.
        request = request.header("cargo-protocol", "version=1");
        request = request.header(http::header::ACCEPT, "text/plain");

        if let Some((k, v)) = extra_header {
            request = request.header(k, v);
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
            .http_async()?
            .request(request.body(Vec::new())?)
            .await
            .with_context(|| format!("download of {path} failed"))?;

        self.tick()?;

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
                    code: http::StatusCode::UNAUTHORIZED.as_u16() as u32,
                    body: body,
                    url: full_url,
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
                url: full_url,
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

        if progress.update_allowed() {
            let complete = self.fresh.borrow().len();
            let pending = self.pending.get();
            progress.print_now(&format!("{complete} complete; {pending} pending"))?;
        }
        Ok(())
    }
}
