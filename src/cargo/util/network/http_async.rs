//! Async wrapper around cURL for making managing HTTP requests.
//!
//! Requests are executed in parallel using cURL [`Multi`] on
//! a worker thread that is owned by the Client.
//!
//! In general, use the [`GlobalContext::http_async`] helper method
//! which holds a global [`Client`] rather than using this module
//! directly.

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use anyhow::{Context, bail};
use curl::MultiError;
use curl::easy::WriteError;
use curl::easy::{Easy2, Handler, InfoType, SslOpt, SslVersion};
use curl::multi::{Easy2Handle, Multi};

use futures::channel::oneshot;
use tracing::{debug, error, trace};

use crate::CargoResult;
use crate::GlobalContext;
use crate::util::network::http::define_configure_http_handle;

type Response = http::Response<Vec<u8>>;
type Request = http::Request<Vec<u8>>;

struct Message {
    easy: Easy2<Collector>,
    sender: oneshot::Sender<CargoResult<Response>>,
}

/// HTTP Client. Creating a new client spawns a cURL `Multi` and
/// thread that is used for all HTTP requests by this client.
pub struct Client {
    multiplexing: bool,
    channel: Option<Sender<Message>>,
    handle: Option<JoinHandle<()>>,
}

impl Client {
    /// Spawns a new worker thread where HTTP request execute.
    pub fn new(gctx: &GlobalContext) -> CargoResult<Client> {
        let (tx, rx) = mpsc::channel();
        let multiplexing = gctx.http_config()?.multiplexing.unwrap_or(true);
        let handle = std::thread::spawn(move || WorkerServer::run(rx, multiplexing));
        Ok(Client {
            multiplexing,
            channel: Some(tx),
            handle: Some(handle),
        })
    }

    /// Perform an HTTP request using this client.
    pub async fn request(&self, gctx: &GlobalContext, request: Request) -> CargoResult<Response> {
        let url = request.uri().to_string();
        debug!(target: "network::fetch", url);
        if let Some(offline_flag) = gctx.offline_flag() {
            bail!("attempting to make an HTTP request, but {offline_flag} was specified")
        }
        let mut collector = Collector::new();
        let (parts, body) = request.into_parts();
        collector.request_body = body;

        let mut handle = curl::easy::Easy2::new(collector);
        crate::try_old_curl_http2_pipewait!(self.multiplexing, handle);
        let timeout = configure_http_handle(gctx, &mut handle)?;
        timeout.configure2(&mut handle)?;

        handle.url(&url)?;
        handle.follow_location(true)?;

        match parts.method {
            http::Method::GET => handle.get(true)?,
            http::Method::POST => handle.post(true)?,
            http::Method::PUT => handle.put(true)?,
            method => handle.custom_request(method.as_str())?,
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
        handle.http_headers(headers)?;

        let (sender, receiver) = oneshot::channel();
        let req = Message {
            easy: handle,
            sender,
        };

        self.channel
            .as_ref()
            .unwrap()
            .send(req)
            .map_err(|e| crate::util::internal(format!("async http tx channel dead: {e}")))?;
        receiver
            .await
            .map_err(|e| crate::util::internal(format!("async http rx channel dead: {e}")))?
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        // Close the channel
        drop(self.channel.take().unwrap());
        // Join the thread
        let _ = self.handle.take().unwrap().join();
    }
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("http_async::Client").finish()
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
            oneshot::Sender<CargoResult<Response>>,
        ),
    >,
    token: usize,
}

impl WorkerServer {
    fn run(incoming_work: Receiver<Message>, multiplex: bool) {
        let mut multi = Multi::new();
        // let's not flood the server with connections
        if let Err(e) = multi.set_max_host_connections(2) {
            error!("failed to set max host connections in curl: {e}");
        }
        if let Err(e) = multi.pipelining(false, multiplex) {
            error!("failed to enable multiplexing/pipelining in curl: {e}");
        }

        let mut worker = Self {
            incoming_work,
            multi,
            handles: HashMap::new(),
            token: 0,
        };
        worker.worker_loop();
    }

    fn fail_and_drain(&mut self, e: MultiError, context: &'static str) {
        for (_token, (_handle, sender)) in self.handles.drain() {
            let _ = sender.send(CargoResult::Err(e.clone().into()).context(context));
        }
    }

