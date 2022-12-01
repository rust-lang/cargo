pub use self::source::GitSource;
pub use self::utils::{fetch, GitCheckout, GitDatabase, GitRemote};
mod known_hosts;
mod oxide;
mod source;
mod utils;

pub mod fetch {
    pub type Error = gix::env::collate::fetch::Error<gix::refspec::parse::Error>;
}
