//! Async wrapper around cURL for making managing HTTP requests.
//!
//! Requests are executed in parallel using cURL [`Multi`] on
//! a worker thread that is owned by the Client.

use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::str::FromStr;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use curl::easy::WriteError;
use curl::easy::{Easy2, Handler, InfoType};
use curl::multi::{Easy2Handle, Multi};

use crate::util::network::http::HandleConfiguration;
use futures::channel::oneshot;
use tracing::{debug, error, trace};

type Response = http::Response<Vec<u8>>;
type Request = http::Request<Vec<u8>>;
type HttpResult<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error(transparent)]
    Multi(#[from] curl::MultiError),

    #[error(transparent)]
    Easy(#[from] curl::Error),

    #[error("failed to convert header value of `{name}` to string: {bytes:?}")]
    BadHeader { name: String, bytes: Vec<u8> },
}

struct Message {
    easy: Easy2<Collector>,
    sender: oneshot::Sender<HttpResult<Response>>,
}

/// HTTP Client. Creating a new client spawns a cURL `Multi` and
/// thread that is used for all HTTP requests by this client.
pub struct Client {
    channel: Option<Sender<Message>>,
    thread_handle: Option<JoinHandle<()>>,
    handle_config: HandleConfiguration,
}

impl Client {
    /// Spawns a new worker thread where HTTP request execute.
    pub fn new(handle_config: HandleConfiguration) -> Client {
        let (tx, rx) = mpsc::channel();
        let handle = std::thread::spawn(move || WorkerServer::run(rx, handle_config.multiplexing));
        Client {
            channel: Some(tx),
            thread_handle: Some(handle),
            handle_config,
        }
    }

    /// Perform an HTTP request using this client.
    pub async fn request(&self, request: Request) -> HttpResult<Response> {
        let url = request.uri().to_string();
        debug!(target: "network::fetch", url);
        let mut collector = Collector::new();
        let (parts, body) = request.into_parts();
        let body_len = body.len();
        collector.request_body = Cursor::new(body);
        collector.debug = self.handle_config.verbose;
        let mut handle = curl::easy::Easy2::new(collector);
        self.handle_config.configure2(&mut handle)?;

        handle.url(&url)?;
        handle.follow_location(true)?;

        match parts.method {
            http::Method::HEAD => handle.nobody(true)?,
            http::Method::GET => handle.get(true)?,
            http::Method::POST => {
                handle.post_field_size(body_len as u64)?;
                handle.post(true)?;
            }
            http::Method::PUT => {
                handle.in_filesize(body_len as u64)?;
                handle.put(true)?;
            }
            method => {
                handle.upload(true)?;
                handle.in_filesize(body_len as u64)?;
                handle.custom_request(method.as_str())?;
            }
        }

        let mut headers = curl::easy::List::new();
        for (name, value) in parts.headers {
            if let Some(name) = name {
                let value: &str = value.to_str().map_err(|_| Error::BadHeader {
                    name: name.to_string(),
                    bytes: value.as_bytes().to_owned(),
                })?;
                headers.append(&format!("{}: {}", name, value))?;
            }
        }
        handle.http_headers(headers)?;

        let (sender, receiver) = oneshot::channel();
        let req = Message {
            easy: handle,
            sender,
        };

        self.channel.as_ref().unwrap().send(req).unwrap();
        receiver.await.unwrap()
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        // Close the channel
        drop(self.channel.take().unwrap());
        // Join the thread
        let _ = self.thread_handle.take().unwrap().join();
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
            oneshot::Sender<HttpResult<Response>>,
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

    fn fail_and_drain(&mut self, e: &Error) {
        for (_token, (_handle, sender)) in self.handles.drain() {
            let _ = sender.send(Err(e.clone()));
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
                    self.fail_and_drain(&Error::Multi(e));
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
                        let mut response = std::mem::replace(
                            &mut easy.get_mut().response,
                            Response::new(Vec::new()),
                        );
                        if let Ok(status) = easy.response_code()
                            && status != 0
                            && let Ok(status) = http::StatusCode::from_u16(status as u16)
                        {
                            *response.status_mut() = status;
                        }
                        // Would be nice to set HTTP version via `response.version_mut()`, but `curl` doesn't have it exposed.
                        let extensions = Extensions {
                            client_ip: easy.primary_ip().ok().flatten().map(str::to_string),
                            effective_url: easy.effective_url().ok().flatten().map(str::to_string),
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
                        // Without this, we end up sitting in `Multi::wait` too long while new work is
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
                            self.fail_and_drain(&Error::Multi(e));
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
    response: Response,
    request_body: Cursor<Vec<u8>>,
    debug: bool,
}

impl Collector {
    fn new() -> Self {
        Collector {
            response: Response::new(Vec::new()),
            request_body: Cursor::new(Vec::new()),
            debug: false,
        }
    }
}

impl Handler for Collector {
    fn write(&mut self, data: &[u8]) -> Result<usize, WriteError> {
        self.response.body_mut().extend_from_slice(data);
        Ok(data.len())
    }

    fn header(&mut self, data: &[u8]) -> bool {
        if let Some((name, value)) = handle_http_header(data)
            && let Ok(name) = http::HeaderName::from_str(name)
            && let Ok(value) = http::HeaderValue::from_str(value)
        {
            self.response.headers_mut().append(name, value);
        }
        true
    }

    fn read(&mut self, data: &mut [u8]) -> Result<usize, curl::easy::ReadError> {
        Ok(self.request_body.read(data).unwrap())
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
    effective_url: Option<String>,
}

pub trait ResponsePartsExtensions {
    fn client_ip(&self) -> Option<&str>;
    fn effective_url(&self) -> Option<&str>;
}

impl ResponsePartsExtensions for http::response::Parts {
    fn client_ip(&self) -> Option<&str> {
        self.extensions
            .get::<Extensions>()
            .and_then(|extensions| extensions.client_ip.as_deref())
    }

    fn effective_url(&self) -> Option<&str> {
        self.extensions
            .get::<Extensions>()
            .and_then(|extensions| extensions.effective_url.as_deref())
    }
}

impl ResponsePartsExtensions for Response {
    fn client_ip(&self) -> Option<&str> {
        self.extensions()
            .get::<Extensions>()
            .and_then(|extensions| extensions.client_ip.as_deref())
    }

    fn effective_url(&self) -> Option<&str> {
        self.extensions()
            .get::<Extensions>()
            .and_then(|extensions| extensions.effective_url.as_deref())
    }
}

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
