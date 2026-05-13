use crate::core::global_cache_tracker;
use crate::core::{PackageId, SourceId};
use crate::sources::registry::download;
use crate::sources::registry::{LoadResponse, MaybeLock, RegistryConfig, RegistryData};
use crate::util::cache_lock::CacheLockMode;
use crate::util::interning::InternedString;
use crate::util::{CargoResult, Filesystem, GlobalContext};
use anyhow::Context as _;
use cargo_util::paths;
use std::fs::{self, File};
use std::io::ErrorKind;
use std::path::Path;
use std::task::Poll;
use url::{Position, Url};
use wasip2::http::outgoing_handler;
use wasip2::http::types::{Fields, IncomingBody, Method, OutgoingBody, OutgoingRequest, Scheme};
use wasip2::io::streams::StreamError;

const ETAG: &str = "etag";
const LAST_MODIFIED: &str = "last-modified";
const IF_NONE_MATCH: &str = "if-none-match";
const IF_MODIFIED_SINCE: &str = "if-modified-since";
const UNKNOWN: &str = "Unknown";

pub struct HttpRegistry<'gctx> {
    name: InternedString,
    index_path: Filesystem,
    cache_path: Filesystem,
    gctx: &'gctx GlobalContext,
    url: Url,
    requested_update: bool,
    registry_config: Option<RegistryConfig>,
    quiet: bool,
}

struct HttpResponse {
    status: u16,
    headers: Vec<(String, Vec<u8>)>,
    body: Vec<u8>,
}

impl<'gctx> HttpRegistry<'gctx> {
    pub fn new(
        source_id: SourceId,
        gctx: &'gctx GlobalContext,
        name: &str,
    ) -> CargoResult<HttpRegistry<'gctx>> {
        let url = source_id.url().as_str();
        if !url.ends_with('/') {
            anyhow::bail!("sparse registry url must end in a slash `/`: {url}")
        }
        assert!(source_id.is_sparse());
        let url = Url::parse(
            url.strip_prefix("sparse+")
                .expect("sparse registry needs sparse+ prefix"),
        )?;

