pub use self::utils::{GitRemote, GitDatabase, GitCheckout, GitRevision};
pub use self::source::{GitSource, canonicalize_url};
mod utils;
mod source;
