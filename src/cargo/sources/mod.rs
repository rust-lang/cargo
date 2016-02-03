pub use self::path::PathSource;
pub use self::git::GitSource;
pub use self::registry::{RegistrySource, CRATES_IO};
pub use self::replaced::ReplacedSource;
pub use self::config::SourceConfigMap;

pub mod path;
pub mod git;
pub mod registry;
pub mod config;
pub mod replaced;
