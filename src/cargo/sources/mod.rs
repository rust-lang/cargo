pub use self::config::SourceConfigMap;
pub use self::directory::DirectorySource;
pub use self::git::GitSource;
pub use self::path::PathSource;
pub use self::registry::{RegistrySource, CRATES_IO_DOMAIN, CRATES_IO_INDEX, CRATES_IO_REGISTRY};
pub use self::replaced::ReplacedSource;

pub mod config;
pub mod directory;
pub mod git;
pub mod path;
pub mod registry;
pub mod replaced;
