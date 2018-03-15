pub use self::utils::{fetch, GitCheckout, GitDatabase, GitRemote, GitRevision};
pub use self::source::{canonicalize_url, GitSource};
mod utils;
mod source;
