//! Async wrapper around cURL for making managing HTTP requests.
//!
//! Requests are executed in parallel using cURL [`Multi`] on
//! a worker thread that is owned by the Client.

use std::collections::HashMap;
use std::io::Cursor;
use std::io::Read;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::thread::JoinHandle;
use std::time::Duration;
use std::time::Instant;

use curl::easy::Easy2;
use curl::easy::Handler;
use curl::easy::InfoType;
use curl::easy::WriteError;
use curl::multi::Easy2Handle;
use curl::multi::Multi;
use futures::channel::oneshot;
use tracing::{debug, error, trace, warn};

use crate::util::network::http::HandleConfiguration;
use crate::util::network::http::HttpTimeout;

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

    #[error(
        "transfer too slow: failed to transfer more than {low_speed_limit} bytes in {}s (transferred {transferred} bytes)",
        timeout_dur.as_secs()
    )]
    TooSlow {
        low_speed_limit: u32,
        timeout_dur: Duration,
        transferred: u64,
    },

    #[error("failed to convert header value of `{name}` to string: {bytes:?}")]
    BadHeader { name: String, bytes: Vec<u8> },
}

struct Message {
    easy: Easy2<Collector>,
    sender: oneshot::Sender<HttpResult<Response>>,
}

#[derive(Default)]
struct Stats {
    dl_remaining: AtomicI64,
    dl_transferred: AtomicU64,
}

/// HTTP Client. Creating a new client spawns a cURL `Multi` and
/// thread that is used for all HTTP requests by this client.
pub struct Client {
    channel: Option<Sender<Message>>,
    thread_handle: Option<JoinHandle<()>>,
    handle_config: HandleConfiguration,
    stats: Arc<Stats>,
}

impl Client {
    /// Spawns a new worker thread where HTTP request execute.
    pub fn new(handle_config: HandleConfiguration) -> Client {
        let (tx, rx) = mpsc::channel();
        let stats = Arc::new(Stats::default());
        let timeout = handle_config.timeout.clone();
        let worker_stats = stats.clone();
        let handle = std::thread::spawn(move || {
            WorkerServer::run(rx, handle_config.multiplexing, timeout, worker_stats)
        });
        Client {
            channel: Some(tx),
            thread_handle: Some(handle),
            handle_config,
            stats,
        }
    }

    /// Perform an HTTP request using this client.
    pub async fn request(&self, request: Request) -> HttpResult<Response> {
        let url = request.uri().to_string();
        debug!(target: "network::fetch", url);
        let mut collector = Collector::new(self.stats.clone());
        let (parts, body) = request.into_parts();
        let body_len = body.len();
        collector.request_body = Cursor::new(body);
        collector.debug = self.handle_config.verbose;
        let mut handle = curl::easy::Easy2::new(collector);
        self.handle_config.configure2(&mut handle)?;

        handle.url(&url)?;
        handle.follow_location(true)?;
        handle.progress(true)?;

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

    /// Returns the number pending bytes across all active transfers.
    pub fn bytes_pending(&self) -> u64 {
        self.stats
            .dl_remaining
            .load(Ordering::Acquire)
            .try_into()
            .unwrap()
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
    /// Channel to receive new work
    incoming_work: Receiver<Message>,
    /// curl multi interface
    multi: Multi,
    /// Map of token to curl handle and response channel
    handles: HashMap<
        usize,
        (
            Easy2Handle<Collector>,
            oneshot::Sender<HttpResult<Response>>,
        ),
    >,
    /// Next token to use
    token: usize,
    /// Global timeout configuration
    timeout: HttpTimeout,
    /// Global transfer statistics
    stats: Arc<Stats>,
    /// Instant when the current low speed window started
    low_speed_window_start: Instant,
    /// Amount of total bytes transferred when the current low speed window started
    low_speed_window_initial: u64,
}

impl WorkerServer {
    fn run(
        incoming_work: Receiver<Message>,
        multiplex: bool,
        timeout: HttpTimeout,
        stats: Arc<Stats>,
    ) {
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
            timeout,
            stats,
            low_speed_window_start: Instant::now(),
            low_speed_window_initial: 0,
        };
        worker.worker_loop();
    }

    fn fail_and_drain(&mut self, e: &Error) {
        warn!(
            target: "network",
            "failing all outstanding HTTP requests: {e}"
        );
        for (_token, (_handle, sender)) in self.handles.drain() {
            let _ = sender.send(Err(e.clone()));
        }
    }

    /// Marks the start of a new timeout window.
    fn reset_low_speed_timeout(&mut self) {
        self.low_speed_window_start = Instant::now();
        self.low_speed_window_initial = self.stats.dl_transferred.load(Ordering::Acquire);
    }

    /// Return an error if we're at the end of a timeout window, we haven't
    /// made enough progress.
    fn check_low_speed_timeout(&mut self) -> Option<Error> {
        // Make sure we've waited for the timeout duration
        if Instant::now().duration_since(self.low_speed_window_start) < self.timeout.dur {
            return None;
        }

        // Calculate how much we've transferred since the last check.
        let current = self.stats.dl_transferred.load(Ordering::Acquire);
        let transferred = current.saturating_sub(self.low_speed_window_initial);
        self.reset_low_speed_timeout();
        if transferred < self.timeout.low_speed_limit.into() {
            Some(Error::TooSlow {
                low_speed_limit: self.timeout.low_speed_limit,
                timeout_dur: self.timeout.dur,
                transferred,
            })
        } else {
            None
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
                        // Check for low speed timeout.
                        if let Some(timeout_error) = self.check_low_speed_timeout() {
                            self.fail_and_drain(&timeout_error);
                            continue;
                        }

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
                                self.reset_low_speed_timeout();
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
    /// The response being built
    response: Response,
    /// The body to transmit
    request_body: Cursor<Vec<u8>>,
    /// Whether we're in debug mode
    debug: bool,
    /// Global transfer statistics.
    global_stats: Arc<Stats>,
    /// How much has this particular transfer added to global `dl_remaining` stats.
    dl_remaining_delta: i64,
}

impl Collector {
    fn new(stats: Arc<Stats>) -> Self {
        Collector {
            response: Response::new(Vec::new()),
            request_body: Cursor::new(Vec::new()),
            debug: false,
            global_stats: stats,
            dl_remaining_delta: 0,
        }
    }
}

impl Handler for Collector {
    fn write(&mut self, data: &[u8]) -> Result<usize, WriteError> {
        self.response.body_mut().extend_from_slice(data);
        self.global_stats
            .dl_transferred
            .fetch_add(data.len() as u64, Ordering::Release);
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

    fn progress(&mut self, dltotal: f64, dlnow: f64, _ultotal: f64, _ulnow: f64) -> bool {
        if dlnow > dltotal {
            return true;
        }
        let dl_total = dltotal as i64;
        let dl_current = dlnow as i64;

        let remaining = dl_total - dl_current;

        self.global_stats
            .dl_remaining
            .fetch_add(remaining - self.dl_remaining_delta, Ordering::Release);
        self.dl_remaining_delta = remaining;
        true
    }
}

impl Drop for Collector {
    fn drop(&mut self) {
        // Zero out this transfer's contribution to the global dl_remaining.
        self.global_stats
            .dl_remaining
            .fetch_add(-self.dl_remaining_delta, Ordering::Release);
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
