pub use self::utils::{GitRemote, GitDatabase, GitCheckout, GitRevision, fetch};
pub use self::source::{GitSource, canonicalize_url};
mod utils;
mod source;