    fn worker_loop(&mut self) {
        const INITIAL_DELAY: Duration = Duration::from_millis(1);
        let mut wait_backoff = INITIAL_DELAY;
        loop {
            // Start any pending work.
            while let Ok(msg) = self.incoming_work.try_recv() {
                self.enqueue_request(msg);
                wait_backoff = INITIAL_DELAY;
            }

            match self.multi.perform() {
                Err(e) if e.is_call_perform() => {
                    // cURL states if you receive `is_call_perform`, this means that you should call `perform` again.
                }
                Err(e) => {
                    self.fail_and_drain(e, "failed to execute `Multi::perform`");
                }
                Ok(running) => {
                    self.multi.messages(|msg| {
                        let t = msg.token().expect("all handles have tokens");
                        trace!(token = t, "finish");
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
                        // Would be nice to set HTTP version via `response.version_mut()`, but `curl` doesn't have it exposed.
                        let extensions = Extensions {
                            client_ip: easy.primary_ip().ok().flatten().map(str::to_string),
                        };
                        response.extensions_mut().insert(extensions);
                        let _ = sender.send(result.map(|()| response).map_err(Into::into));
                    });

                    if running > 0 {
                        let max_timeout = Duration::from_millis(1000);
                        let mut timeout = self
                            .multi
                            .get_timeout()
                            .ok()
                            .flatten()
                            .unwrap_or(max_timeout)
                            .min(max_timeout);
                        if timeout.is_zero() {
                            // curl said not to wait.
                            continue;
                        }
                        // Ideally we would use `Multi::poll` + a `MultiWaker` instead of `Multi::wait`
                        // to wake the thread when new work is queued. But it requires curl 7.68+,
                        // which is not available everywhere we support.
                        //
                        // Instead, we use an exponential backoff approach so that as long as requests
                        // are being queued, we poll quickly to allow the requests to be added sooner.
                        // Without this, we end up setting in `Multi::wait` too long while new work is
                        // added to the channel.
                        //
                        // `get_timeout` says we should wait *at most* the timeout amount, so reducing
                        // the wait time is fine.
                        if wait_backoff < timeout {
                            wait_backoff *= 2;
                            timeout = wait_backoff
                        }
                        trace!(
                            pending = self.handles.len(),
                            timeout = timeout.as_millis(),
                            "curl wait"
                        );
                        if let Err(e) = self.multi.wait(&mut [], timeout) {
                            self.fail_and_drain(e, "failed to execute `Multi::wait`");
                        }
                    } else {
                        // Block, waiting for more work
                        trace!("all work completed");
                        match self.incoming_work.recv() {
                            Ok(msg) => {
                                trace!("resuming work");
                                self.enqueue_request(msg);
                                wait_backoff = INITIAL_DELAY;
                            }
                            Err(_) => {
                                // The sending channel is closed. Shut down the worker.
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    /// Adds the request to the `Multi`, or send an error back through the channel.
    fn enqueue_request(&mut self, message: Message) {
        match self.multi.add2(message.easy) {
            Ok(mut handle) => {
                self.token = self.token.wrapping_add(1);
                handle.set_token(self.token).ok();
                self.handles.insert(self.token, (handle, message.sender));
            }
            Err(e) => {
                let _ = message.sender.send(Err(e.into()));
            }
        }
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
        if let Some((name, value)) = handle_http_header(data)
            && let Ok(name) = http::HeaderName::from_str(name)
            && let Ok(value) = http::HeaderValue::from_str(value)
        {
            self.inner().headers_mut().append(name, value);
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

define_configure_http_handle!(handle: Easy2<Collector>, {handle.get_mut().debug = true});

/// Splits HTTP `HEADER: VALUE` to a tuple.
fn handle_http_header(buf: &[u8]) -> Option<(&str, &str)> {
    if buf.is_empty() {
        return None;
    }
    let buf = std::str::from_utf8(buf).ok()?.trim_end();
    // Don't let server sneak extra lines anywhere.
    if buf.contains('\n') {
        return None;
    }
    let (tag, value) = buf.split_once(':')?;
    let value = value.trim();
    Some((tag, value))
}
