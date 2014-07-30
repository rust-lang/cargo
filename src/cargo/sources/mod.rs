pub use self::path::PathSource;
pub use self::git::GitSource;
pub use self::registry::DummyRegistrySource;

pub mod path;
pub mod git;
pub mod registry;
