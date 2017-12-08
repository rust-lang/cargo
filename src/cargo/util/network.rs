use curl;
use git2;

use failure::Error;

use util::Config;
use util::errors::{CargoResult, HttpNot200};

fn maybe_spurious(err: &Error) -> bool {
    for e in err.causes() {
        if let Some(git_err) = e.downcast_ref::<git2::Error>() {
            match git_err.class() {
                git2::ErrorClass::Net |
                git2::ErrorClass::Os => return true,
                _ => ()
            }
        }
        if let Some(curl_err) = e.downcast_ref::<curl::Error>() {
            if curl_err.is_couldnt_connect() ||
                curl_err.is_couldnt_resolve_proxy() ||
                curl_err.is_couldnt_resolve_host() ||
                curl_err.is_operation_timedout() ||
                curl_err.is_recv_error() {
                return true
            }
        }
        if let Some(not_200) = e.downcast_ref::<HttpNot200>() {
            if 500 <= not_200.code && not_200.code < 600 {
                return true
            }
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
/// ```ignore
/// use util::network;
/// cargo_result = network.with_retry(&config, || something.download());
/// ```
pub fn with_retry<T, F>(config: &Config, mut callback: F) -> CargoResult<T>
    where F: FnMut() -> CargoResult<T>
{
    let mut remaining = config.net_retry()?;
    loop {
        match callback() {
            Ok(ret) => return Ok(ret),
            Err(ref e) if maybe_spurious(e) && remaining > 0 => {
                let msg = format!("spurious network error ({} tries \
                          remaining): {}", remaining, e);
                config.shell().warn(msg)?;
                remaining -= 1;
            }
            //todo impl from
            Err(e) => return Err(e.into()),
        }
    }
}
#[test]
fn with_retry_repeats_the_call_then_works() {
    //Error HTTP codes (5xx) are considered maybe_spurious and will prompt retry
    let error1 = HttpNot200 { code: 501, url: "Uri".to_string() }.into();
    let error2 = HttpNot200 { code: 502, url: "Uri".to_string() }.into();
    let mut results: Vec<CargoResult<()>> = vec![Ok(()), Err(error1), Err(error2)];
    let config = Config::default().unwrap();
    let result = with_retry(&config, || results.pop().unwrap());
    assert_eq!(result.unwrap(), ())
}

#[test]
fn with_retry_finds_nested_spurious_errors() {
    use util::CargoError;

    //Error HTTP codes (5xx) are considered maybe_spurious and will prompt retry
    //String error messages are not considered spurious
    let error1 = CargoError::from(HttpNot200 { code: 501, url: "Uri".to_string() });
    let error1 = CargoError::from(error1.context("A non-spurious wrapping err"));
    let error2 = CargoError::from(HttpNot200 { code: 502, url: "Uri".to_string() });
    let error2 = CargoError::from(error2.context("A second chained error"));
    let mut results: Vec<CargoResult<()>> = vec![Ok(()), Err(error1), Err(error2)];
    let config = Config::default().unwrap();
    let result = with_retry(&config, || results.pop().unwrap());
    assert_eq!(result.unwrap(), ())
}
