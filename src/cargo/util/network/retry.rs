//! Utilities for retrying a network operation.
//!
//! Some network errors are considered "spurious", meaning it is not a real
//! error (such as a 404 not found) and is likely a transient error (like a
//! bad network connection) that we can hope will resolve itself shortly. The
//! [`Retry`] type offers a way to repeatedly perform some kind of network
//! operation with a delay if it detects one of these possibly transient
//! errors.
//!
//! This supports errors from [`git2`], [`gix`], [`curl`], and
//! [`HttpNotSuccessful`] 5xx HTTP errors.
//!
//! The number of retries can be configured by the user via the `net.retry`
//! config option. This indicates the number of times to retry the operation
//! (default 3 times for a total of 4 attempts).
//!
//! There are hard-coded constants that indicate how long to sleep between
//! retries. The constants are tuned to balance a few factors, such as the
//! responsiveness to the user (we don't want cargo to hang for too long
//! retrying things), and accommodating things like Cloudfront's default
//! negative TTL of 10 seconds (if Cloudfront gets a 5xx error for whatever
//! reason it won't try to fetch again for 10 seconds).
//!
//! The timeout also implements a primitive form of random jitter. This is so
//! that if multiple requests fail at the same time that they don't all flood
//! the server at the same time when they are retried. This jitter still has
//! some clumping behavior, but should be good enough.
//!
//! [`Retry`] is the core type for implementing retry logic. The
//! [`Retry::try`] method can be called with a callback, and it will
//! indicate if it needs to be called again sometime in the future if there
//! was a possibly transient error. The caller is responsible for sleeping the
//! appropriate amount of time and then calling [`Retry::try`] again.
//!
//! [`with_retry`] is a convenience function that will create a [`Retry`] and
//! handle repeatedly running a callback until it succeeds, or it runs out of
//! retries.
//!
//! Some interesting resources about retries:
//! - <https://aws.amazon.com/blogs/architecture/exponential-backoff-and-jitter/>
//! - <https://en.wikipedia.org/wiki/Exponential_backoff>
//! - <https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Retry-After>

use crate::util::errors::{GitCliError, HttpNotSuccessful};
use crate::{CargoResult, GlobalContext};
use anyhow::Error;
use rand::Rng;
use std::cmp::min;
use std::time::Duration;

/// State for managing retrying a network operation.
pub struct Retry<'a> {
    gctx: &'a GlobalContext,
    /// The number of failed attempts that have been done so far.
    ///
    /// Starts at 0, and increases by one each time an attempt fails.
    retries: u64,
    /// The maximum number of times the operation should be retried.
    ///
    /// 0 means it should never retry.
    max_retries: u64,
}

/// The result of attempting some operation via [`Retry::try`].
pub enum RetryResult<T> {
    /// The operation was successful.
    ///
    /// The wrapped value is the return value of the callback function.
    Success(T),
    /// The operation was an error, and it should not be tried again.
    Err(anyhow::Error),
    /// The operation failed, and should be tried again in the future.
    ///
    /// The wrapped value is the number of milliseconds to wait before trying
    /// again. The caller is responsible for waiting this long and then
    /// calling [`Retry::try`] again.
    Retry(u64),
}

/// Maximum amount of time a single retry can be delayed (milliseconds).
const MAX_RETRY_SLEEP_MS: u64 = 10 * 1000;
/// The minimum initial amount of time a retry will be delayed (milliseconds).
///
/// The actual amount of time will be a random value above this.
const INITIAL_RETRY_SLEEP_BASE_MS: u64 = 500;
/// The maximum amount of additional time the initial retry will take (milliseconds).
///
/// The initial delay will be [`INITIAL_RETRY_SLEEP_BASE_MS`] plus a random range
/// from 0 to this value.
const INITIAL_RETRY_JITTER_MS: u64 = 1000;

