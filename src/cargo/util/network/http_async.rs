//! Async wrapper around cURL for making managing HTTP requests.
//!
//! Requests are executed in parallel using cURL [`Multi`] on
//! a worker thread that is owned by the Client.
//!
//! In general, use the [`GlobalContext::http_async`] helper method
//! which holds a global [`Client`] rather than using this module
//! directly.

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use anyhow::{Context, bail};
use curl::easy::WriteError;
use curl::easy::{Easy2, Handler, InfoType, SslOpt, SslVersion};
use curl::multi::{Easy2Handle, Multi};

use futures::channel::oneshot;
use tracing::{debug, error};

use crate::util::context::{SslVersionConfig, SslVersionConfigRange};
use crate::util::network::http::HttpTimeout;
use crate::{CargoResult, GlobalContext, version};

type Response = http::Response<Vec<u8>>;
type Request = http::Request<Vec<u8>>;

struct Message {
    request: Request,
    easy: Easy2<Collector>,
    sender: oneshot::Sender<anyhow::Result<Response>>,
}

/// HTTP Client. Creating a new client spawns a cURL `Multi` and
/// thread that is used for all HTTP requests on this client.
pub struct Client {
    channel: Sender<Message>,
}

impl Client {
    /// Spawns a new worker thread where HTTP request execute.
    pub fn new() -> Client {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(|| WorkerServer::run(rx));
        Client { channel: tx }
    }

    pub async fn request(
        &self,
        gctx: &GlobalContext,
        mut request: Request,
    ) -> anyhow::Result<Response> {
        if let Some(offline_flag) = gctx.offline_flag() {
            bail!(
                "attempting to make an HTTP request, but {offline_flag} was \
                specified"
            )
        }
        let (sender, receiver) = oneshot::channel();
        let mut collector = Collector::new();
        std::mem::swap(request.body_mut(), &mut collector.request_body);
        let mut easy = curl::easy::Easy2::new(collector);
        let timeout = configure_http_handle(gctx, &mut easy)?;
        timeout.configure2(&mut easy)?;
        let req = Message {
            request,
            easy,
            sender,
        };
        self.channel
            .send(req)
            .map_err(|e| crate::util::internal(format!("async http tx channel dead: {e}")))?;
        receiver
            .await
            .map_err(|e| crate::util::internal(format!("async http rx channel dead: {e}")))?
    }
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client").finish()
    }
}

/// Additional fields on an [`http::Response`].
#[derive(Clone)]
struct Extensions {
    client_ip: Option<String>,
}

pub trait ResponsePartsExtensions {
    fn client_ip(&self) -> Option<&str>;
}

impl ResponsePartsExtensions for http::response::Parts {
    fn client_ip(&self) -> Option<&str> {
        self.extensions
            .get::<Extensions>()
            .and_then(|extensions| extensions.client_ip.as_deref())
    }
}

impl ResponsePartsExtensions for Response {
    fn client_ip(&self) -> Option<&str> {
        self.extensions()
            .get::<Extensions>()
            .and_then(|extensions| extensions.client_ip.as_deref())
    }
}

/// Manages the cURL `Multi`. Processes incoming work sent over the
/// channel, and returns responses.
struct WorkerServer {
    incoming_work: Receiver<Message>,
    multi: Multi,
    handles: HashMap<
        usize,
        (
            Easy2Handle<Collector>,
            oneshot::Sender<anyhow::Result<Response>>,
        ),
    >,
    token: usize,
}

impl WorkerServer {
    fn run(incoming_work: Receiver<Message>) {
        let mut worker = Self {
            incoming_work,
            multi: Multi::new(),
            handles: HashMap::new(),
            token: 0,
        };
        worker.worker_loop();
    }

