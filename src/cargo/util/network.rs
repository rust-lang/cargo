use util::{CargoResult, Config, errors};

/// Wrapper method for network call retry logic.
///
/// Retry counts provided by Config object 'net.retry'. Config shell outputs
/// a warning on per retry.
///
/// Closure must return a CargoResult.
///
/// Example:
/// use util::network;
/// cargo_result = network.with_retry(&config, || something.download());
pub fn with_retry<T, E, F>(config: &Config, mut callback: F) -> CargoResult<T>
    where F: FnMut() -> Result<T, E>,
          E: errors::NetworkError
{
    let mut remaining = try!(config.net_retry());
    loop {
        match callback() {
            Ok(ret) => return Ok(ret),
            Err(ref e) if e.maybe_spurious() && remaining > 0 => {
                let msg = format!("spurious network error ({} tries \
                          remaining): {}", remaining, e);
                try!(config.shell().warn(msg));
                remaining -= 1;
            }
            Err(e) => return Err(Box::new(e)),
        }
    }
}
#[test]
fn with_retry_repeats_the_call_then_works() {

    use std::error::Error;
    use util::human;
    use std::fmt;

    #[derive(Debug)]
    struct NetworkRetryError {
        error: Box<errors::CargoError>,
    }

    impl Error for NetworkRetryError {
        fn description(&self) -> &str {
            self.error.description()
        }
        fn cause(&self) -> Option<&Error> {
            self.error.cause()
        }
    }

    impl NetworkRetryError {
        fn new(error: &str) -> NetworkRetryError {
            let error = human(error.to_string());
            NetworkRetryError {
                error: error,
            }
        }
    }

    impl fmt::Display for NetworkRetryError {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            fmt::Display::fmt(&self.error, f)
        }
    }

    impl errors::CargoError for NetworkRetryError {
        fn is_human(&self) -> bool {
            false
        }
    }

    impl errors::NetworkError for NetworkRetryError {
        fn maybe_spurious(&self) -> bool {
            true
        }
    }

    let error1 = NetworkRetryError::new("one");
    let error2 = NetworkRetryError::new("two");
    let mut results: Vec<Result<(), NetworkRetryError>> = vec![Ok(()),
    Err(error1), Err(error2)];
    let config = Config::default().unwrap();
    let result = with_retry(&config, || results.pop().unwrap());
    assert_eq!(result.unwrap(), ())
}