        Ok(HttpRegistry {
            name: name.into(),
            index_path: gctx.registry_index_path().join(name),
            cache_path: gctx.registry_cache_path().join(name),
            gctx,
            url,
            requested_update: false,
            registry_config: None,
            quiet: false,
        })
    }

    fn is_fresh(&self, path: &Path, index_version: Option<&str>) -> bool {
        if index_version.is_none() {
            return false;
        }
        if !self.requested_update {
            tracing::trace!(
                "using local {} as user did not request update",
                path.display()
            );
            return true;
        }
        if self.gctx.cli_unstable().no_index_update || !self.gctx.network_allowed() {
            tracing::trace!("using local {} without network", path.display());
            return true;
        }
        false
    }

    fn full_url(&self, path: &Path) -> CargoResult<Url> {
        let path = path
            .to_str()
            .with_context(|| format!("registry path is not UTF-8: {}", path.display()))?;
        self.url
            .join(path)
            .with_context(|| format!("failed to build registry URL for `{path}`"))
    }

    fn load_registry_config(&mut self) -> CargoResult<RegistryConfig> {
        if let Some(config) = &self.registry_config {
            return Ok(config.clone());
        }

        let config_path = self
            .assert_index_locked(&self.index_path)
            .join(RegistryConfig::NAME);
        if !self.requested_update {
            match fs::read(&config_path) {
                Ok(raw_data) => match serde_json::from_slice(&raw_data) {
                    Ok(config) => {
                        self.registry_config = Some(config);
                        return Ok(self.registry_config.as_ref().unwrap().clone());
                    }
                    Err(e) => tracing::debug!("failed to decode cached config.json: {}", e),
                },
                Err(e) => {
                    if e.kind() != ErrorKind::NotFound {
                        tracing::debug!("failed to read config.json cache: {}", e)
                    }
                }
            }
        }

        if !self.gctx.network_allowed() || self.gctx.cli_unstable().no_index_update {
            anyhow::bail!("config.json not found in registry cache")
        }

        let response = self.fetch_index_path(Path::new(RegistryConfig::NAME), Vec::new())?;
        match response.status {
            200 => {
                let config = serde_json::from_slice(&response.body)?;
                self.registry_config = Some(config);
                if paths::create_dir_all(config_path.parent().unwrap()).is_ok() {
                    if let Err(e) = fs::write(&config_path, &response.body) {
                        tracing::debug!("failed to write config.json cache: {}", e);
                    }
                }
                Ok(self.registry_config.as_ref().unwrap().clone())
            }
            404 | 410 | 451 => anyhow::bail!("config.json not found in registry"),
            status => anyhow::bail!(
                "failed to fetch registry config from `{}`: HTTP status {}",
                self.full_url(Path::new(RegistryConfig::NAME))?,
                status
            ),
        }
    }

    fn fetch_index_path(
        &self,
        path: &Path,
        headers: Vec<(String, String)>,
    ) -> CargoResult<HttpResponse> {
        let url = self.full_url(path)?;
        self.fetch_url(&url, headers)
            .with_context(|| format!("failed to download `{}`", url))
    }

    fn fetch_url(&self, url: &Url, headers: Vec<(String, String)>) -> CargoResult<HttpResponse> {
        let entries = headers
            .iter()
            .map(|(name, value)| (name.clone(), value.as_bytes().to_vec()))
            .collect::<Vec<_>>();
        let fields = Fields::from_list(&entries)
            .map_err(|e| anyhow::anyhow!("WASI HTTP rejected request headers: {e:?}"))?;
        let request = OutgoingRequest::new(fields);
        request
            .set_method(&Method::Get)
            .map_err(|()| anyhow::anyhow!("WASI HTTP rejected GET method"))?;
        let scheme = match url.scheme() {
            "http" => Scheme::Http,
            "https" => Scheme::Https,
            other => Scheme::Other(other.to_string()),
        };
        request
            .set_scheme(Some(&scheme))
            .map_err(|()| anyhow::anyhow!("WASI HTTP rejected URL scheme `{}`", url.scheme()))?;
        let authority = &url[Position::BeforeHost..Position::AfterPort];
        request
            .set_authority(Some(authority))
            .map_err(|()| anyhow::anyhow!("WASI HTTP rejected URL authority `{authority}`"))?;
        let path_with_query = &url[Position::BeforePath..];
        request
            .set_path_with_query(Some(path_with_query))
            .map_err(|()| {
                anyhow::anyhow!("WASI HTTP rejected URL path and query `{path_with_query}`")
            })?;

        let body = request
            .body()
            .map_err(|()| anyhow::anyhow!("failed to create WASI HTTP request body"))?;
        let response = outgoing_handler::handle(request, None)
            .map_err(|e| anyhow::anyhow!("WASI HTTP failed to start `{url}`: {e:?}"))?;
        OutgoingBody::finish(body, None)
            .map_err(|e| anyhow::anyhow!("WASI HTTP failed to finish request body: {e:?}"))?;

        response.subscribe().block();
        let response = response
            .get()
            .ok_or_else(|| anyhow::anyhow!("WASI HTTP response was not ready for `{url}`"))?
            .map_err(|()| anyhow::anyhow!("WASI HTTP response for `{url}` was already consumed"))?
            .map_err(|e| anyhow::anyhow!("WASI HTTP request for `{url}` failed: {e:?}"))?;

        let status = response.status();
        let headers = response.headers().entries();
        let body = response
            .consume()
            .map_err(|()| anyhow::anyhow!("WASI HTTP response body for `{url}` was consumed"))?;
        let body = Self::read_body(body)
            .with_context(|| format!("failed to read WASI HTTP response body from `{url}`"))?;

        Ok(HttpResponse {
            status,
            headers,
            body,
        })
    }

    fn read_body(body: IncomingBody) -> CargoResult<Vec<u8>> {
        let stream = body
            .stream()
            .map_err(|()| anyhow::anyhow!("failed to open WASI HTTP response body stream"))?;
        let mut bytes = Vec::new();
        loop {
            match stream.blocking_read(64 * 1024) {
                Ok(chunk) if chunk.is_empty() => break,
                Ok(chunk) => bytes.extend_from_slice(&chunk),
                Err(StreamError::Closed) => break,
                Err(e) => anyhow::bail!("WASI HTTP body stream failed: {e:?}"),
            }
        }
        drop(stream);

        let trailers = IncomingBody::finish(body);
        trailers.subscribe().block();
        if let Some(result) = trailers.get() {
            result
                .map_err(|()| anyhow::anyhow!("WASI HTTP trailers were already consumed"))?
                .map_err(|e| anyhow::anyhow!("WASI HTTP trailers failed: {e:?}"))?;
        }
        Ok(bytes)
    }

    fn header_value<'a>(headers: &'a [(String, Vec<u8>)], name: &str) -> Option<&'a [u8]> {
        headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_slice())
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

    fn load(
        &mut self,
        _root: &Path,
        path: &Path,
        index_version: Option<&str>,
    ) -> Poll<CargoResult<LoadResponse>> {
        tracing::trace!("load: {}", path.display());
        if self.is_fresh(path, index_version) {
            return Poll::Ready(Ok(LoadResponse::CacheValid));
        }
        if !self.gctx.network_allowed() || self.gctx.cli_unstable().no_index_update {
            return Poll::Ready(Ok(LoadResponse::NotFound));
        }

        let mut headers = vec![
            ("cargo-protocol".to_string(), "version=1".to_string()),
            ("accept".to_string(), "text/plain".to_string()),
        ];
        if let Some(index_version) = index_version {
            if let Some((key, value)) = index_version.split_once(':') {
                match key {
                    ETAG => headers.push((IF_NONE_MATCH.to_string(), value.trim().to_string())),
                    LAST_MODIFIED => {
                        headers.push((IF_MODIFIED_SINCE.to_string(), value.trim().to_string()))
                    }
                    _ => tracing::debug!("unexpected index version: {}", index_version),
                }
            }
        }

        let response = match self.fetch_index_path(path, headers) {
            Ok(response) => response,
            Err(e) => return Poll::Ready(Err(e)),
        };

        let response = match response.status {
            200 => {
                let index_version = if let Some(etag) = Self::header_value(&response.headers, ETAG)
                {
                    format!("{}: {}", ETAG, String::from_utf8_lossy(etag))
                } else if let Some(last_modified) =
                    Self::header_value(&response.headers, LAST_MODIFIED)
                {
                    format!(
                        "{}: {}",
                        LAST_MODIFIED,
                        String::from_utf8_lossy(last_modified)
                    )
                } else {
                    UNKNOWN.to_string()
                };
                LoadResponse::Data {
                    raw_data: response.body,
                    index_version: Some(index_version),
                }
            }
            304 if index_version.is_some() => LoadResponse::CacheValid,
            304 => {
                return Poll::Ready(Err(anyhow::anyhow!(
                    "server said not modified (HTTP 304) when no local cache exists"
                )));
            }
            404 | 410 | 451 => LoadResponse::NotFound,
            status => {
                return Poll::Ready(Err(anyhow::anyhow!(
                    "failed to fetch registry index `{}`: HTTP status {}",
                    self.full_url(path)
                        .map(|url| url.to_string())
                        .unwrap_or_else(|_| path.display().to_string()),
                    status
                )));
            }
        };

        Poll::Ready(Ok(response))
    }

    fn config(&mut self) -> Poll<CargoResult<Option<RegistryConfig>>> {
        Poll::Ready(self.load_registry_config().map(Some))
    }

    fn invalidate_cache(&mut self) {
        self.requested_update = true;
    }

    fn set_quiet(&mut self, quiet: bool) {
        self.quiet = quiet;
    }

    fn is_updated(&self) -> bool {
        self.requested_update
    }

    fn download(&mut self, pkg: PackageId, checksum: &str) -> CargoResult<MaybeLock> {
        let config = self.load_registry_config()?;
        match download::download(
            &self.cache_path,
            self.gctx,
            self.name,
            pkg,
            checksum,
            config,
        )? {
            ready @ MaybeLock::Ready(_) => Ok(ready),
            MaybeLock::Download {
                url,
                descriptor,
                authorization,
            } => {
                if !self.quiet {
                    self.gctx.shell().status("Downloading", &descriptor)?;
                }
                let mut headers = Vec::new();
                if let Some(authorization) = authorization {
                    headers.push(("authorization".to_string(), authorization));
                }
                let url = Url::parse(&url)?;
                let response = self.fetch_url(&url, headers)?;
                if response.status != 200 {
                    anyhow::bail!(
                        "failed to download `{}` from `{}`: HTTP status {}",
                        pkg,
                        url,
                        response.status
                    );
                }
                let file = download::finish_download(
                    &self.cache_path,
                    self.gctx,
                    self.name,
                    pkg,
                    checksum,
                    &response.body,
                )?;
                Ok(MaybeLock::Ready(file))
            }
        }
    }

    fn finish_download(
        &mut self,
        pkg: PackageId,
        checksum: &str,
        data: &[u8],
    ) -> CargoResult<File> {
        download::finish_download(&self.cache_path, self.gctx, self.name, pkg, checksum, data)
    }

    fn is_crate_downloaded(&self, pkg: PackageId) -> bool {
        download::is_crate_downloaded(&self.cache_path, self.gctx, pkg)
    }

    fn assert_index_locked<'a>(&self, path: &'a Filesystem) -> &'a Path {
        self.gctx
            .assert_package_cache_locked(CacheLockMode::DownloadExclusive, path)
    }

    fn block_until_ready(&mut self) -> CargoResult<()> {
        Ok(())
    }
}
