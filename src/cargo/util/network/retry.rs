//! Utilities for retrying a network operation.

use crate::util::errors::HttpNotSuccessful;
use crate::{CargoResult, Config};
use anyhow::Error;
use rand::Rng;
use std::cmp::min;
use std::time::Duration;

pub struct Retry<'a> {
    config: &'a Config,
    retries: u64,
    max_retries: u64,
}

pub enum RetryResult<T> {
    Success(T),
    Err(anyhow::Error),
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
    pub fn new(config: &'a Config) -> CargoResult<Retry<'a>> {
        Ok(Retry {
            config,
            retries: 0,
            max_retries: config.net_config()?.retry.unwrap_or(3) as u64,
        })
    }

    /// Returns `Ok(None)` for operations that should be re-tried.
    pub fn r#try<T>(&mut self, f: impl FnOnce() -> CargoResult<T>) -> RetryResult<T> {
        match f() {
            Err(ref e) if maybe_spurious(e) && self.retries < self.max_retries => {
                let err_msg = e
                    .downcast_ref::<HttpNotSuccessful>()
                    .map(|http_err| http_err.display_short())
                    .unwrap_or_else(|| e.root_cause().to_string());
                let msg = format!(
                    "spurious network error ({} tries remaining): {err_msg}",
                    self.max_retries - self.retries,
                );
                if let Err(e) = self.config.shell().warn(msg) {
                    return RetryResult::Err(e);
                }
                self.retries += 1;
                let sleep = if self.retries == 1 {
                    let mut rng = rand::thread_rng();
                    INITIAL_RETRY_SLEEP_BASE_MS + rng.gen_range(0..INITIAL_RETRY_JITTER_MS)
                } else {
                    min(
                        ((self.retries - 1) * 3) * 1000 + INITIAL_RETRY_SLEEP_BASE_MS,
                        MAX_RETRY_SLEEP_MS,
                    )
                };
                RetryResult::Retry(sleep)
            }
            Err(e) => RetryResult::Err(e),
            Ok(r) => RetryResult::Success(r),
        }
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
        if 500 <= not_200.code && not_200.code < 600 {
            return true;
        }
    }

    use gix::protocol::transport::IsSpuriousError;

    if let Some(err) = err.downcast_ref::<crate::sources::git::fetch::Error>() {
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
/// # use crate::cargo::util::{CargoResult, Config};
/// # let download_something = || return Ok(());
/// # let config = Config::default().unwrap();
/// use cargo::util::network;
/// let cargo_result = network::retry::with_retry(&config, || download_something());
/// ```
pub fn with_retry<T, F>(config: &Config, mut callback: F) -> CargoResult<T>
where
    F: FnMut() -> CargoResult<T>,
{
    let mut retry = Retry::new(config)?;
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
    let config = Config::default().unwrap();
    *config.shell() = Shell::from_write(Box::new(Vec::new()));
    let result = with_retry(&config, || results.pop().unwrap());
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
    let config = Config::default().unwrap();
    *config.shell() = Shell::from_write(Box::new(Vec::new()));
    let result = with_retry(&config, || results.pop().unwrap());
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
    let config = Config::default().unwrap();
    *config.shell() = Shell::from_write(Box::new(Vec::new()));
    let mut retry = Retry::new(&config).unwrap();
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
