
pub use self::dependency::Dependency;
pub use self::registry::{
  Registry,
  MemRegistry};

pub use self::manifest::{
  Manifest,
  Project,
  LibTarget,
  ExecTarget};

pub use self::package::Package;

mod dependency;
mod manifest;
mod package;
mod registry;
mod resolver;
