use util::Config;
use util::errors::CargoResult;

use curl;
use git2;

// =============================================================================
// NetworkError chain
error_chain!{
    types {
        NetworkError, NetworkErrorKind, NetworkResultExt, NetworkResult;
    }

    foreign_links {
        Git(git2::Error);
        Curl(curl::Error);
    }

    errors {
        HttpNot200(code: u32, url: String) {
            description("failed to get a 200 response")
            display("failed to get 200 response from `{}`, got {}", url, code)
        }
    }
}

impl NetworkError {
    pub fn maybe_spurious(&self) -> bool {
        match &self.0 {
            &NetworkErrorKind::Msg(_) => false,
            &NetworkErrorKind::Git(ref git_err) => {
                match git_err.class() {
                    git2::ErrorClass::Net |
                    git2::ErrorClass::Os => true,
                    _ => false
                }
            }
            &NetworkErrorKind::Curl(ref curl_err) => {
                curl_err.is_couldnt_connect() ||
                    curl_err.is_couldnt_resolve_proxy() ||
                    curl_err.is_couldnt_resolve_host() ||
                    curl_err.is_operation_timedout() ||
                    curl_err.is_recv_error()
            }
            &NetworkErrorKind::HttpNot200(code, ref _url)  => {
                500 <= code && code < 600
            }
        }
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
    where F: FnMut() -> NetworkResult<T>
{
    let mut remaining = config.net_retry()?;
    loop {
        match callback() {
            Ok(ret) => return Ok(ret),
            Err(ref e) if e.maybe_spurious() && remaining > 0 => {
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
    let error1 = NetworkErrorKind::HttpNot200(501, "Uri".to_string()).into();
    let error2 = NetworkErrorKind::HttpNot200(502, "Uri".to_string()).into();
    let mut results: Vec<NetworkResult<()>> = vec![Ok(()), Err(error1), Err(error2)];
    let config = Config::default().unwrap();
    let result = with_retry(&config, || results.pop().unwrap());
    assert_eq!(result.unwrap(), ())
}