impl<'a> Retry<'a> {
    pub fn new(gctx: &'a GlobalContext) -> CargoResult<Retry<'a>> {
        Ok(Retry {
            gctx,
            retries: 0,
            max_retries: gctx.net_config()?.retry.unwrap_or(3) as u64,
        })
    }

    /// Calls the given callback, and returns a [`RetryResult`] which
    /// indicates whether or not this needs to be called again at some point
    /// in the future to retry the operation if it failed.
    pub fn r#try<T>(&mut self, f: impl FnOnce() -> CargoResult<T>) -> RetryResult<T> {
        match f() {
            Err(ref e) if maybe_spurious(e) && self.retries < self.max_retries => {
                let err = e.downcast_ref::<HttpNotSuccessful>();
                let err_msg = err
                    .map(|http_err| http_err.display_short())
                    .unwrap_or_else(|| e.root_cause().to_string());
                let left_retries = self.max_retries - self.retries;
                let msg = format!(
                    "spurious network error ({} {} remaining): {err_msg}",
                    left_retries,
                    if left_retries != 1 { "tries" } else { "try" }
                );
                if let Err(e) = self.gctx.shell().warn(msg) {
                    return RetryResult::Err(e);
                }
                self.retries += 1;
                let sleep = err
                    .and_then(|v| Self::parse_retry_after(v, &jiff::Timestamp::now()))
                    // Limit the Retry-After to a maximum value to avoid waiting too long.
                    .map(|retry_after| retry_after.min(MAX_RETRY_SLEEP_MS))
                    .unwrap_or_else(|| self.next_sleep_ms());
                RetryResult::Retry(sleep)
            }
            Err(e) => RetryResult::Err(e),
            Ok(r) => RetryResult::Success(r),
        }
    }

    /// Gets the next sleep duration in milliseconds.
    fn next_sleep_ms(&self) -> u64 {
        if let Ok(sleep) = self.gctx.get_env("__CARGO_TEST_FIXED_RETRY_SLEEP_MS") {
            return sleep.parse().expect("a u64");
        }

        if self.retries == 1 {
            let mut rng = rand::rng();
            INITIAL_RETRY_SLEEP_BASE_MS + rng.random_range(0..INITIAL_RETRY_JITTER_MS)
        } else {
            min(
                ((self.retries - 1) * 3) * 1000 + INITIAL_RETRY_SLEEP_BASE_MS,
                MAX_RETRY_SLEEP_MS,
            )
        }
    }

    /// Parse the HTTP `Retry-After` header.
    /// Returns the number of milliseconds to wait before retrying according to the header.
    fn parse_retry_after(response: &HttpNotSuccessful, now: &jiff::Timestamp) -> Option<u64> {
        // Only applies to HTTP 429 (too many requests) and 503 (service unavailable).
        if !matches!(response.code, 429 | 503) {
            return None;
        }

        // Extract the Retry-After header value.
        let retry_after = response
            .headers
            .iter()
            .filter_map(|h| h.split_once(':'))
            .map(|(k, v)| (k.trim(), v.trim()))
            .find(|(k, _)| k.eq_ignore_ascii_case("retry-after"))?
            .1;

        // First option: Retry-After is a positive integer of seconds to wait.
        if let Ok(delay_secs) = retry_after.parse::<u32>() {
            return Some(delay_secs as u64 * 1000);
        }

        // Second option: Retry-After is a future HTTP date string that tells us when to retry.
        if let Ok(retry_time) = jiff::fmt::rfc2822::parse(retry_after) {
            let diff_ms = now
                .until(&retry_time)
                .unwrap()
                .total(jiff::Unit::Millisecond)
                .unwrap();
            if diff_ms > 0.0 {
                return Some(diff_ms as u64);
            }
        }
        None
    }
}

