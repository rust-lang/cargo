use std;
use std::error::Error;

use error_chain::ChainedError;

use util::Config;
use util::errors::{CargoError, CargoErrorKind, CargoResult};

use git2;
fn maybe_spurious<E, EKind>(err: &E) -> bool
    where E: ChainedError<ErrorKind=EKind> + 'static {
    //Error inspection in non-verbose mode requires inspecting the
    //error kind to avoid printing Internal errors. The downcasting
    //machinery requires &(Error + 'static), but the iterator (and
    //underlying `cause`) return &Error. Because the borrows are
    //constrained to this handling method, and because the original
    //error object is constrained to be 'static, we're casting away
    //the borrow's actual lifetime for purposes of downcasting and
    //inspecting the error chain
    unsafe fn extend_lifetime(r: &Error) -> &(Error + 'static) {
        std::mem::transmute::<&Error, &Error>(r)    
    }

    for e in err.iter() {
        let e = unsafe { extend_lifetime(e) };
        if let Some(cargo_err) = e.downcast_ref::<CargoError>() {
            match cargo_err.kind() {
                &CargoErrorKind::Git(ref git_err) => {
                    match git_err.class() {
                        git2::ErrorClass::Net |
                        git2::ErrorClass::Os => return true,
                        _ => ()
                    }
                }
                &CargoErrorKind::Curl(ref curl_err) 
                    if curl_err.is_couldnt_connect() ||
                        curl_err.is_couldnt_resolve_proxy() ||
                        curl_err.is_couldnt_resolve_host() ||
                        curl_err.is_operation_timedout() ||
                        curl_err.is_recv_error() => {
                    return true
                }
                &CargoErrorKind::HttpNot200(code, ref _url) if 500 <= code && code < 600 => {
                    return true
                }
                _ => ()
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
/// Closure must return a CargoResult.
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
    let error1 = CargoErrorKind::HttpNot200(501, "Uri".to_string()).into();
    let error2 = CargoErrorKind::HttpNot200(502, "Uri".to_string()).into();
    let mut results: Vec<CargoResult<()>> = vec![Ok(()), Err(error1), Err(error2)];
    let config = Config::default().unwrap();
    let result = with_retry(&config, || results.pop().unwrap());
    assert_eq!(result.unwrap(), ())
}

#[test]
fn with_retry_finds_nested_spurious_errors() {
    //Error HTTP codes (5xx) are considered maybe_spurious and will prompt retry
    //String error messages are not considered spurious
    let error1 : CargoError = CargoErrorKind::HttpNot200(501, "Uri".to_string()).into();
    let error1 = CargoError::with_chain(error1, "A non-spurious wrapping err");
    let error2 = CargoError::from_kind(CargoErrorKind::HttpNot200(502, "Uri".to_string()));
    let error2 = CargoError::with_chain(error2, "A second chained error");
    let mut results: Vec<CargoResult<()>> = vec![Ok(()), Err(error1), Err(error2)];
    let config = Config::default().unwrap();
    let result = with_retry(&config, || results.pop().unwrap());
    assert_eq!(result.unwrap(), ())
}
