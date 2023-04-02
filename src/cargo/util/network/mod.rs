//! Utilities for networking.

use std::task::Poll;

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

// When dynamically linked against libcurl, we want to ignore some failures
// when using old versions that don't support certain features.
#[macro_export]
macro_rules! try_old_curl {
    ($e:expr, $msg:expr) => {
        let result = $e;
        if cfg!(target_os = "macos") {
            if let Err(e) = result {
                warn!("ignoring libcurl {} error: {}", $msg, e);
            }
        } else {
            result.with_context(|| {
                anyhow::format_err!("failed to enable {}, is curl not built right?", $msg)
            })?;
        }
    };
}
