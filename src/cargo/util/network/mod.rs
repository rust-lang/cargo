//! Utilities for networking.

use std::task::Poll;

pub mod http;
pub mod proxy;
pub mod retry;
pub mod sleep;

pub trait PollExt<T> {
    fn expect(self, msg: &str) -> T;
}

impl<T> PollExt<T> for Poll<T> {
    #[track_caller]
    fn expect(self, msg: &str) -> T {
        match self {
            Poll::Ready(val) => val,
            Poll::Pending => panic!("{}", msg),
        }
    }
}

/// When dynamically linked against libcurl, we want to ignore some failures
/// when using old versions that don't support certain features.
#[macro_export]
macro_rules! try_old_curl {
    ($e:expr, $msg:expr) => {
        let result = $e;
        if cfg!(target_os = "macos") {
            if let Err(e) = result {
                ::tracing::warn!(target: "network", "ignoring libcurl {} error: {}", $msg, e);
            }
        } else {
            use ::anyhow::Context;
            result.with_context(|| {
                ::anyhow::format_err!("failed to enable {}, is curl not built right?", $msg)
            })?;
        }
    };
}

/// Enable HTTP/2 and pipewait to be used as it'll allow true multiplexing
/// which makes downloads much faster.
///
/// Currently Cargo requests the `http2` feature of the `curl` crate which
/// means it should always be built in. On OSX, however, we ship cargo still
/// linked against the system libcurl. Building curl with ALPN support for
/// HTTP/2 requires newer versions of OSX (the SecureTransport API) than we
/// want to ship Cargo for. By linking Cargo against the system libcurl then
/// older curl installations won't use HTTP/2 but newer ones will. All that to
/// basically say we ignore errors here on OSX, but consider this a fatal error
/// to not activate HTTP/2 on all other platforms.
///
/// `pipewait` is an option which indicates that if there's a bunch of parallel
/// requests to the same host they all wait until the pipelining status of the
/// host is known. This means that we won't initiate dozens of connections but
/// rather only one. Once the main one is opened we realized that pipelining is
/// possible and multiplexing is possible. All in all this reduces the number
/// of connections down to a more manageable state.
#[macro_export]
macro_rules! try_old_curl_http2_pipewait {
    ($multiplexing:expr, $handle:expr) => {
        if $multiplexing {
            $crate::try_old_curl!($handle.http_version(curl::easy::HttpVersion::V2), "HTTP/2");
        } else {
            $handle.http_version(curl::easy::HttpVersion::V11)?;
        }
        $crate::try_old_curl!($handle.pipewait(true), "pipewait");
    };
}
