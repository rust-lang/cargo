use util::Config;
use util::errors::{CargoError, CargoErrorKind, CargoResult};

use git2;

fn maybe_spurious(err: &CargoError) -> bool {
    match err.kind() {
            &CargoErrorKind::Git(ref git_err) => {
                match git_err.class() {
                    git2::ErrorClass::Net |
                    git2::ErrorClass::Os => true,
                    _ => false
                }
            }
            &CargoErrorKind::Curl(ref curl_err) => {
                curl_err.is_couldnt_connect() ||
                    curl_err.is_couldnt_resolve_proxy() ||
                    curl_err.is_couldnt_resolve_host() ||
                    curl_err.is_operation_timedout() ||
                    curl_err.is_recv_error()
            }
            &CargoErrorKind::HttpNot200(code, ref _url)  => {
                500 <= code && code < 600
            }
            _ => false
    }
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
