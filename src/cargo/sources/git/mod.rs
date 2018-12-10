pub use self::source::{canonicalize_url, GitSource};
pub use self::utils::{fetch, GitCheckout, GitDatabase, GitRemote, GitRevision};
mod source;
mod utils;