fn maybe_spurious(err: &Error) -> bool {
    if let Some(git_err) = err.downcast_ref::<git2::Error>() {
        match git_err.class() {
            git2::ErrorClass::Net
            | git2::ErrorClass::Os
            | git2::ErrorClass::Zlib
            | git2::ErrorClass::Http => return git_err.code() != git2::ErrorCode::Certificate,
            _ => (),
        }
    }
    if let Some(curl_err) = err.downcast_ref::<curl::Error>() {
        if curl_err.is_couldnt_connect()
            || curl_err.is_couldnt_resolve_proxy()
            || curl_err.is_couldnt_resolve_host()
            || curl_err.is_operation_timedout()
            || curl_err.is_recv_error()
            || curl_err.is_send_error()
            || curl_err.is_http2_error()
            || curl_err.is_http2_stream_error()
            || curl_err.is_ssl_connect_error()
            || curl_err.is_partial_file()
        {
            return true;
        }
    }
    if let Some(not_200) = err.downcast_ref::<HttpNotSuccessful>() {
        if 500 <= not_200.code && not_200.code < 600 || not_200.code == 429 {
            return true;
        }
    }

    use gix::protocol::transport::IsSpuriousError;

    if let Some(err) = err.downcast_ref::<crate::sources::git::fetch::Error>() {
        if err.is_spurious() {
            return true;
        }
    }

    if let Some(err) = err.downcast_ref::<GitCliError>() {
        if err.is_spurious() {
            return true;
        }
    }

    false
}

/// Wrapper method for network call retry logic.
///
/// Retry counts provided by Config object `net.retry`. Config shell outputs
/// a warning on per retry.
///
/// Closure must return a `CargoResult`.
///
/// # Examples
///
/// ```
/// # use crate::cargo::util::{CargoResult, GlobalContext};
/// # let download_something = || return Ok(());
/// # let gctx = GlobalContext::default().unwrap();
/// use cargo::util::network;
/// let cargo_result = network::retry::with_retry(&gctx, || download_something());
/// ```
pub fn with_retry<T, F>(gctx: &GlobalContext, mut callback: F) -> CargoResult<T>
where
    F: FnMut() -> CargoResult<T>,
{
    let mut retry = Retry::new(gctx)?;
    loop {
        match retry.r#try(&mut callback) {
            RetryResult::Success(r) => return Ok(r),
            RetryResult::Err(e) => return Err(e),
            RetryResult::Retry(sleep) => std::thread::sleep(Duration::from_millis(sleep)),
        }
    }
}

#[test]
fn with_retry_repeats_the_call_then_works() {
    use crate::core::Shell;

    //Error HTTP codes (5xx) are considered maybe_spurious and will prompt retry
    let error1 = HttpNotSuccessful {
        code: 501,
        url: "Uri".to_string(),
        ip: None,
        body: Vec::new(),
        headers: Vec::new(),
    }
    .into();
    let error2 = HttpNotSuccessful {
        code: 502,
        url: "Uri".to_string(),
        ip: None,
        body: Vec::new(),
        headers: Vec::new(),
    }
    .into();
    let mut results: Vec<CargoResult<()>> = vec![Ok(()), Err(error1), Err(error2)];
    let gctx = GlobalContext::default().unwrap();
    *gctx.shell() = Shell::from_write(Box::new(Vec::new()));
    let result = with_retry(&gctx, || results.pop().unwrap());
    assert!(result.is_ok())
}

#[test]
fn with_retry_finds_nested_spurious_errors() {
    use crate::core::Shell;

    //Error HTTP codes (5xx) are considered maybe_spurious and will prompt retry
    //String error messages are not considered spurious
    let error1 = anyhow::Error::from(HttpNotSuccessful {
        code: 501,
        url: "Uri".to_string(),
        ip: None,
        body: Vec::new(),
        headers: Vec::new(),
    });
    let error1 = anyhow::Error::from(error1.context("A non-spurious wrapping err"));
    let error2 = anyhow::Error::from(HttpNotSuccessful {
        code: 502,
        url: "Uri".to_string(),
        ip: None,
        body: Vec::new(),
        headers: Vec::new(),
    });
    let error2 = anyhow::Error::from(error2.context("A second chained error"));
    let mut results: Vec<CargoResult<()>> = vec![Ok(()), Err(error1), Err(error2)];
    let gctx = GlobalContext::default().unwrap();
    *gctx.shell() = Shell::from_write(Box::new(Vec::new()));
    let result = with_retry(&gctx, || results.pop().unwrap());
    assert!(result.is_ok())
}