    fn worker_loop(&mut self) {
        loop {
            // Start any pending work.
            while let Ok(msg) = self.incoming_work.try_recv() {
                self.enqueue_request(msg);
            }

            match self.multi.perform() {
                Err(e) if e.is_call_perform() => {
                    // cURL states if you receive `is_call_perform`, this means that you should call `perform` again.
                }
                Err(e) => {
                    error!("cURL multi error: {e}");
                    // Send error to all in-progress requests.
                    for (_token, (_handle, sender)) in self.handles.drain() {
                        let _ = sender.send(CargoResult::Err(e.clone().into()));
                    }
                }
                Ok(running) => {
                    self.multi.messages(|msg| {
                        let t = msg.token().expect("all handles have tokens");
                        let Some((handle, sender)) = self.handles.remove(&t) else {
                            error!("missing entry {t} in handle table");
                            return;
                        };
                        let result = msg.result_for2(&handle).expect("handle must have a result");
                        let mut easy = self.multi.remove2(handle).expect("handle must be in multi");
                        let mut response = easy
                            .get_mut()
                            .response
                            .take()
                            .expect("requests only finish once");
                        if let Ok(status) = easy.response_code()
                            && status != 0
                            && let Ok(status) = http::StatusCode::from_u16(status as u16)
                        {
                            *response.status_mut() = status;
                        }

                        // Would be nice to set this, but `curl-sys` doesn't have it exposed yet.
                        response.version_mut();

                        let extensions = Extensions {
                            client_ip: easy.primary_ip().ok().flatten().map(str::to_string),
                        };
                        response.extensions_mut().insert(extensions);

                        let _ = sender.send(result.map(|()| response).map_err(Into::into));
                    });

                    if running > 0 {
                        self.multi.wait(&mut [], Duration::from_secs(1)).unwrap();
                    } else if running == 0 {
                        // Block, waiting for more work
                        match self.incoming_work.recv() {
                            Ok(msg) => {
                                self.enqueue_request(msg);
                            }
                            Err(_) => {
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    fn enqueue_request(&mut self, message: Message) {
        debug!(target: "network::fetch", url = message.request.uri().to_string());

        match self.enqueue_request_inner(message.request, message.easy) {
            Ok(mut handle) => {
                self.token = self.token.wrapping_add(1);
                handle.set_token(self.token).ok();
                self.handles.insert(self.token, (handle, message.sender));
            }
            Err(e) => {
                let _ = message.sender.send(Err(e));
            }
        }
    }

    fn enqueue_request_inner(
        &mut self,
        req: Request,
        mut easy: Easy2<Collector>,
    ) -> anyhow::Result<Easy2Handle<Collector>> {
        let (parts, _) = req.into_parts();
        easy.url(&parts.uri.to_string())?;
        easy.follow_location(true)?;

        match parts.method {
            http::Method::GET => easy.get(true)?,
            http::Method::POST => easy.post(true)?,
            http::Method::PUT => easy.put(true)?,
            method => easy.custom_request(method.as_str())?,
        }

        let mut headers = curl::easy::List::new();
        for (name, value) in parts.headers {
            if let Some(name) = name {
                let value = value
                    .to_str()
                    .with_context(|| format!("invalid header value: {:?}", value))?;
                headers.append(&format!("{}: {}", name, value))?;
            }
        }
        easy.http_headers(headers)?;
        Ok(self.multi.add2(easy)?)
    }
}

/// Interface that cURL (`Easy2`) uses to make progress.
struct Collector {
    response: Option<Response>,
    request_body: Vec<u8>,
    debug: bool,
}

impl Collector {
    fn new() -> Self {
        Collector {
            response: Some(Response::new(Vec::new())),
            request_body: Vec::new(),
            debug: false,
        }
    }

    fn inner(&mut self) -> &mut Response {
        self.response.as_mut().unwrap()
    }
}

impl Handler for Collector {
    fn write(&mut self, data: &[u8]) -> Result<usize, WriteError> {
        self.inner().body_mut().extend_from_slice(data);
        Ok(data.len())
    }

    fn header(&mut self, data: &[u8]) -> bool {
        if let Some(pos) = data.iter().position(|&c| c == b':') {
            // Change to `split_once`, once stable.
            let (name, mut value) = (&data[..pos], &data[pos + 1..]);
            if let Ok(name) = http::HeaderName::from_bytes(name.trim_ascii()) {
                if value.ends_with(b"\r\n") {
                    value = &value[..value.len() - 2]
                } else if value.ends_with(b"\n") {
                    value = &value[..value.len() - 1]
                }
                if let Ok(value) = http::HeaderValue::from_bytes(value.trim_ascii_start()) {
                    let map = self.inner().headers_mut();
                    map.append(name, value);
                }
            }
        }
        true
    }

    fn read(&mut self, data: &mut [u8]) -> Result<usize, curl::easy::ReadError> {
        let count = std::cmp::min(self.request_body.len(), data.len());
        data[..count].copy_from_slice(&self.request_body[..count]);
        self.request_body.drain(..count);
        Ok(count)
    }

    fn debug(&mut self, kind: InfoType, data: &[u8]) {
        if self.debug {
            super::http::debug(kind, data);
        }
    }

    fn progress(&mut self, _dltotal: f64, _dlnow: f64, _ultotal: f64, _ulnow: f64) -> bool {
        true
    }
}

/// Configure a libcurl http handle with the defaults options for Cargo
/// Note: keep in sync with `http::configure_http_handle`.
fn configure_http_handle(
    gctx: &GlobalContext,
    handle: &mut Easy2<Collector>,
) -> CargoResult<HttpTimeout> {
    let http = gctx.http_config()?;
    if let Some(proxy) = super::proxy::http_proxy(http) {
        handle.proxy(&proxy)?;
    }
    if let Some(cainfo) = &http.cainfo {
        let cainfo = cainfo.resolve_path(gctx);
        handle.cainfo(&cainfo)?;
    }
    // Use `proxy_cainfo` if explicitly set; otherwise, fall back to `cainfo` as curl does #15376.
    if let Some(proxy_cainfo) = http.proxy_cainfo.as_ref().or(http.cainfo.as_ref()) {
        let proxy_cainfo = proxy_cainfo.resolve_path(gctx);
        handle.proxy_cainfo(&format!("{}", proxy_cainfo.display()))?;
    }
    if let Some(check) = http.check_revoke {
        handle.ssl_options(SslOpt::new().no_revoke(!check))?;
    }

    if let Some(user_agent) = &http.user_agent {
        handle.useragent(user_agent)?;
    } else {
        handle.useragent(&format!("cargo/{}", version()))?;
    }

    fn to_ssl_version(s: &str) -> CargoResult<SslVersion> {
        let version = match s {
            "default" => SslVersion::Default,
            "tlsv1" => SslVersion::Tlsv1,
            "tlsv1.0" => SslVersion::Tlsv10,
            "tlsv1.1" => SslVersion::Tlsv11,
            "tlsv1.2" => SslVersion::Tlsv12,
            "tlsv1.3" => SslVersion::Tlsv13,
            _ => bail!(
                "Invalid ssl version `{s}`,\
                 choose from 'default', 'tlsv1', 'tlsv1.0', 'tlsv1.1', 'tlsv1.2', 'tlsv1.3'."
            ),
        };
        Ok(version)
    }

    // Empty string accept encoding expands to the encodings supported by the current libcurl.
    handle.accept_encoding("")?;
    if let Some(ssl_version) = &http.ssl_version {
        match ssl_version {
            SslVersionConfig::Single(s) => {
                let version = to_ssl_version(s.as_str())?;
                handle.ssl_version(version)?;
            }
            SslVersionConfig::Range(SslVersionConfigRange { min, max }) => {
                let min_version = min
                    .as_ref()
                    .map_or(Ok(SslVersion::Default), |s| to_ssl_version(s))?;
                let max_version = max
                    .as_ref()
                    .map_or(Ok(SslVersion::Default), |s| to_ssl_version(s))?;
                handle.ssl_min_max_version(min_version, max_version)?;
            }
        }
    } else if cfg!(windows) {
        // This is a temporary workaround for some bugs with libcurl and
        // schannel and TLS 1.3.
        //
        // Our libcurl on Windows is usually built with schannel.
        // On Windows 11 (or Windows Server 2022), libcurl recently (late
        // 2022) gained support for TLS 1.3 with schannel, and it now defaults
        // to 1.3. Unfortunately there have been some bugs with this.
        // https://github.com/curl/curl/issues/9431 is the most recent. Once
        // that has been fixed, and some time has passed where we can be more
        // confident that the 1.3 support won't cause issues, this can be
        // removed.
        //
        // Windows 10 is unaffected. libcurl does not support TLS 1.3 on
        // Windows 10. (Windows 10 sorta had support, but it required enabling
        // an advanced option in the registry which was buggy, and libcurl
        // does runtime checks to prevent it.)
        handle.ssl_min_max_version(SslVersion::Default, SslVersion::Tlsv12)?;
    }

    if let Some(true) = http.debug {
        handle.verbose(true)?;
        tracing::debug!(target: "network", "{:#?}", curl::Version::get());
        handle.get_mut().debug = true;
    }

    HttpTimeout::new(gctx)
}
