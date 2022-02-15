use anyhow::Error;

use crate::util::errors::{CargoResult, HttpNot200};
use crate::util::Config;

pub struct Retry<'a> {
    config: &'a Config,
    remaining: u32,
}

impl<'a> Retry<'a> {
    pub fn new(config: &'a Config) -> CargoResult<Retry<'a>> {
        Ok(Retry {
            config,
            remaining: config.net_config()?.retry.unwrap_or(2),
        })
    }

    pub fn r#try<T>(&mut self, f: impl FnOnce() -> CargoResult<T>) -> CargoResult<Option<T>> {
        match f() {
            Err(ref e) if maybe_spurious(e) && self.remaining > 0 => {
                let msg = format!(
                    "spurious network error ({} tries remaining): {}",
                    self.remaining,
                    e.root_cause(),
                );
                self.config.shell().warn(msg)?;
                self.remaining -= 1;
                Ok(None)
            }
            other => other.map(Some),
        }
    }
}

fn maybe_spurious(err: &Error) -> bool {
    if let Some(git_err) = err.downcast_ref::<git2::Error>() {
        match git_err.class() {
            git2::ErrorClass::Net
            | git2::ErrorClass::Os
            | git2::ErrorClass::Zlib
            | git2::ErrorClass::Http => return true,
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
    if let Some(not_200) = err.downcast_ref::<HttpNot200>() {
        if 500 <= not_200.code && not_200.code < 600 {
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
/// let cargo_result = network::with_retry(&config, || download_something());
/// ```
pub fn with_retry<T, F>(config: &Config, mut callback: F) -> CargoResult<T>
where
    F: FnMut() -> CargoResult<T>,
{
    let mut retry = Retry::new(config)?;
    loop {
        if let Some(ret) = retry.r#try(&mut callback)? {
            return Ok(ret);
        }
    }
}

#[test]
fn with_retry_repeats_the_call_then_works() {
    use crate::core::Shell;

    //Error HTTP codes (5xx) are considered maybe_spurious and will prompt retry
    let error1 = HttpNot200 {
        code: 501,
        url: "Uri".to_string(),
    }
    .into();
    let error2 = HttpNot200 {
        code: 502,
        url: "Uri".to_string(),
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
    let error1 = anyhow::Error::from(HttpNot200 {
        code: 501,
        url: "Uri".to_string(),
    });
    let error1 = anyhow::Error::from(error1.context("A non-spurious wrapping err"));
    let error2 = anyhow::Error::from(HttpNot200 {
        code: 502,
        url: "Uri".to_string(),
    });
    let error2 = anyhow::Error::from(error2.context("A second chained error"));
    let mut results: Vec<CargoResult<()>> = vec![Ok(()), Err(error1), Err(error2)];
    let config = Config::default().unwrap();
    *config.shell() = Shell::from_write(Box::new(Vec::new()));
    let result = with_retry(&config, || results.pop().unwrap());
    assert!(result.is_ok())
}

#[test]
fn curle_http2_stream_is_spurious() {
    let code = curl_sys::CURLE_HTTP2_STREAM;
    let err = curl::Error::new(code);
    assert!(maybe_spurious(&err.into()));
}