#[test]
fn default_retry_schedule() {
    use crate::core::Shell;

    let spurious = || -> CargoResult<()> {
        Err(anyhow::Error::from(HttpNotSuccessful {
            code: 500,
            url: "Uri".to_string(),
            ip: None,
            body: Vec::new(),
            headers: Vec::new(),
        }))
    };
    let gctx = GlobalContext::default().unwrap();
    *gctx.shell() = Shell::from_write(Box::new(Vec::new()));
    let mut retry = Retry::new(&gctx).unwrap();
    match retry.r#try(|| spurious()) {
        RetryResult::Retry(sleep) => {
            assert!(
                sleep >= INITIAL_RETRY_SLEEP_BASE_MS
                    && sleep < INITIAL_RETRY_SLEEP_BASE_MS + INITIAL_RETRY_JITTER_MS
            );
        }
        _ => panic!("unexpected non-retry"),
    }
    match retry.r#try(|| spurious()) {
        RetryResult::Retry(sleep) => assert_eq!(sleep, 3500),
        _ => panic!("unexpected non-retry"),
    }
    match retry.r#try(|| spurious()) {
        RetryResult::Retry(sleep) => assert_eq!(sleep, 6500),
        _ => panic!("unexpected non-retry"),
    }
    match retry.r#try(|| spurious()) {
        RetryResult::Err(_) => {}
        _ => panic!("unexpected non-retry"),
    }
}

#[test]
fn curle_http2_stream_is_spurious() {
    let code = curl_sys::CURLE_HTTP2_STREAM;
    let err = curl::Error::new(code);
    assert!(maybe_spurious(&err.into()));
}

#[test]
fn retry_after_parsing() {
    use crate::core::Shell;
    fn spurious(code: u32, header: &str) -> HttpNotSuccessful {
        HttpNotSuccessful {
            code,
            url: "Uri".to_string(),
            ip: None,
            body: Vec::new(),
            headers: vec![header.to_string()],
        }
    }

    // Start of year 2025.
    let now = jiff::Timestamp::new(1735689600, 0).unwrap();
    let headers = spurious(429, "Retry-After: 10");
    assert_eq!(Retry::parse_retry_after(&headers, &now), Some(10_000));
    let headers = spurious(429, "retry-after: Wed, 01 Jan 2025 00:00:10 GMT");
    let actual = Retry::parse_retry_after(&headers, &now).unwrap();
    assert_eq!(10000, actual);

    let headers = spurious(429, "Content-Type: text/html");
    assert_eq!(Retry::parse_retry_after(&headers, &now), None);

    let headers = spurious(429, "retry-after: Fri, 01 Jan 2000 00:00:00 GMT");
    assert_eq!(Retry::parse_retry_after(&headers, &now), None);

    let headers = spurious(429, "retry-after: -1");
    assert_eq!(Retry::parse_retry_after(&headers, &now), None);

    let headers = spurious(400, "retry-after: 1");
    assert_eq!(Retry::parse_retry_after(&headers, &now), None);

    let gctx = GlobalContext::default().unwrap();
    *gctx.shell() = Shell::from_write(Box::new(Vec::new()));
    let mut retry = Retry::new(&gctx).unwrap();
    match retry
        .r#try(|| -> CargoResult<()> { Err(anyhow::Error::from(spurious(429, "Retry-After: 7"))) })
    {
        RetryResult::Retry(sleep) => assert_eq!(sleep, 7_000),
        _ => panic!("unexpected non-retry"),
    }
}

#[test]
fn git_cli_error_spurious() {
    let error = GitCliError::new(Error::msg("test-git-cli-error"), false);
    assert!(!maybe_spurious(&error.into()));

    let error = GitCliError::new(Error::msg("test-git-cli-error"), true);
    assert!(maybe_spurious(&error.into()));
}
